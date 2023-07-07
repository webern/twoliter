use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const EMBED_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/embed");

fn main() {
    // checkout_bottlerocket();
    let embed_dir = PathBuf::from(EMBED_DIR);
    let embed_dir = embed_dir
        .canonicalize()
        .expect(&format!("Unable to canonicalize '{}'", embed_dir.display()));

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let tools_dir = out_dir.join("tools");
    fs::create_dir_all(&tools_dir).unwrap();
    let tar_path = out_dir.join("tools.tar.gz");

    // TODO - move this to the Twoliter git repo
    // Copy Bottlerocket Makefile.toml
    fs::copy(
        embed_dir.join("Makefile.toml"),
        tools_dir.join("Makefile.toml"),
    )
    .unwrap();

    // Create tarball
    let tar_gz = File::create(&tar_path).unwrap();
    let enc = GzEncoder::new(&tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all("", &tools_dir).unwrap();
}

// /// This is a temporary function that will be removed once Bottlerocket tools are checked-in to this
// /// repository.
// fn checkout_bottlerocket() {
//     fs::create_dir_all(".ignore/hack").unwrap();
//     if PathBuf::from(".ignore/hack/bottlerocket/.git").exists() {
//         return;
//     }
//
//     let status = Command::new("git")
//         .arg("clone")
//         .arg("git@github.com:webern/bottlerocket.git")
//         .current_dir(".ignore/hack")
//         .status()
//         .unwrap();
//
//     if !status.success() {
//         panic!("git clone command failed.")
//     }
//
//     let status = Command::new("git")
//         .arg("remote")
//         .arg("add")
//         .arg("upstream")
//         .arg("git@github.com:bottlerocket-os/bottlerocket.git")
//         .current_dir(".ignore/hack/bottlerocket")
//         .status()
//         .unwrap();
//
//     if !status.success() {
//         panic!("git remote add command failed.")
//     }
//
//     let status = Command::new("git")
//         .arg("fetch")
//         .arg("origin")
//         .arg("twoliter-spike")
//         .current_dir(".ignore/hack/bottlerocket")
//         .status()
//         .unwrap();
//
//     if !status.success() {
//         panic!("git fetch command failed.")
//     }
//
//     let status = Command::new("git")
//         .arg("checkout")
//         .arg("twoliter-spike")
//         .current_dir(".ignore/hack/bottlerocket")
//         .status()
//         .unwrap();
//
//     if !status.success() {
//         panic!("git checkout command failed.")
//     }
// }
