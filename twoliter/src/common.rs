use anyhow::{ensure, Context, Result};
use log::{self, debug, LevelFilter};
use tokio::process::Command;

/// Run a `tokio::process::Command` and return a `Result` letting us know whether or not it worked.
pub(crate) async fn exec(cmd: &mut Command) -> Result<()> {
    debug!("Running: {:?}", cmd);

    match log::max_level() {
        // For quiet levels of logging we capture stdout and stderr
        LevelFilter::Off | LevelFilter::Error | LevelFilter::Warn => {
            let output = cmd
                .output()
                .await
                .context(format!("Unable to start command '{:?}'", cmd))?;
            ensure!(
                output.status.success(),
                "Command '{:?}' was unsuccessful, exit code {}:\n{}\n{}",
                cmd,
                output.status.code().unwrap_or(1),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // For less quiet log levels we stream to stdout and stderr.
        LevelFilter::Info | LevelFilter::Debug | LevelFilter::Trace => {
            let status = cmd
                .status()
                .await
                .context(format!("Unable to start command '{:?}'", cmd))?;

            ensure!(
                status.success(),
                "Command '{:?}' was unsuccessful, exit code {:?}",
                cmd,
                status.code().unwrap_or(1),
            );
        }
    }
    Ok(())
}

/// Wrappers around tokio::fs commands to add context to the error messages.
pub(crate) mod fs {
    use anyhow::{Context, Result};
    use std::path::Path;

    pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
        tokio::fs::write(path.as_ref(), contents)
            .await
            .context(format!("Unable to write to '{}'", path.as_ref().display()))
    }

    pub async fn create_dir_all(path: impl AsRef<Path>) -> Result<()> {
        tokio::fs::create_dir_all(path.as_ref())
            .await
            .context(format!(
                "Unable to create directory '{}'",
                path.as_ref().display()
            ))
    }

    pub async fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
        tokio::fs::read_to_string(&path)
            .await
            .context(format!("Unable to read file '{}'", path.as_ref().display()))
    }
}
