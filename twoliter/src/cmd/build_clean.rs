use crate::cargo_make::CargoMake;
use crate::project;
use crate::tools;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub(crate) struct BuildClean {
    /// Path to Twoliter.toml. Will search for Twoliter.toml when absent.
    #[clap(long = "project-path")]
    project_path: Option<PathBuf>,
}

impl BuildClean {
    pub(super) async fn run(&self) -> Result<()> {
        let project = project::load_or_find_project(self.project_path.clone()).await?;
        let toolsdir = project.tools_dir();
        tools::install_tools(&toolsdir).await?;

        CargoMake::new(&project)?
            .env("TWOLITER_TOOLS_DIR", toolsdir.display().to_string())
            .makefile(project.makefile())
            .project_dir(project.project_dir())
            .exec("clean")
            .await?;

        Ok(())
    }
}
