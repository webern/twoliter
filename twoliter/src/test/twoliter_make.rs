use crate::cmd::make::Make;
use crate::test::{copy_dir, test_projects_dir};
use tempfile::TempDir;

#[tokio::test]
async fn twoliter_make_variant() {
    let project_source_dir = test_projects_dir().join("bottlerocket-like");
    let tmp = TempDir::new().unwrap();
    let test_root = tmp.path();
    let project_dir = test_root.join("bottlerocket-like");
    let cargo_home = test_root.join("cargo_home");
    copy_dir(&project_source_dir, &test_root).await.unwrap();
    let cmd = Make {
        project_path: Some(project_dir.join("Twoliter.toml")),
        cargo_home: cargo_home.clone(),
        makefile_task: "build-variant".to_string(),
        additional_args: vec![],
        arch: "x86_64".to_string(),
    };
    let _ = cmd.run().await.unwrap();
}
