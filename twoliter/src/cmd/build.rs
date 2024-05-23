use super::build_clean::BuildClean;
use crate::cargo_make::CargoMake;
use crate::common::fs;
use crate::docker::DockerContainer;
use crate::project;
use crate::project::Project;
use crate::tools::install_tools;
use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, error};
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::runtime::Handle;

#[derive(Debug, Parser)]
pub(crate) enum BuildCommand {
    Clean(BuildClean),
    Variant(BuildVariant),
}

impl BuildCommand {
    pub(crate) async fn run(self) -> Result<()> {
        match self {
            BuildCommand::Clean(command) => command.run().await,
            BuildCommand::Variant(command) => command.run().await,
        }
    }
}

/// Build a Bottlerocket variant image.
#[derive(Debug, Parser)]
pub(crate) struct BuildVariant {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long = "project-path")]
    project_path: Option<PathBuf>,

    /// The architecture to build for.
    #[clap(long = "arch", default_value = "x86_64")]
    arch: String,

    /// The variant to build.
    variant: String,

    /// The URL to the lookaside cache where sources are stored to avoid pulling them from upstream.
    /// Defaults to https://cache.bottlerocket.aws
    lookaside_cache: Option<String>,

    /// If sources are not found in the lookaside cache, this flag will cause buildsys to pull them
    /// from the upstream URL found in a package's `Cargo.toml`.
    #[clap(long = "upstream-source-fallback")]
    upstream_source_fallback: bool,
}

impl BuildVariant {
    pub(super) async fn run(&self) -> Result<()> {
        let project = project::load_or_find_project(self.project_path.clone()).await?;
        let toolsdir = project.tools_dir();
        install_tools(&toolsdir).await?;
        let makefile_path = toolsdir.join("Makefile.toml");

        let mut alpha_sdk = AlphaSdk::new(&project).await?;
        alpha_sdk.copy_rpms(&project).await?;
        Self::ensure_sbkeys(&project, Some(&alpha_sdk)).await?;
        Self::ensure_models_dir(&project, Some(&mut alpha_sdk)).await?;

        let mut optional_envs = Vec::new();
        if let Some(lookaside_cache) = &self.lookaside_cache {
            optional_envs.push(("BUILDSYS_LOOKASIDE_CACHE", lookaside_cache))
        }

        // Hold the result of the cargo make call so we can clean up the project directory first.
        let res = CargoMake::new(&project)?
            .env("TWOLITER_TOOLS_DIR", toolsdir.display().to_string())
            .env("BUILDSYS_ARCH", &self.arch)
            .env("BUILDSYS_VARIANT", &self.variant)
            .env("BUILDSYS_VERSION_IMAGE", project.release_version())
            .env("GO_MODULES", project.find_go_modules().await?.join(" "))
            .env(
                "BUILDSYS_UPSTREAM_SOURCE_FALLBACK",
                self.upstream_source_fallback.to_string(),
            )
            .envs(optional_envs.into_iter())
            .makefile(makefile_path)
            .project_dir(project.project_dir())
            .exec("build")
            .await;

        res
    }

    /// If the Alpha SDK is being used (i.e. for a Build Variant command), then copy out actual
    /// `generate-sbkeys-script` from the Alpha SDK. Otherwise, just create an empty file there.
    async fn ensure_sbkeys(project: &Project, alpha_sdk: Option<&AlphaSdk>) -> Result<()> {
        let sbkeys_dir = project.project_dir().join("sbkeys");
        if !sbkeys_dir.is_dir() {
            // Create a sbkeys directory in the main project
            debug!("sbkeys dir not found. Creating a temporary directory");
            fs::create_dir_all(&sbkeys_dir).await?;
            if let Some(alpha_sdk) = alpha_sdk {
                alpha_sdk.copy_sbkeys_script(&sbkeys_dir).await?;
            } else {
                let script = sbkeys_dir.join("generate-local-keys");
                fs::write(&script, "# intentionally empty").await?;
                fs::set_permissions(&script, Permissions::from_mode(0o755)).await?;
            }
        }
        Ok(())
    }

    async fn ensure_models_dir(project: &Project, alpha_sdk: Option<&mut AlphaSdk>) -> Result<()> {
        // TODO: Remove once models is no longer conditionally compiled.
        // Create the models directory for the sdk to mount
        let models_dir = project.project_dir().join("sources/models");
        if !models_dir.is_dir() {
            debug!("models source dir not found. Creating a temporary directory");
            fs::create_dir_all(&models_dir.join("src/variant"))
                .await
                .context("Unable to create models source directory")?;
            if let Some(alpha_sdk) = alpha_sdk {
                alpha_sdk.created_files.push(models_dir)
            }
        }
        Ok(())
    }
}

struct AlphaSdk {
    container: DockerContainer,
    temp_packages_dir: PathBuf,
    created_files: Vec<PathBuf>,
}

impl AlphaSdk {
    async fn new(project: &Project) -> Result<Self> {
        let container = DockerContainer::new(
            format!("sdk-{}", project.token()),
            project
                .sdk()
                .context(format!(
                    "No SDK defined in {}",
                    project.filepath().display(),
                ))?
                .uri(),
        )
        .await?;

        let temp_packages_dir = project.temp_dir().join("sdk_rpms");
        fs::create_dir_all(&temp_packages_dir).await?;

        Ok(Self {
            container,
            temp_packages_dir,
            created_files: vec![],
        })
    }

    async fn copy_rpms(&mut self, project: &Project) -> Result<()> {
        self.container
            .cp_out(
                Path::new("twoliter/alpha/build/rpms"),
                &self.temp_packages_dir,
            )
            .await?;

        let rpms_dir = project.rpms_dir();
        fs::create_dir_all(&rpms_dir).await?;
        debug!("Moving rpms to build dir");
        let temp_rpms_dir = self.temp_packages_dir.join("rpms");

        // TODO - this breaks with per_package dirs
        let mut read_dir = tokio::fs::read_dir(&temp_rpms_dir)
            .await
            .context(format!("Unable to read dir '{}'", temp_rpms_dir.display()))?;
        while let Some(entry) = read_dir.next_entry().await.context(format!(
            "Error while reading entries in dir '{}'",
            temp_rpms_dir.display()
        ))? {
            debug!("Moving '{}'", entry.path().display());
            fs::rename(entry.path(), rpms_dir.join(entry.file_name())).await?;
        }

        Ok(())
    }

    async fn copy_sbkeys_script(&self, sbkeys_dir: &Path) -> Result<()> {
        self.container
            .cp_out(
                Path::new("twoliter/alpha/sbkeys/generate-local-sbkeys"),
                &sbkeys_dir,
            )
            .await
            .context("Unable to copy the sbkeys script from the Alpha SDK")
    }

    async fn cleanup(objects: Vec<PathBuf>) -> Result<()> {
        for file_name in objects {
            let added = Path::new(&file_name);
            if added.is_file() {
                fs::remove_file(added).await?;
            } else if added.is_dir() {
                fs::remove_dir_all(added).await?;
            }
        }
        Ok(())
    }
}

impl Drop for AlphaSdk {
    fn drop(&mut self) {
        let handle = Handle::current();
        let to_delete = self.created_files.clone();
        let thread = std::thread::spawn(move || {
            // Using Handle::block_on to run async code in the new thread.
            handle.block_on(async {
                match Self::cleanup(to_delete).await {
                    Ok(_) => (),
                    // Note: Debug is the correct way to pretty-print anyhow::Error.
                    Err(e) => error!("Unable to delete Alpha SDK temp objects: {:?}", e),
                }
            });
        });

        // Ignore the error type from thread.join() which is unusable and means that a panic has
        // occurred.
        let _ = thread.join();
    }
}
