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

/// Prepare a directory for the `docker build` command that will create the twoliter container.
/// Returns the path to the dockerfile and the path to the context directory as
/// `(dockerfile, context)`.
pub(crate) async fn prepare_dir(
    dir: impl AsRef<Path>,
) -> Result<(impl AsRef<Path>, impl AsRef<Path>)> {
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
