mod build;
mod exec;

use crate::cmd::build::BuildCommand;
use anyhow::Result;
use clap::Parser;
use env_logger::Builder;
use exec::Exec;
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
    #[clap(subcommand)]
    Build(BuildCommand),

    Exec(Exec),
}

/// Entrypoint for the `twoliter` command line program.
pub(super) async fn run(args: Args) -> Result<()> {
    match args.subcommand {
        Subcommand::Build(build_command) => build_command.run().await,
        Subcommand::Exec(exec_command) => exec_command.run().await,
    }
}

/// Initialize the logger with the value passed by `--log-level` (or its default) when the
/// `RUST_LOG` environment variable is not present. If present, the `RUST_LOG` environment variable
/// overrides `--log-level`/`level`.
pub(super) fn init_logger(level: LevelFilter) {
    match std::env::var(env_logger::DEFAULT_FILTER_ENV).ok() {
        Some(_) => {
            // RUST_LOG exists; env_logger will use it.
            Builder::from_default_env().init();
        }
        None => {
            // RUST_LOG does not exist; use provided log level for this crate only.
            Builder::new()
                .filter(Some(env!("CARGO_CRATE_NAME")), level)
                .init();
        }
    }
}
