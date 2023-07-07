use crate::docker::{DockerRun, Mount};
use crate::{docker, project};
use anyhow::Result;
use clap::Parser;
use log::{debug, trace};
use std::collections::HashSet;
use std::path::PathBuf;

/// Run a cargo make command in Twoliter's build environment.
#[derive(Debug, Parser)]
pub(crate) struct Exec {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long = "project-path")]
    project_path: Option<PathBuf>,

    /// Path to the docker daemon socket.
    #[clap(long = "docker-socket", default_value = "/var/run/docker.sock")]
    docker_socket: String,

    /// Arguments to be passed to cargo make
    cargo_make_args: Vec<String>,
}

impl Exec {
    pub(super) async fn run(&self) -> Result<()> {
        let (project, path) = project::load_or_find_project(self.project_path.clone()).await?;
        let project_dir = path.parent().unwrap().canonicalize().unwrap();
        // TODO - get smart about sdk: https://github.com/bottlerocket-os/twoliter/issues/11
        let sdk = project.sdk.clone().unwrap_or_default();
        // TODO - peek at cargo make args to see if we can figure out what the arch is (so we don't
        // pull two SDK containers). The arch for Twoliter execution doesn't matter.
        let image = docker::create_twoliter_image_if_not_exists(&sdk.uri("x86_64")).await?;
        let project_mount = Mount {
            source: project_dir.clone(),
            destination: project_dir.clone(),
            ..Default::default()
        };
        let socket_mount = Mount {
            source: PathBuf::from(self.docker_socket.clone()),
            destination: PathBuf::from("/var/run/docker.sock"),
            ..Default::default()
        };
        let mut docker_command = DockerRun::new(image)
            .remove()
            .name("twoliter-exec")
            .mount(project_mount)
            .mount(socket_mount)
            .user(nix::unistd::Uid::effective().to_string())
            .workdir(project_dir.display().to_string())
            .command_arg("cargo")
            .command_arg("make")
            .command_arg("--disable-check-for-updates")
            .command_arg("--makefile")
            .command_arg("/local/Makefile.toml")
            .command_arg("--cwd")
            .command_arg(project_dir.display().to_string());

        // TODO - this can panic if non-unicode env
        let makefile_vars = env_vars();
        for (key, val) in std::env::vars() {
            if makefile_vars.contains(key.as_str()) {
                debug!("Passing env var {} to cargo make", key);
                docker_command = docker_command
                    .command_arg("-e".to_string())
                    .command_arg(format!("{}={}", key, val));
            } else {
                debug!("Not passing env var {} to cargo make", key);
            }
        }

        docker_command = docker_command
            .command_arg("-e")
            .command_arg(format!("BUILDSYS_ROOT_DIR={}", project_dir.display()));

        // These have to go last because the last of these might be the Makefile.toml target.
        for cargo_make_arg in &self.cargo_make_args {
            docker_command = docker_command.command_arg(cargo_make_arg);
        }
        docker_command.execute().await?;
        Ok(())
    }
}

const ENV_VARS: [&str; 66] = [
    "AMI_DATA_FILE_SUFFIX",
    "BOOT_CONFIG",
    "BOOT_CONFIG_INPUT",
    "BUILDSYS_ALLOW_FAILED_LICENSE_CHECK",
    "BUILDSYS_ARCH",
    "BUILDSYS_BUILD_DIR",
    "BUILDSYS_IMAGES_DIR",
    "BUILDSYS_JOBS",
    "BUILDSYS_KMOD_KIT",
    "BUILDSYS_KMOD_KIT_PATH",
    "BUILDSYS_LICENSES_CONFIG_PATH",
    "BUILDSYS_NAME",
    "BUILDSYS_NAME_FRIENDLY",
    "BUILDSYS_NAME_FULL",
    "BUILDSYS_NAME_VARIANT",
    "BUILDSYS_NAME_VERSION",
    "BUILDSYS_OUTPUT_DIR",
    "BUILDSYS_OVA",
    "BUILDSYS_OVA_PATH",
    "BUILDSYS_OVF_TEMPLATE",
    "BUILDSYS_PACKAGES_DIR",
    "BUILDSYS_PRETTY_NAME",
    "BUILDSYS_REGISTRY",
    "BUILDSYS_RELEASE_CONFIG_PATH",
    "BUILDSYS_ROOT_DIR",
    "BUILDSYS_SDK_IMAGE",
    "BUILDSYS_SDK_NAME",
    "BUILDSYS_SDK_VERSION",
    "BUILDSYS_SOURCES_DIR",
    "BUILDSYS_STATE_DIR",
    "BUILDSYS_TIMESTAMP",
    "BUILDSYS_TOOLCHAIN",
    "BUILDSYS_TOOLS_DIR",
    "BUILDSYS_UPSTREAM_LICENSE_FETCH",
    "BUILDSYS_UPSTREAM_SOURCE_FALLBACK",
    "BUILDSYS_VARIANT",
    "BUILDSYS_VARIANT_DIR",
    "BUILDSYS_VERSION_BUILD",
    "BUILDSYS_VERSION_FULL",
    "BUILDSYS_VERSION_IMAGE",
    "CARGO_MAKE_CARGO_ARGS",
    "CARGO_MAKE_DEFAULT_TESTSYS_KUBECONFIG_PATH",
    "CARGO_MAKE_TESTSYS_ARGS",
    "CARGO_MAKE_TESTSYS_KUBECONFIG_ARG",
    "PUBLISH_AMI_NAME_DEFAULT",
    "PUBLISH_EXPIRATION_POLICY_PATH",
    "PUBLISH_INFRA_CONFIG_PATH",
    "PUBLISH_REPO",
    "PUBLISH_REPO_BASE_DIR",
    "PUBLISH_REPO_KEY",
    "PUBLISH_REPO_OUTPUT_DIR",
    "PUBLISH_REPO_ROOT_JSON",
    "PUBLISH_SSM_TEMPLATES_PATH",
    "PUBLISH_TUFTOOL_VERSION",
    "PUBLISH_WAVE_POLICY_PATH",
    "REPO_METADATA_EXPIRING_WITHIN",
    "REPO_VALIDATE_TARGETS",
    "SSM_DATA_FILE_SUFFIX",
    "TESTSYS_STARTING_COMMIT",
    "TESTSYS_STARTING_VERSION",
    "TESTSYS_TEST",
    "TESTSYS_TEST_CONFIG_PATH",
    "TESTSYS_TEST_CONFIG_PATH",
    "TESTSYS_TESTS_DIR",
    "VMWARE_IMPORT_SPEC_PATH",
    "VMWARE_VM_NAME_DEFAULT",
];

fn env_vars() -> HashSet<&'static str> {
    ENV_VARS.into_iter().collect()
}
