use crate::docker::{DockerBuild, ImageArchUri, ImageUri};
use crate::embed::TOOLS_BINARY_DATA;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::File;
use tar::Archive;
use tempfile::TempDir;
use tokio::fs;

const TWOLITER_DOCKERFILE: &str = include_str!("Twoliter.dockerfile");

/// Creates the container needed for twoliter to use as its build environment.
pub(crate) async fn create_twoliter_image_if_not_exists(sdk: &ImageArchUri) -> Result<ImageUri> {
    // TODO - exit early if exists https://github.com/bottlerocket-os/twoliter/issues/12
    let temp_dir = TempDir::new()
        .context("Unable to create a temporary directory for Twoliter image creation")?;
    let context_dir = temp_dir.path().join("context");
    let tools_tar_gz = context_dir.join("tools.tar.gz");
    fs::create_dir_all(&context_dir).await.context(format!(
        "Unable to create directory '{}'",
        context_dir.display()
    ))?;

    // Write the tarball.
    fs::write(&tools_tar_gz, TOOLS_BINARY_DATA).await.unwrap();

    // Unpack the contents.
    let tar_gz = File::open(&tools_tar_gz).unwrap();
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(&context_dir).unwrap();
    drop(archive);
    fs::remove_file(&tools_tar_gz).await.unwrap();

    let dockerfile_path = temp_dir.path().join("Twoliter.dockerfile");
    fs::write(&dockerfile_path, TWOLITER_DOCKERFILE)
        .await
        .context(format!(
            "Unable to write to '{}'",
            dockerfile_path.display()
        ))?;

    // TODO - correctly tag https://github.com/bottlerocket-os/twoliter/issues/12
    let image_uri = ImageUri::new(None, "twoliter", "latest");

    DockerBuild::default()
        .dockerfile(dockerfile_path)
        .context_dir(context_dir)
        .build_arg("BASE", sdk.uri())
        .tag(image_uri.clone())
        .execute()
        .await
        .context("Unable to build the twoliter container")?;

    Ok(image_uri)
}

#[cfg(test)]
mod test {
    use crate::embed::TOOLS_BINARY_DATA;
    use flate2::read::GzDecoder;
    use std::fs;
    use std::fs::File;
    use std::process::Command;
    use tar::Archive;
    use tempfile::TempDir;

    /// In this test we unpack the embedded tarball and check that the artifacts we extect are there
    /// in good order.
    #[test]
    fn test_the_correctness_of_tools_tar_gz() {
        let temp_dir = TempDir::new().unwrap();
        let tools_tar_gz = temp_dir.path().join("tools.tar.gz");

        // Write the tarball.
        fs::write(&tools_tar_gz, TOOLS_BINARY_DATA).unwrap();

        // Unpack the contents.
        let tar_gz = File::open(&tools_tar_gz).unwrap();
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path()).unwrap();

        // Add assertions here to make sure we have packaged the right things.
        assert_eq!(
            1,
            Command::new("./buildsys")
                .arg("--version")
                .current_dir(temp_dir.path())
                .status()
                .unwrap()
                .code()
                .unwrap()
        );
    }
}
