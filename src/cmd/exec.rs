use crate::docker::{DockerRun, Mount};
use crate::{docker, project};
use anyhow::Result;
use clap::Parser;
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
            .command_arg(project_dir.display().to_string())
            .command_arg("--verbose")
            .env("TWOLITER_PROJECT_DIR", project_dir.display().to_string());
        for cargo_make_arg in &self.cargo_make_args {
            docker_command = docker_command.command_arg(cargo_make_arg);
        }
        docker_command
            .command_arg("-e")
            .command_arg(format!("BUILDSYS_ROOT_DIR={}", project_dir.display()))
            .execute()
            .await?;
        Ok(())
    }
}
