use crate::common::exec;
use crate::docker::twoliter::prepare_dir;
use crate::project;
use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, trace};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::process::Command;

/// Run a cargo make command in Twoliter's build environment. Certain environment variable paths
/// from Makefile.toml are taken here as explicit arguments so that the caller can decide which of
/// these configurable paths may need to be mounted by Twoliter.
#[derive(Debug, Parser)]
pub(crate) struct Exec {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long)]
    project_path: Option<PathBuf>,

    /// It is required to pass this instead of using `CARGO_HOME` so that there can be no confusion
    /// between the `CARGO_HOME` that is intended for the build, and the user's default
    /// `CARGO_HOME`.
    #[clap(long)]
    cargo_home: PathBuf,

    /// Cargo make task. E.g. the word "build" if we want to execute `cargo make build`.
    makefile_task: String,

    /// Arguments to be passed to cargo make
    additional_args: Vec<String>,
}

impl Exec {
    pub(super) async fn run(&self) -> Result<()> {
        let (_project, path) = project::load_or_find_project(self.project_path.clone()).await?;
        let project_dir = path.parent().context(format!(
            "Unable to find the parent directory containing project file '{}'",
            path.display()
        ))?;
        // TODO - get smart about sdk: https://github.com/bottlerocket-os/twoliter/issues/11
        // let sdk = project.sdk.clone().unwrap_or_default();
        // TODO - peek at cargo make args to see if we can figure out what the arch is (so we don't
        // pull two SDK containers). The arch for Twoliter execution doesn't matter.
        // let image = docker::create_twoliter_image_if_not_exists(&sdk.uri("x86_64")).await?;

        // Write the makefile to a tempdir.
        // TODO - we should use a stable dir for this instead of unpacking it every time.
        let temp_dir = TempDir::new().context("Unable to create tempdir for Makefile.toml")?;
        let (_, context) = prepare_dir(&temp_dir).await?;
        let makefile = context.as_ref().join("files").join("Makefile.toml");

        let mut args = vec![
            "make".to_string(),
            "--disable-check-for-updates".to_string(),
            "--makefile".to_string(),
            makefile.display().to_string(),
            "--cwd".to_string(),
            project_dir.display().to_string(),
        ];

        for (key, val) in std::env::vars() {
            if is_build_system_env(key.as_str()) {
                debug!("Passing env var {} to cargo make", key);
                args.push("-e".to_string());
                args.push(format!("{}={}", key, val));
            } else {
                trace!("Not passing env var {} to cargo make", key);
            }
        }

        args.push("-e".to_string());
        args.push(format!("CARGO_HOME={}", self.cargo_home.display()));
        args.push(self.makefile_task.clone());

        // These have to go last because the last of these might be the Makefile.toml target.
        for cargo_make_arg in &self.additional_args {
            args.push(cargo_make_arg.clone());
        }

        exec(Command::new("cargo").args(args)).await?;
        Ok(())
    }
}

/// A list of environment variables that don't conform to naming conventions, but we need to pass
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

#[test]
fn test_is_build_system_env() {
    assert!(is_build_system_env(
        "CARGO_MAKE_DEFAULT_TESTSYS_KUBECONFIG_PATH"
    ));
    assert!(is_build_system_env("BUILDSYS_PRETTY_NAME"));
    assert!(!is_build_system_env("PATH"));
    assert!(!is_build_system_env("HOME"));
}
