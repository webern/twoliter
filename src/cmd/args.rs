use crate::docker;
use crate::docker::default_sdk;
use anyhow::Result;
use clap::Parser;
use log::LevelFilter;

/// A tool for building custom variants of Bottlerocket.
#[derive(Debug, Parser)]
#[clap(about, long_about = None)]
pub(crate) struct Args {
    #[clap(long = "log-level", default_value = "warn")]
    pub(crate) log_level: LevelFilter,

    #[clap(subcommand)]
    pub(crate) subcommand: Subcommand,
}

#[derive(Debug, Parser)]
pub(crate) enum Subcommand {
    /// Build something, such as a Bottlerocket image or a kit of packages.
    #[clap(subcommand)]
    Build(BuildCommand),
}

#[derive(Debug, Parser)]
pub(crate) enum BuildCommand {
    Variant(BuildVariant),
}

impl BuildCommand {
    pub(crate) async fn run(self) -> Result<()> {
        match self {
            BuildCommand::Variant(build_variant) => build_variant.run().await,
        }
    }
}

/// Build a Bottlerocket variant image.
#[derive(Debug, Parser)]
pub(crate) struct BuildVariant {}

impl BuildVariant {
    pub(super) async fn run(&self) -> Result<()> {
        let _ = docker::create_twoliter_image_if_not_exists(&default_sdk()).await?;
        Ok(())
    }
}
