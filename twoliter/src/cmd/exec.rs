use crate::docker::{DockerRun, Mount};
use crate::{docker, project};
use anyhow::Result;
use clap::Parser;
use log::{debug, trace};
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
            .mount(project_mount)
            .mount(socket_mount)
            .mount(tmp_mount)
            .user(nix::unistd::Uid::effective().to_string())
            .workdir(project_dir.display().to_string())
            .command_arg("cargo")
            .command_arg("make")
            .command_arg("--disable-check-for-updates")
            .command_arg("--makefile")
            .command_arg("/twoliter/tools/Makefile.toml")
            .command_arg("--cwd")
            .command_arg(project_dir.display().to_string())
            ._env("CARGO_LOG", "cargo::core::compiler::fingerprint=info");

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
        for cargo_make_arg in &self.cargo_make_args {
            docker_command = docker_command.command_arg(cargo_make_arg);
        }
        docker_command.execute().await?;
        Ok(())
    }
}

const ENV_VARS: [&str; 12] = [
    "ALLOW_MISSING_KEY",
    "AMI_DATA_FILE_SUFFIX",
    "BOOT_CONFIG",
    "BOOT_CONFIG_INPUT",
    "CARGO_MAKE_CARGO_ARGS",
    "CARGO_MAKE_DEFAULT_TESTSYS_KUBECONFIG_PATH",
    "CARGO_MAKE_TESTSYS_ARGS",
    "CARGO_MAKE_TESTSYS_KUBECONFIG_ARG",
    "MARK_OVA_AS_TEMPLATE",
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

#[test]
fn test_is_build_system_env() {
    assert!(is_build_system_env(
        "CARGO_MAKE_DEFAULT_TESTSYS_KUBECONFIG_PATH"
    ));
    assert!(is_build_system_env("BUILDSYS_PRETTY_NAME"));
    assert!(!is_build_system_env("PATH"));
    assert!(!is_build_system_env("HOME"));
}
