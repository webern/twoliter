use anyhow::{Context, Result};
use flate2::read::ZlibDecoder;
use log::debug;
use std::path::Path;
use tar::Archive;
use tempfile::TempDir;

const TAR_GZ_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tools.tar.gz"));

pub(crate) async fn install_tools() -> Result<TempDir> {
    let tempdir = TempDir::new().context("Unable to create a tempdir for Twoliter's tools")?;
    let tools_dir = tempdir.path();
    debug!("Installing tools to '{}'", tools_dir.display());

    // Write out the embedded tools and scripts.
    unpack_tarball(&tools_dir)
        .await
        .context("Unable to install tools")?;

    Ok(tempdir)
}

async fn unpack_tarball(tools_dir: impl AsRef<Path>) -> Result<()> {
    let tools_dir = tools_dir.as_ref();
    let tar = ZlibDecoder::new(TAR_GZ_DATA);
    let mut archive = Archive::new(tar);
    archive.unpack(tools_dir).context(format!(
        "Unable to unpack tarball into directory '{}'",
        tools_dir.display()
    ))?;
    debug!("Installed tools to '{}'", tools_dir.display());
    Ok(())
}

#[tokio::test]
async fn test_install_tools() {
    let tempdir = install_tools().await.unwrap();

    // Assert that the expected files exist in the tools directory.

    // Check that non-binary files were copied.
    assert!(tempdir.path().join("Dockerfile").is_file());
    assert!(tempdir.path().join("Makefile.toml").is_file());
    assert!(tempdir.path().join("docker-go").is_file());
    assert!(tempdir.path().join("partyplanner").is_file());
    assert!(tempdir.path().join("rpm2img").is_file());
    assert!(tempdir.path().join("rpm2kmodkit").is_file());
    assert!(tempdir.path().join("rpm2migrations").is_file());

    // Check that binaries were copied.
    assert!(tempdir.path().join("bottlerocket-variant").is_file());
    assert!(tempdir.path().join("buildsys").is_file());
    assert!(tempdir.path().join("pubsys").is_file());
    assert!(tempdir.path().join("pubsys-setup").is_file());
    assert!(tempdir.path().join("testsys").is_file());
    assert!(tempdir.path().join("tuftool").is_file());
}
