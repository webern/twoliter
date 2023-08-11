use bytes::BufMut;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{env, fs};

const DATA_INPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/embedded");
const TOOLS_HASH_RS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tools_hash.rs");

fn main() {
    println!("cargo:rerun-if-changed={}", DATA_INPUT_DIR);
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

    // TODO - use Makefile.toml when we have imported the Bottlerocket monorepo git history.
    let makefile_source = data_input_dir.join("Makefile.temp.toml");
    copy_file(makefile_source, tools_dir.join("Makefile.toml"));

    // Create tarball
    let mut buf_writer = Vec::new().writer();
    let enc = GzEncoder::new(&mut buf_writer, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all("", &tools_dir).unwrap();
    
    // Drop tar object to ensure any finalizing steps are done.
    drop(tar);
    
    let tar_data = buf_writer.get_ref();

    // Create a hash of the in-memory tarball to be used when installing tools.
    let mut hasher = Sha256::new();
    hasher.update(tar_data);
    let hashed = hasher.finalize();
    let hash = format!("{:02X}", hashed).to_ascii_lowercase();
    
    // Write the tarball to the OUT_DIR where it can be imported during the build.
    fs::write(&tar_path, tar_data)
        .expect(&format!("Unable to write to file '{}'", tar_path.display()));
    // Write the tarball hash to a constant in a generated Rust filethat can be used during install.
    let tools_hash_rs_path = PathBuf::from(TOOLS_HASH_RS_PATH);
    let tools_hash_rs_content = format!(
        "/*! Generated file. !*/\n\npub(crate) const TOOLS_HASH: &str = \"{}\";",
        hash
    );
    fs::write(&tools_hash_rs_path, tools_hash_rs_content).unwrap();
}

// Copy a file and provide a useful error message if it fails.
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
