use crate::common::fs;
use crate::project;
use anyhow::{Context, Result};
use clap::Parser;
use flate2::read::GzDecoder;
use log::debug;
use std::path::{Path, PathBuf};
use tar::Archive;

const TARBALL_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tools.tar.gz"));

pub(crate) async fn install_tools(tools_dir: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(&tools_dir)
        .await
        .context("Unable to create a directory for Twoliter's tools")?;

    // Write out the embedded tools and scripts.
    unpack_tarball(&tools_dir)
        .await
        .context("Unable to install tools")?;

    Ok(())
}

async fn unpack_tarball(tools_dir: impl AsRef<Path>) -> Result<()> {
    // TODO - check and return if already installed.
    let tools_dir = tools_dir.as_ref();
    let tar = GzDecoder::new(TARBALL_DATA);
    let mut archive = Archive::new(tar);
    archive.unpack(&tools_dir).context(format!(
        "Unable to unpack tarball into directory '{}'",
        tools_dir.display()
    ))?;
    debug!("Installed tools to '{}'", tools_dir.display());
    Ok(())
}

#[tokio::test]
async fn test_prepare_dir() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let (dockerfile, context) = prepare_dir(&temp_dir).await.unwrap();
    assert!(dockerfile.as_ref().is_file());
    assert!(context.as_ref().is_dir());
    assert!(context.as_ref().join(FILES).join("Makefile.toml").is_file())
}
