use crate::common::fs;
use crate::project;
use crate::tools::install_tools;
use anyhow::{ensure, Context, Result};
use clap::Parser;
use log::debug;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;

/// Build a Bottlerocket kit.
#[derive(Debug, Parser)]
pub(crate) struct BuildKit {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long = "project-path")]
    project_path: Option<PathBuf>,

    /// The architecture to build for.
    #[clap(long = "arch", default_value = "x86_64")]
    arch: String,

    /// The kit to build.
    name: String,

    /// The URL to the lookaside cache where sources are stored to avoid pulling them from upstream.
    /// Defaults to https://cache.bottlerocket.aws
    lookaside_cache: Option<String>,
    // /// If sources are not found in the lookaside cache, this flag will cause buildsys to pull them
    // /// from the upstream URL found in a package's `Cargo.toml`.
    // #[clap(long = "upstream-source-fallback")]
    // upstream_source_fallback: bool,
}

impl BuildKit {
    pub(super) async fn run(&self) -> Result<()> {
        let project = project::load_or_find_project(self.project_path.clone()).await?;
        // let token = project.token();
        let toolsdir = project.project_dir().join("build/tools");
        install_tools(&toolsdir).await?;
        // let makefile_path = toolsdir.join("Makefile.toml");

        let mut created_files = Vec::new();

        // TODO: Remove once models is no longer conditionally compiled.
        // Create the models directory for the sdk to mount
        let models_dir = project.project_dir().join("sources/models");
        if !models_dir.is_dir() {
            debug!("models source dir not found. Creating a temporary directory");
            fs::create_dir_all(&models_dir.join("src/variant"))
                .await
                .context("Unable to create models source directory")?;
            created_files.push(models_dir)
        }

        let kits_cargo_toml = project
            .project_dir()
            .join("kits")
            .join(&self.name)
            .join("Cargo.toml");

        let mut envs = HashMap::new();
        envs.insert(
            "BUILDSYS_ROOT_DIR",
            project.project_dir().display().to_string(),
        );

        envs.insert("TLPRIVATE_SDK_IMAGE", project.sdk().unwrap().to_string());
        envs.insert("TWOLITER_TOOLS_DIR", toolsdir.display().to_string());
        envs.insert("BUILDSYS_ARCH", self.arch.to_string());
        // envs.insert("BUILDSYS_SBKEYS_DIR", sbkeys_dir.display().to_string())
        envs.insert(
            "BUILDSYS_VERSION_IMAGE",
            String::from(project.release_version()),
        );

        // TODO - get rid of this requirement
        envs.insert("BUILDSYS_VARIANT", "hello-ootb".to_string());
        envs.insert(
            "BUILDSYS_SOURCES_DIR",
            project.project_dir().join("sources").display().to_string(),
        );

        envs.insert(
            "BUILDSYS_PACKAGES_DIR",
            project.project_dir().join("packages").display().to_string(),
        );

        // TODO - eliminate this
        envs.insert(
            "BUILDSYS_VARIANT_PLATFORM",
            project.project_dir().join("foo").display().to_string(),
        );
        // TODO - eliminate this
        envs.insert(
            "BUILDSYS_VARIANT_RUNTIME",
            project.project_dir().join("foo").display().to_string(),
        );
        // TODO - eliminate this
        envs.insert(
            "BUILDSYS_VARIANT_FAMILY",
            project.project_dir().join("foo").display().to_string(),
        );
        // TODO - eliminate this
        envs.insert(
            "BUILDSYS_VARIANT_FLAVOR",
            project.project_dir().join("foo").display().to_string(),
        );

        // TODO - eliminate this
        envs.insert(
            "PUBLISH_REPO",
            project.project_dir().join("packages").display().to_string(),
        );

        // TODO - eliminate this
        envs.insert(
            "BUILDSYS_SDK_IMAGE",
            project.project_dir().join("packages").display().to_string(),
        );

        // TODO - eliminate this
        envs.insert(
            "BUILDSYS_TOOLCHAIN",
            project.project_dir().join("packages").display().to_string(),
        );

        let status = Command::new("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(kits_cargo_toml.display().to_string())
            .envs(envs)
            .status()
            .await
            .unwrap();

        ensure!(status.success(), "Failed with status {}", status);
        Ok(())
    }
}
