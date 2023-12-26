/*!

This directory and module are for tests, test data, and re-usable test code. This module should only
be compiled for `cfg(test)`, which is accomplished at its declaration in `main.rs`.

!*/
mod cargo_make;
mod twoliter_make;
use anyhow::{Context, Result};
use fs_extra::dir;
use fs_extra::dir::CopyOptions;
use std::path::{Path, PathBuf};
use tokio::runtime::Handle;

/// Return the canonical path to the directory where we store test data.
pub(crate) fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("test")
        .join("data")
        .canonicalize()
        .unwrap()
}

pub(crate) fn projects_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.join("tests").join("projects").canonicalize().unwrap()
}

fn test_projects_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.join("tests").join("projects").canonicalize().unwrap()
}

async fn copy_dir(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref().to_path_buf();
    let to = to.as_ref().to_path_buf();
    let rt = Handle::current();
    let fut = rt.spawn_blocking(move || {
        dir::copy(
            &from,
            &to,
            &CopyOptions {
                overwrite: true,
                skip_exist: false,
                ..Default::default()
            },
        )
        .context(format!(
            "Unable to copy directory from {} to {}",
            from.display(),
            to.display()
        ))
    });
    let _ = fut
        .await
        .context("Unable to join future when copy directory")?;
    Ok(())
}
