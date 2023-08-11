use crate::project;
use crate::tools::install_tools;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

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

/// Install items that Twoliter needs on the build host.
#[derive(Debug, Parser)]
pub(crate) struct InstallTools {
    /// Path to the project file. Will search for Twoliter.toml when absent.
    #[clap(long)]
    project_path: Option<PathBuf>,

    /// Install anyway, even if we detect that the right version is already installed.
    #[clap(long)]
    force: bool,
}

impl InstallTools {
    pub(super) async fn run(&self) -> Result<()> {
        let project = project::load_or_find_project(self.project_path.clone()).await?;
        install_tools(project.project_dir(), self.force).await?;
        Ok(())
    }
}
