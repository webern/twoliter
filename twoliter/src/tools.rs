use crate::common::fs;
use crate::tools_hash::TOOLS_HASH;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use log::{debug, trace, warn};
use std::path::Path;
use tar::Archive;

const TARBALL_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tools.tar.gz"));

pub(crate) async fn install_tools(tools_dir: impl AsRef<Path>, force: bool) -> Result<()> {
    let tools_dir = tools_dir.as_ref();
    let sentinel_filepath = tools_dir.join("install");
    if !force && !should_install(&sentinel_filepath).await {
        debug!("Not installing tools because hashes matched");
        return Ok(());
    }

    trace!("Installing tools to '{}'", tools_dir.display());

    fs::create_dir_all(&tools_dir)
        .await
        .context("Unable to create a directory for Twoliter's tools")?;

    // Write out the embedded tools and scripts.
    unpack_tarball(&tools_dir)
        .await
        .context("Unable to install tools")?;

    // Write out a file that can be used to detect what version of the tools has been installed.
    let installed = tools_dir.join("installed");
    fs::write(&installed, &TOOLS_HASH).await.context(format!(
        "Unable to write the tools hash to '{}'",
        installed.display(),
    ))?;

    Ok(())
}

async fn should_install(sentinel_filepath: &Path) -> bool {
    if !sentinel_filepath.is_file() {
        trace!(
            "Installing because this file was not found '{}",
            sentinel_filepath.display()
        );
        return true;
    }
    let installed = match fs::read_to_string(&sentinel_filepath).await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "Unable to read file '{}', installing tools anyway: {}",
                sentinel_filepath.display(),
                e
            );
            return true;
        }
    };
    trace!("installed hash is '{}'", installed);
    let do_install = installed != TOOLS_HASH;
    if do_install {
        debug!("installed '{}', our hash '{}'", installed, TOOLS_HASH)
    }
    do_install
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
