use crate::common::fs;
use crate::docker::{DockerRun, Mount};
use crate::{docker, project};
use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, trace, warn};
use serde::{Deserialize, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use std::env;
use std::path::{Path, PathBuf};

/// Run a cargo make command in Twoliter's build environment. Certain environment variable paths
/// from Makefile.toml are taken here as explicit arguments so that the caller can decide which of
/// these configurable paths may need to be mounted by Twoliter.
#[derive(Debug, Parser)]
pub(crate) struct Exec {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long = "project-path")]
    project_path: Option<PathBuf>,

    /// Path to the docker daemon socket.
    #[clap(long = "docker-socket", default_value = "/var/run/docker.sock")]
    docker_socket: String,

    /// Cargo make target. E.g. the word "build" if we want to execute `cargo make build`.
    makefile_target: Target,

    /// Arguments to be passed to cargo make
    additional_args: Vec<String>,
}

impl Exec {
    pub(super) async fn run(&self) -> Result<()> {
        let (project, path) = project::load_or_find_project(self.project_path.clone()).await?;
        let project_dir = canonicalize(path.parent().context(format!(
            "Unable to find the parent directory containing project file '{}'",
            path.display()
        ))?)?;
        // TODO - get smart about sdk: https://github.com/bottlerocket-os/twoliter/issues/11
        let sdk = project.sdk.clone().unwrap_or_default();
        // TODO - peek at cargo make args to see if we can figure out what the arch is (so we don't
        // pull two SDK containers). The arch for Twoliter execution doesn't matter.
        let image = docker::create_twoliter_image_if_not_exists(&sdk.uri("x86_64")).await?;

        let socket_mount = Mount {
            source: PathBuf::from(self.docker_socket.clone()),
            destination: PathBuf::from("/var/run/docker.sock"),
            ..Default::default()
        };

        // Mount /tmp for processes that use mktmp or otherwise expect to be able to use mount /tmp
        // in docker run statements.
        let tmp_dir = std::env::temp_dir();
        let tmp_mount = Mount {
            source: tmp_dir.clone(),
            destination: tmp_dir,
            ..Default::default()
        };

        let mut docker_command = DockerRun::new(image)
            .remove()
            .name("twoliter-exec")
            .mount(socket_mount)
            .mount(tmp_mount)
            .user(nix::unistd::Uid::effective().to_string())
            .workdir(project_dir.display().to_string())
            .command_arg("cargo")
            .command_arg("make")
            .command_arg("--loglevel=debug")
            .command_arg("--disable-check-for-updates")
            .command_arg("--makefile")
            .command_arg("/twoliter/tools/Makefile.toml")
            .command_arg("--cwd")
            .command_arg(project_dir.display().to_string())
            ._env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
            ._env("HOME", "/twoliter");

        let mounts = self.prepare_mounts(&project_dir).await?;
        for mount in mounts {
            docker_command = docker_command.mount(mount);
        }

        // TODO - this can panic if non-unicode env
        for (key, val) in std::env::vars() {
            if is_build_system_env(key.as_str()) {
                debug!("Passing env var {} to cargo make", key);
                docker_command = docker_command
                    .command_arg("-e".to_string())
                    .command_arg(format!("{}={}", key, val));
            } else {
                trace!("Not passing env var {} to cargo make", key);
            }
        }

        docker_command = docker_command
            .command_arg("-e")
            .command_arg(format!("BUILDSYS_ROOT_DIR={}", project_dir.display()));

        // These have to go last because the last of these might be the Makefile.toml target.
        for cargo_make_arg in &self.additional_args {
            docker_command = docker_command.command_arg(cargo_make_arg);
        }
        docker_command.execute().await?;
        Ok(())
    }

    /// Figure out which paths need to be mounted and create some of the directories if they should
    /// be created. `project_dir` is expected to be already canonicalized.
    async fn prepare_mounts(&self, project_dir: impl AsRef<Path>) -> Result<Vec<Mount>> {
        let project_dir = project_dir.as_ref();
        let mut mounts = vec![Mount::new(project_dir)];

        // Get paths from env args and prepare add them to our mounts.
        // TODO - we actually need to consider the dependency graph between tasks.
        for path_var in self.makefile_target.paths() {
            let path = match env::var(path_var.name) {
                Ok(value) => value,
                Err(e) => {
                    warn!(
                        "Unable to read environment variable '{}': {}",
                        path_var.name, e
                    );
                    continue;
                }
            };

            add(
                &mut mounts,
                project_dir,
                &PathBuf::from(path),
                path_var.r#type,
                path_var.create,
            )
            .await?;
        }
        // TODO: mount paths if we find any in the args and we need them.

        if let Some(testsys_test_path) = find_testsys_test_path(env::args()) {
            let testsys_test_path = canonicalize(testsys_test_path)?;
            mounts.push(Mount::new(testsys_test_path));
        }

        Ok(mounts)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum PathType {
    File,
    Dir,
}

// Short-hand so the above function calls can fit on one line.
const FILE: PathType = PathType::File;
const DIR: PathType = PathType::Dir;

// Readability for the above function calls, which can still fit on one line using these.
const CREATE: bool = true;
const NO_CREATE: bool = false;

/// If `create` is `true` and the path is a filepath, the parent dir will be created and mounted.
/// If `create` is `true` and the path is a dir, the dir will be created and mounted.
/// If `create` is `false` no directory will be created and the function will error because it
/// cannot canonicalize the path.
/// If a path should be mounted, it will be added to the `mounts` vec.
async fn add(
    mounts: &mut Vec<Mount>,
    project_dir: &Path,
    path: &Path,
    path_type: PathType,
    create: bool,
) -> Result<()> {
    let exists = path.exists();
    let in_project = path.starts_with(project_dir);
    let uncanonicalized_mount_path = if create && !exists && !in_project {
        match path_type {
            PathType::File => {
                let parent = path.parent().context(format!(
                    "Unable to create a directory for file '{}' \
                    because the parent directory could not be found",
                    path.display()
                ))?;
                if !parent.exists() {
                    fs::create_dir_all(&parent).await.context(format!(
                        "Unable to create a directory for '{}'",
                        path.display()
                    ))?;
                }
                parent
            }
            PathType::Dir => {
                fs::create_dir_all(&path)
                    .await
                    .context(format!("Unable to create directory '{}'", path.display()))?;
                path
            }
        }
    } else if !exists && in_project {
        // The path does not exist and we have not been asked to create it, but it is within the
        // project_dir which is going to be mounted anyway. Instead of producing an error in this
        // case, we should simply skip the mounting of the path.
        return Ok(());
    } else {
        // Either the path already exists or we were not asked to create it. Nothing to do.
        path
    };

    let mount_path = canonicalize(uncanonicalized_mount_path)?;
    mounts.push(Mount::new(mount_path));
    Ok(())
}

/// A list of environment variables that don't conform to naming convensions, but we need to pass
/// through to the `cargo make` invocation.
const ENV_VARS: [&str; 13] = [
    "ALLOW_MISSING_KEY",
    "AMI_DATA_FILE_SUFFIX",
    "BOOT_CONFIG",
    "BOOT_CONFIG_INPUT",
    "CARGO_MAKE_CARGO_ARGS",
    "CARGO_MAKE_DEFAULT_TESTSYS_KUBECONFIG_PATH",
    "CARGO_MAKE_TESTSYS_ARGS",
    "CARGO_MAKE_TESTSYS_KUBECONFIG_ARG",
    "MARK_OVA_AS_TEMPLATE",
    "RELEASE_START_TIME",
    "SSM_DATA_FILE_SUFFIX",
    "VMWARE_IMPORT_SPEC_PATH",
    "VMWARE_VM_NAME_DEFAULT",
];

fn is_build_system_env(key: impl AsRef<str>) -> bool {
    let key = key.as_ref();
    if key.starts_with("BOOT_CONFIG") {
        true
    } else if key.starts_with("BUILDSYS_") {
        true
    } else if key.starts_with("PUBLISH_") {
        true
    } else if key.starts_with("REPO_") {
        true
    } else if key.starts_with("TESTSYS_") {
        true
    } else {
        ENV_VARS.contains(&key)
    }
}

fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
    path.as_ref().canonicalize().context(format!(
        "Unable to canonicalize the path '{}'",
        path.as_ref().display(),
    ))
}

/// We have to search through the arguments for calls that look like this:
/// `cargo make testsys test -f /some/path`
fn find_testsys_test_path<I, S>(iter: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // let args: Vec<String> = iter.into_iter().collect();
    // args.fi
    let iter = iter.into_iter();
    let mut iter = iter.skip_while(|s| s.as_ref() != "testsys");
    // Advance iter to the next argument, which needs to be `test`, if we are extracting a file path
    iter.next();
    // if this argument is `test`, then we will continue to search for a file path.
    match iter.next() {
        Some(x) if x.as_ref() == "test" => {}
        _ => return None,
    }

    // Search for a file argument.
    let mut iter =
        iter.skip_while(|s| !s.as_ref().starts_with("-f") && !s.as_ref().starts_with("--file"));
    // TODO - this comment is wrong
    // Advance the argument to the file argument and extract that argument as a string. It might
    // contain the file path if the form -f=/the/path was used.
    let file_arg: String = match iter.next() {
        Some(s) => s.as_ref().to_string(),
        None => {
            // Impossible code path, but it's not an error even if it somehow happened.
            return None;
        }
    };

    // Check to see if equals was used, if so, parse the file path from the arg.
    if file_arg.starts_with("-f=") || file_arg.starts_with("--file=") {
        match file_arg.split("=").skip(1).next() {
            Some(s) if !s.is_empty() => return Some(PathBuf::from(s)),
            _ => {
                // It's weird, but it's none of our business. We will just report no path found.
                return None;
            }
        }
    }

    // An equals sign was not used, we expect the path in the next argument. If we didn't find it,
    // we return None as above.
    iter.next().map(|s| PathBuf::from(s.as_ref()))
}

#[test]
fn test_is_build_system_env() {
    assert!(is_build_system_env(
        "CARGO_MAKE_DEFAULT_TESTSYS_KUBECONFIG_PATH"
    ));
    assert!(is_build_system_env("BUILDSYS_PRETTY_NAME"));
    assert!(!is_build_system_env("PATH"));
    assert!(!is_build_system_env("HOME"));
}

#[test]
fn test_find_testsys_test_path_1() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "test",
        "--blah",
        "true",
        "-f",
        "/the/path",
    ];
    let path = find_testsys_test_path(args).unwrap();
    assert_eq!(path.display().to_string(), "/the/path");
}

#[test]
fn test_find_testsys_test_path_2() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "test",
        "--blah",
        "true",
        "--file",
        "/the/path",
    ];
    let path = find_testsys_test_path(args).unwrap();
    assert_eq!(path.display().to_string(), "/the/path");
}

#[test]
fn test_find_testsys_test_path_3() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "test",
        "--blah",
        "true",
        "-f=/the/path",
    ];
    let path = find_testsys_test_path(args).unwrap();
    assert_eq!(path.display().to_string(), "/the/path");
}

#[test]
fn test_find_testsys_test_path_4() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "test",
        "--blah",
        "true",
        "--file=/the/path",
    ];
    let path = find_testsys_test_path(args).unwrap();
    assert_eq!(path.display().to_string(), "/the/path");
}

#[test]
fn test_find_testsys_test_path_not_found_1() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "test",
        "--blah",
        "true",
        "-f=",
    ];
    assert!(find_testsys_test_path(args).is_none())
}

#[test]
fn test_find_testsys_test_path_not_found_2() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "test",
        "--blah",
        "true",
        "--file=",
    ];
    assert!(find_testsys_test_path(args).is_none())
}

#[test]
fn test_find_testsys_test_path_not_found_3() {
    let args = [
        "--foo",
        "--bar=baz",
        "testsys",
        "foo",
        "--blah",
        "true",
        "--file=/the/path",
    ];
    assert!(find_testsys_test_path(args).is_none())
}

#[test]
fn test_find_testsys_test_path_not_found_4() {
    let args = [
        "--foo",
        "--bar=baz",
        "build",
        "test",
        "--blah",
        "true",
        "--file=/the/path",
    ];
    assert!(find_testsys_test_path(args).is_none())
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
enum Target {
    Setup,
    SetupBuild,
    Fetch,
    FetchSdk,
    FetchToolchain,
    FetchSources,
    FetchVendored,
    UnitTests,
    Check,
    CheckFmt,
    CheckLints,
    CheckClippy,
    CheckShell,
    CheckGolangciLint,
    CheckMigrations,
    BuildTools,
    PublishSetupTools,
    InfraTools,
    PublishTools,
    BuildSbkeys,
    CheckCargoVersion,
    BootConfig,
    ValidateBootConfig,
    BuildPackage,
    BuildVariant,
    CheckLicenses,
    FetchLicenses,
    Build,
    Tuftool,
    CreateInfra,
    PublishSetup,
    PublishSetupWithoutKey,
    ValidateRepo,
    CheckRepoExpirations,
    RefreshRepo,
    Ami,
    AmiPublic,
    AmiPrivate,
    GrantAmi,
    RevokeAmi,
    ValidateAmi,
    Ssm,
    PromoteSsm,
    ValidateSsm,
    UploadOvaBase,
    UploadOva,
    VmwareTemplate,
    Clean,
    CleanSources,
    CleanPackages,
    CleanImages,
    CleanRepos,
    CleanState,
    PurgeCache,
    PurgeGoVendor,
    PurgeCargo,
    TestTools,
    SetupTest,
    Test,
    CleanTest,
    ResetTest,
    UninstallTest,
    PurgeTest,
    WatchTest,
    WatchTestAll,
    LogTest,
    Testsys,
    Default,
}

derive_display_from_serialize!(Target);
derive_fromstr_from_deserialize!(Target);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct PathVar {
    name: &'static str,
    r#type: PathType,
    create: bool,
}

impl PathVar {
    /// Shorthand constructor to get (most of) the constants on one line each.
    const fn new(name: &'static str, r#type: PathType, create: bool) -> Self {
        Self {
            name,
            r#type,
            create,
        }
    }
}

const BOOT_CONFIG: PathVar = PathVar::new("BOOT_CONFIG", FILE, NO_CREATE);
const BOOT_CONFIG_INPUT: PathVar = PathVar::new("BOOT_CONFIG_INPUT", DIR, NO_CREATE);
const BUILDSYS_BUILD_DIR: PathVar = PathVar::new("BUILDSYS_BUILD_DIR", DIR, CREATE);
const BUILDSYS_IMAGES_DIR: PathVar = PathVar::new("BUILDSYS_IMAGES_DIR", DIR, CREATE);
const BUILDSYS_KMOD_KIT_PATH: PathVar = PathVar::new("BUILDSYS_KMOD_KIT_PATH", DIR, CREATE);
const BUILDSYS_LICENSES_CONFIG_PATH: PathVar =
    PathVar::new("BUILDSYS_LICENSES_CONFIG_PATH", FILE, NO_CREATE);
const BUILDSYS_OUTPUT_DIR: PathVar = PathVar::new("BUILDSYS_OUTPUT_DIR", DIR, CREATE);
const BUILDSYS_OVA_PATH: PathVar = PathVar::new("BUILDSYS_OVA_PATH", FILE, CREATE);
const BUILDSYS_OVF_TEMPLATE: PathVar = PathVar::new("BUILDSYS_OVF_TEMPLATE", FILE, NO_CREATE);
const BUILDSYS_PACKAGES_DIR: PathVar = PathVar::new("BUILDSYS_PACKAGES_DIR", DIR, CREATE);
const BUILDSYS_ROOT_DIR: PathVar = PathVar::new("BUILDSYS_ROOT_DIR", DIR, NO_CREATE);
const BUILDSYS_SBKEYS_PROFILE_DIR: PathVar =
    PathVar::new("BUILDSYS_SBKEYS_PROFILE_DIR", DIR, CREATE);
const BUILDSYS_SOURCES_DIR: PathVar = PathVar::new("BUILDSYS_SOURCES_DIR", DIR, NO_CREATE);
const BUILDSYS_STATE_DIR: PathVar = PathVar::new("BUILDSYS_STATE_DIR", DIR, CREATE);
const BUILDSYS_TOOLS_DIR: PathVar = PathVar::new("BUILDSYS_TOOLS_DIR", DIR, NO_CREATE);
const BUILDSYS_VARIANT_DIR: PathVar = PathVar::new("BUILDSYS_VARIANT_DIR", DIR, CREATE);
const CARGO_HOME: PathVar = PathVar::new("CARGO_HOME", DIR, CREATE);
const GO_MODULES: PathVar = PathVar::new("GO_MODULES", DIR, NO_CREATE);
const GO_MOD_CACHE: PathVar = PathVar::new("GO_MOD_CACHE", DIR, CREATE);
const PUBLISH_EXPIRATION_POLICY_PATH: PathVar =
    PathVar::new("PUBLISH_EXPIRATION_POLICY_PATH", FILE, NO_CREATE);
const PUBLISH_INFRA_CONFIG_PATH: PathVar = PathVar::new("PUBLISH_INFRA_CONFIG_PATH", FILE, CREATE);
const PUBLISH_REPO_BASE_DIR: PathVar = PathVar::new("PUBLISH_REPO_BASE_DIR", DIR, CREATE);
const PUBLISH_REPO_KEY: PathVar = PathVar::new("PUBLISH_REPO_KEY", FILE, CREATE);
const PUBLISH_REPO_OUTPUT_DIR: PathVar = PathVar::new("PUBLISH_REPO_OUTPUT_DIR", FILE, CREATE);
const PUBLISH_REPO_ROOT_JSON: PathVar = PathVar::new("PUBLISH_REPO_ROOT_JSON", FILE, CREATE);
const PUBLISH_SSM_TEMPLATES_PATH: PathVar =
    PathVar::new("PUBLISH_SSM_TEMPLATES_PATH", FILE, CREATE);
const TESTSYS_KUBECONFIG: PathVar = PathVar::new("TESTSYS_KUBECONFIG", FILE, CREATE);
const TESTSYS_MGMT_CLUSTER_KUBECONFIG: PathVar =
    PathVar::new("TESTSYS_MGMT_CLUSTER_KUBECONFIG", FILE, CREATE);
const TESTSYS_TESTS_DIR: PathVar = PathVar::new("TESTSYS_TESTS_DIR", DIR, CREATE);
const TESTSYS_TEST_CONFIG_PATH: PathVar = PathVar::new("TESTSYS_TEST_CONFIG_PATH", FILE, NO_CREATE);
const TESTSYS_USERDATA: PathVar = PathVar::new("TESTSYS_USERDATA", FILE, CREATE);
const VMWARE_IMPORT_SPEC_PATH: PathVar = PathVar::new("VMWARE_IMPORT_SPEC_PATH", FILE, NO_CREATE);

const BUILDSYS_PATHS: &[PathVar] = &[
    BOOT_CONFIG,
    BOOT_CONFIG_INPUT,
    BUILDSYS_SBKEYS_PROFILE_DIR,
    BUILDSYS_BUILD_DIR,
    BUILDSYS_IMAGES_DIR,
    BUILDSYS_KMOD_KIT_PATH,
    BUILDSYS_LICENSES_CONFIG_PATH,
    BUILDSYS_OUTPUT_DIR,
    BUILDSYS_STATE_DIR,
    BUILDSYS_TOOLS_DIR,
    BUILDSYS_VARIANT_DIR,
    BUILDSYS_VARIANT_DIR,
    CARGO_HOME,
    GO_MOD_CACHE,
    GO_MODULES,
    BUILDSYS_SBKEYS_PROFILE_DIR,
];

const DOCKER_RUN_PATHS: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_ROOT_DIR,
    BUILDSYS_SOURCES_DIR,
    BUILDSYS_TOOLS_DIR,
    BUILDSYS_SOURCES_DIR,
    GO_MOD_CACHE,
    GO_MODULES,
];

const RUST_TOOLS_PATHS: &[PathVar] = &[CARGO_HOME, BUILDSYS_TOOLS_DIR];

const PUBLISH_PATHS: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_TOOLS_DIR,
    PUBLISH_EXPIRATION_POLICY_PATH,
    PUBLISH_INFRA_CONFIG_PATH,
    PUBLISH_REPO_ROOT_JSON,
    PUBLISH_REPO_KEY,
    PUBLISH_REPO_BASE_DIR,
    PUBLISH_REPO_OUTPUT_DIR,
];

const PUBLISH_REFRESH_REPO_PATHS: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_TOOLS_DIR,
    PUBLISH_EXPIRATION_POLICY_PATH,
    PUBLISH_INFRA_CONFIG_PATH,
    PUBLISH_REPO_ROOT_JSON,
    PUBLISH_REPO_KEY,
    PUBLISH_REPO_BASE_DIR,
    PUBLISH_REPO_OUTPUT_DIR,
    PUBLISH_EXPIRATION_POLICY_PATH,
];

const PUBLISH_AMI_PATHS: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_ROOT_DIR,
    BUILDSYS_TOOLS_DIR,
    BUILDSYS_VARIANT_DIR,
    PUBLISH_INFRA_CONFIG_PATH,
];

const PUBLISH_SSM_PATH: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_ROOT_DIR,
    BUILDSYS_TOOLS_DIR,
    BUILDSYS_VARIANT_DIR,
    PUBLISH_INFRA_CONFIG_PATH,
    PUBLISH_SSM_TEMPLATES_PATH,
];

const VMWARE_PATHS: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_VARIANT_DIR,
    BUILDSYS_OVA_PATH,
    BUILDSYS_OVF_TEMPLATE,
    PUBLISH_INFRA_CONFIG_PATH,
    VMWARE_IMPORT_SPEC_PATH,
];

const TESTSYS_PATHS: &[PathVar] = &[
    CARGO_HOME,
    BUILDSYS_ROOT_DIR,
    BUILDSYS_TOOLS_DIR,
    TESTSYS_KUBECONFIG,
    TESTSYS_MGMT_CLUSTER_KUBECONFIG,
    TESTSYS_TESTS_DIR,
    TESTSYS_TEST_CONFIG_PATH,
    TESTSYS_USERDATA,
    VMWARE_IMPORT_SPEC_PATH,
];

impl Target {
    fn paths(&self) -> &'static [PathVar] {
        match self {
            Target::Setup => &[
                BUILDSYS_BUILD_DIR,
                BUILDSYS_OUTPUT_DIR,
                BUILDSYS_PACKAGES_DIR,
                BUILDSYS_STATE_DIR,
                GO_MOD_CACHE,
            ],
            Target::SetupBuild => &[],
            Target::Fetch => &[],
            Target::FetchSdk => &[],
            Target::FetchToolchain => &[],
            Target::FetchSources => &[],
            Target::FetchVendored => DOCKER_RUN_PATHS,
            Target::UnitTests => DOCKER_RUN_PATHS,
            Target::Check => &[],
            Target::CheckFmt => DOCKER_RUN_PATHS,
            Target::CheckLints => &[],
            Target::CheckClippy => DOCKER_RUN_PATHS,
            Target::CheckShell => &[BUILDSYS_TOOLS_DIR],
            Target::CheckGolangciLint => &[GO_MOD_CACHE, GO_MODULES],
            Target::CheckMigrations => &[BUILDSYS_ROOT_DIR, BUILDSYS_SOURCES_DIR],
            Target::BuildTools => RUST_TOOLS_PATHS,
            Target::PublishSetupTools => RUST_TOOLS_PATHS,
            Target::InfraTools => RUST_TOOLS_PATHS,
            Target::PublishTools => RUST_TOOLS_PATHS,
            Target::BuildSbkeys => RUST_TOOLS_PATHS,
            Target::CheckCargoVersion => &[CARGO_HOME],
            Target::BootConfig => &[BOOT_CONFIG_INPUT, BOOT_CONFIG, GO_MOD_CACHE],
            Target::ValidateBootConfig => &[BOOT_CONFIG_INPUT, BOOT_CONFIG, GO_MOD_CACHE],
            Target::BuildPackage => BUILDSYS_PATHS,
            Target::BuildVariant => BUILDSYS_PATHS,
            Target::CheckLicenses => DOCKER_RUN_PATHS,
            Target::FetchLicenses => DOCKER_RUN_PATHS,
            Target::Build => &[],
            Target::Tuftool => RUST_TOOLS_PATHS,
            Target::CreateInfra => RUST_TOOLS_PATHS,
            Target::PublishSetup => PUBLISH_PATHS,
            Target::PublishSetupWithoutKey => PUBLISH_PATHS,
            Target::ValidateRepo => PUBLISH_PATHS,
            Target::CheckRepoExpirations => PUBLISH_PATHS,
            Target::RefreshRepo => PUBLISH_REFRESH_REPO_PATHS,
            Target::Ami => PUBLISH_AMI_PATHS,
            Target::AmiPublic => PUBLISH_AMI_PATHS,
            Target::AmiPrivate => PUBLISH_AMI_PATHS,
            Target::GrantAmi => PUBLISH_AMI_PATHS,
            Target::RevokeAmi => PUBLISH_AMI_PATHS,
            Target::ValidateAmi => PUBLISH_AMI_PATHS,
            Target::Ssm => PUBLISH_SSM_PATH,
            Target::PromoteSsm => PUBLISH_SSM_PATH,
            Target::ValidateSsm => PUBLISH_SSM_PATH,
            Target::UploadOvaBase => VMWARE_PATHS,
            Target::UploadOva => VMWARE_PATHS,
            Target::VmwareTemplate => VMWARE_PATHS,
            Target::Clean => &[],
            Target::CleanSources => &[BUILDSYS_TOOLS_DIR],
            Target::CleanPackages => &[BUILDSYS_PACKAGES_DIR],
            Target::CleanImages => &[BUILDSYS_IMAGES_DIR],
            Target::CleanRepos => &[PUBLISH_REPO_BASE_DIR],
            Target::CleanState => &[BUILDSYS_STATE_DIR],
            Target::PurgeCache => &[],
            Target::PurgeGoVendor => &[GO_MOD_CACHE],
            Target::PurgeCargo => &[CARGO_HOME],
            Target::TestTools => TESTSYS_PATHS,
            Target::SetupTest => TESTSYS_PATHS,
            Target::Test => TESTSYS_PATHS,
            Target::CleanTest => TESTSYS_PATHS,
            Target::ResetTest => TESTSYS_PATHS,
            Target::UninstallTest => TESTSYS_PATHS,
            Target::PurgeTest => TESTSYS_PATHS,
            Target::WatchTest => TESTSYS_PATHS,
            Target::WatchTestAll => TESTSYS_PATHS,
            Target::LogTest => TESTSYS_PATHS,
            Target::Testsys => TESTSYS_PATHS,
            Target::Default => &[],
        }
    }
}
