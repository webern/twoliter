use crate::common::fs;
use crate::project;
use crate::tools::install_tools;
use anyhow::{Context, Result};
use clap::Parser;
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use tar::Archive;

const TARBALL_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tools.tar.gz"));

#[derive(Debug, Parser)]
pub(crate) enum InstallCommand {
    Tools(InstallTools),
}

impl InstallCommand {
    pub(crate) async fn run(self) -> Result<()> {
        match self {
            InstallCommand::Tools(install_tools) => install_tools.run().await,
        }
    }
}

/// Build a Bottlerocket variant image.
#[derive(Debug, Parser)]
pub(crate) struct InstallTools {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long)]
    project_path: Option<PathBuf>,
}

impl InstallTools {
    pub(super) async fn run(&self) -> Result<()> {
        let project = project::load_or_find_project(self.project_path.clone()).await?;
        install_tools(project.project_dir()).await?;
        Ok(())
    }
}
