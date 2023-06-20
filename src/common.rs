pub(crate) const DEFAULT_ARCH: &str = "x86_64";
use anyhow::{ensure, Context, Result};
use tokio::process::Command;

/// Run a `tokio::process::Command` and return a `Result` letting us know whether or not it worked.
pub(crate) async fn exec(cmd: &mut Command) -> Result<()> {
    let output = cmd
        .output()
        .await
        .context(format!("Unable to start command '{:?}'", cmd))?;

    ensure!(
        output.status.success(),
        "Command '{:?}' was unsuccessful:\n{}\n{}",
        cmd,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
