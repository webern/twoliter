use guppy::PackageId;
use snafu::Snafu;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub(super) enum Error {
    #[snafu(display("Failed to read cargo_metadata file '{}': {}", path.display(), source))]
    CargoMetadataRead { path: PathBuf, source: io::Error },

    #[snafu(display("Failed to parse cargo_metadata json from '{}': {}", path.display(), source))]
    CargoMetadataParse { path: PathBuf, source: guppy::Error },

    #[snafu(display("Cargo package graph query failed with root '{id}': {source}"))]
    CargoPackageQuerySnafu { id: PackageId, source: guppy::Error },

    #[snafu(display("Package '{id}' has no 'vendor' field in build-kit metadata"))]
    NoKitVendor { id: String },

    #[snafu(display("Failed to create dependency graph from '{}': {}", path.display(), source))]
    GraphBuild { path: PathBuf, source: guppy::Error },

    #[snafu(display("Failed to read manifest file '{}': {}", path.display(), source))]
    ManifestFileRead { path: PathBuf, source: io::Error },

    #[snafu(display("Failed to load manifest file '{}': {}", path.display(), source))]
    ManifestFileLoad {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[snafu(display("Failed to read external kit metadata file '{}': {}", path.display(), source))]
    ExternalKitMetadataFileRead { path: PathBuf, source: io::Error },

    #[snafu(display("Failed to load external kit metadata file '{}': {}", path.display(), source))]
    ExternalKitMetadataLoad {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[snafu(display("Failed to parse image feature '{}'", what))]
    ParseImageFeature { what: String },

    #[snafu(display(
        "The cargo package we are building, '{name}', could not be found in the graph"
    ))]
    RootDependencyMissing { name: String },
}
