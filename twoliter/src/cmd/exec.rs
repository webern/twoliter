use crate::common::fs;
use crate::docker::{DockerRun, Mount};
use crate::{docker, project};
use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, trace};
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

    /// A path used by buildsys.
    #[clap(long)]
    buildsys_build_dir: Option<PathBuf>,

    /// A path used by buildsys.
    #[clap(long)]
    buildsys_images_dir: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_repo_output_dir: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_repo_root_json: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_ssm_templates: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_wave_policy: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_expiration_policy: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_infra_config: Option<PathBuf>,

    /// A path used by pubsys.
    #[clap(long)]
    pubsys_repo_key: Option<PathBuf>,

    /// A path used by testsys.
    #[clap(long)]
    testsys_default_kubeconfig: Option<PathBuf>,

    /// A path used by testsys.
    #[clap(long)]
    testsys_kubeconfig: Option<PathBuf>,

    /// A path used by testsys.
    #[clap(long)]
    testsys_test_config: Option<PathBuf>,

    /// A path used by testsys.
    #[clap(long)]
    testsys_tests_dir: Option<PathBuf>,

    /// A path used by the cargo makefile.
    #[clap(long)]
    vmware_import_spec: Option<PathBuf>,

    /// Additional paths to mount into the container. If any additional paths need to be mounted,
    /// use this argument to do so.
    #[clap(long = "mount")]
    mounts: Vec<PathBuf>,

    /// Cargo make target. E.g. the word "build" if we want to execute `cargo make build`.
    makefile_target: String,

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

        // Shorthand to fit these calls into one line for readability.
        let m = &mut mounts;
        let p = project_dir;
        add(m, &p, &self.buildsys_build_dir, DIR, CREATE).await?;
        add(m, &p, &self.buildsys_images_dir, DIR, CREATE).await?;
        add(m, &p, &self.pubsys_expiration_policy, FILE, NO_CREATE).await?;
        add(m, &p, &self.pubsys_infra_config, FILE, CREATE).await?;
        add(m, &p, &self.pubsys_repo_key, FILE, CREATE).await?;
        add(m, &p, &self.pubsys_repo_output_dir, DIR, CREATE).await?;
        add(m, &p, &self.pubsys_repo_root_json, FILE, CREATE).await?;
        add(m, &p, &self.pubsys_ssm_templates, FILE, NO_CREATE).await?;
        add(m, &p, &self.pubsys_wave_policy, FILE, NO_CREATE).await?;
        add(m, &p, &self.testsys_default_kubeconfig, FILE, NO_CREATE).await?;
        add(m, &p, &self.testsys_kubeconfig, FILE, CREATE).await?;
        add(m, &p, &self.testsys_test_config, FILE, NO_CREATE).await?;
        add(m, &p, &self.testsys_tests_dir, DIR, CREATE).await?;
        add(m, &p, &self.vmware_import_spec, FILE, NO_CREATE).await?;

        for path in &self.mounts {
            let path = canonicalize(path)?;
            mounts.push(Mount::new(path));
        }

        if let Some(testsys_test_path) = find_testsys_test_path(env::args()) {
            let testsys_test_path = canonicalize(testsys_test_path)?;
            mounts.push(Mount::new(testsys_test_path));
        }

        Ok(mounts)
    }
}

#[derive(Debug, Clone, Copy)]
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
    path: &Option<PathBuf>,
    path_type: PathType,
    create: bool,
) -> Result<()> {
    // Nothing to do if we weren't asked to mount anything.
    let path = match path {
        Some(p) => p,
        None => return Ok(()),
    };
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
