use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{env, fs};

const DATA_INPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/embedded");

fn main() {
    let data_input_dir = PathBuf::from(DATA_INPUT_DIR);
    let data_input_dir = data_input_dir.canonicalize().expect(&format!(
        "Unable to canonicalize '{}'",
        data_input_dir.display()
    ));

    // This is the directory that cargo creates for us so that we can pass things from the build
    // script to the main compilation phase.
    let out_dir =
        PathBuf::from(env::var("OUT_DIR").expect("The cargo variable 'OUT_DIR' is missing"));

    // This is where we will copy all of the things we want to add to our tarball. We will then
    // compress and tar this directory.
    let tools_dir = out_dir.join("tools");
    fs::create_dir_all(&tools_dir).expect(&format!(
        "Unable to create directory '{}'",
        tools_dir.display()
    ));

    // This is the filepath to the tarball we will create.
    let tar_path = out_dir.join("tools.tar.gz");

    // TODO - name this Makefile.toml when we have ported the Bottlerocket monorepo git history.
    let makefile_source = data_input_dir.join("Makefile.temp.toml");
    copy_file(makefile_source, tools_dir.join("Makefile.toml"));
    // Extract Makefile task dependency graph.

    // Create tarball
    let tar_gz =
        File::create(&tar_path).expect(&format!("Unable to create file '{}'", tar_path.display()));
    let enc = GzEncoder::new(&tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all("", &tools_dir).unwrap();
}

fn copy_file<P1, P2>(source: P1, dest: P2)
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let source = source.as_ref();
    let dest = dest.as_ref();
    fs::copy(source, dest).expect(&format!(
        "Unable to copy `{}' to '{}'",
        source.display(),
        dest.display()
    ));
}
