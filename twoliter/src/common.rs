use anyhow::{ensure, Context, Result};
use log::{self, debug, LevelFilter};
use tokio::process::Command;

/// Run a `tokio::process::Command` and return a `Result` letting us know whether or not it worked.
pub(crate) async fn exec(cmd: &mut Command) -> Result<()> {
    debug!("Running: {:?}", cmd);

    match log::max_level() {
        // For non-debugging levels of logging we capture stdout and stderr
        LevelFilter::Off | LevelFilter::Error | LevelFilter::Warn | LevelFilter::Info => {
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

        // For debugging we stream to stdout and stderr.
        LevelFilter::Debug | LevelFilter::Trace => {
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

    pub async fn remove_file(path: impl AsRef<Path>) -> Result<()> {
        tokio::fs::remove_file(path.as_ref()).await.context(format!(
            "Unable to remove file '{}'",
            path.as_ref().display()
        ))
    }

    pub async fn open_file(path: impl AsRef<Path>) -> Result<tokio::fs::File> {
        tokio::fs::File::open(path.as_ref())
            .await
            .context(format!("Unable to open file '{}", path.as_ref().display()))
    }
}
