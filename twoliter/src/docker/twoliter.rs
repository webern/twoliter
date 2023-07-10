use crate::common::fs;
use crate::docker::{DockerBuild, ImageArchUri, ImageUri};
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::path::Path;
use tar::Archive;
use tempfile::TempDir;

const CONTEXT: &str = "context";
const FILES: &str = "files";
const DOCKERFILE: &str = "Twoliter.dockerfile";
const DOCKERFILE_CONTENTS: &str = include_str!("Twoliter.dockerfile");
const TARBALL_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tools.tar.gz"));

/// Creates the container needed for twoliter to use as its build environment.
pub(crate) async fn create_twoliter_image_if_not_exists(sdk: &ImageArchUri) -> Result<ImageUri> {
    // TODO - exit early if exists https://github.com/bottlerocket-os/twoliter/issues/12
    let temp_dir = TempDir::new()
        .context("Unable to create a temporary directory for twoliter image creation")?;

    let (dockerfile, context) = prepare_dir(&temp_dir)
        .await
        .context("Unable to prepare directory for building the twoliter image")?;

    // TODO - correctly tag https://github.com/bottlerocket-os/twoliter/issues/12
    let image_uri = ImageUri::new(None, "twoliter", "latest");

    DockerBuild::default()
        .dockerfile(dockerfile)
        .context_dir(context)
        .build_arg("BASE", sdk.uri())
        .tag(image_uri.clone())
        .execute()
        .await
        .context("Unable to build the twoliter container")?;

    Ok(image_uri)
}

async fn unpack_tarball(path: impl AsRef<Path>, out_dir: impl AsRef<Path>) -> Result<()> {
    fs::write(&path, TARBALL_DATA).await?;
    let tar_file = fs::open_file(&path).await?.into_std().await;
    let tar = GzDecoder::new(tar_file);
    let mut archive = Archive::new(tar);
    archive.unpack(&out_dir).context(format!(
        "Unable to unpack tarball '{}' into directory '{}'",
        path.as_ref().display(),
        out_dir.as_ref().display()
    ))?;
    // Make sure nothing is holding this open before deleting the file.
    drop(archive);
    fs::remove_file(path).await?;
    Ok(())
}

/// Prepare a directory for the `docker build` command that will create the twoliter container.
/// Returns the path to the dockerfile and the path to the context directory as
/// `(dockerfile, context)`.
async fn prepare_dir(dir: impl AsRef<Path>) -> Result<(impl AsRef<Path>, impl AsRef<Path>)> {
    let dir = dir.as_ref();
    let context = dir.join(CONTEXT);
    let files = context.join(FILES);
    fs::create_dir_all(&files).await?;

    // Write out the embedded dockerfile.
    let dockerfile = dir.join(DOCKERFILE);
    fs::write(&dockerfile, DOCKERFILE_CONTENTS).await?;

    // Write out the embedded tools and scripts.
    unpack_tarball(dir.join("tools.tar.gz"), &files).await?;
    Ok((dockerfile, context))
}

#[tokio::test]
async fn test_prepare_dir() {
    let temp_dir = TempDir::new().unwrap();
    let (dockerfile, context) = prepare_dir(&temp_dir).await.unwrap();
    assert!(dockerfile.as_ref().is_file());
    assert!(context.as_ref().is_dir());
    assert!(context.as_ref().join(FILES).join("Makefile.toml").is_file())
}
