use crate::common::exec;
use crate::docker::ImageUri;
use anyhow::Result;
use serde::Serialize;
use serde_plain::derive_display_from_serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Execute a `docker build` command. This follows the builder pattern, for example:
///
/// ```
/// let build = DockerBuild::default().dockerfile("./Dockerfile").context(".").execute().await?;
/// ```
#[derive(Debug, Clone)]
pub(crate) struct DockerBuild {
    dockerfile: Option<PathBuf>,
    context_dir: PathBuf,
    tag: Option<ImageUri>,
    build_args: HashMap<String, String>,
}

impl Default for DockerBuild {
    fn default() -> Self {
        Self {
            dockerfile: None,
            context_dir: PathBuf::from("."),
            tag: None,
            build_args: Default::default(),
        }
    }
}

impl DockerBuild {
    /// Add a value for the `--file` argument.
    pub(crate) fn dockerfile<P: AsRef<Path>>(mut self, dockerfile: P) -> Self {
        self.dockerfile = Some(dockerfile.as_ref().into());
        self
    }

    /// Required: the directory to be passed to the build as the context.
    pub(crate) fn context_dir<P: AsRef<Path>>(mut self, context_dir: P) -> Self {
        self.context_dir = context_dir.as_ref().into();
        self
    }

    /// Add a value for the `--tag` argument.
    pub(crate) fn tag<T: Into<ImageUri>>(mut self, tag: T) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Add a build arg, where `("KEY", value)` becomes `--build-arg=KEY=value`.
    pub(crate) fn build_arg<S1, S2>(mut self, key: S1, value: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        self.build_args.insert(key.into(), value.into());
        self
    }

    /// Add multiple build args, where `("KEY", value)` becomes `--build-arg=KEY=value`.
    pub(crate) fn _build_args<I: IntoIterator<Item = (String, String)>>(
        mut self,
        build_args: I,
    ) -> Self {
        self.build_args.extend(build_args.into_iter());
        self
    }

    /// Run the `docker build` command.
    pub(crate) async fn execut(self) -> Result<()> {
        let mut args = vec!["build".to_string()];
        if let Some(dockerfile) = self.dockerfile.as_ref() {
            args.push("--file".to_string());
            args.push(dockerfile.display().to_string());
        }
        if let Some(tag) = self.tag.as_ref() {
            args.push("--tag".to_string());
            args.push(tag._uri());
        }
        args.extend(
            self.build_args
                .iter()
                .map(|(k, v)| format!("--build-arg={}={}", k, v)),
        );
        args.push(self.context_dir.display().to_string());
        exec(
            Command::new("docker")
                .args(args.into_iter())
                .env("DOCKER_BUILDKIT", "1"),
        )
        .await
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, PartialEq, Hash, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MountType {
    Bind,
    _Volume,
    _Tmpfs,
}

derive_display_from_serialize!(MountType);

impl Default for MountType {
    fn default() -> Self {
        Self::Bind
    }
}

/// The value of a `--mount` argument.
#[derive(Debug, Default, Clone, Ord, PartialOrd, PartialEq, Hash, Eq)]
pub(crate) struct _Mount {
    pub(crate) _type: MountType,
    pub(crate) _source: PathBuf,
    pub(crate) _destination: PathBuf,
    pub(crate) _read_only: bool,
}

impl _Mount {
    /// Create a new `Mount` object, of type `Bind`, with the same external and internal path.
    /// For example, `Mount::new('/foo')` will create `type=bind,source=/foo,target=foo`.
    pub(crate) fn _new(path: impl AsRef<Path>) -> Self {
        Self {
            _type: MountType::Bind,
            _source: path.as_ref().into(),
            _destination: path.as_ref().into(),
            _read_only: false,
        }
    }

    /// Express the mount as the value of `docker run` argument, e.g.
    /// `type=bind,source=/foo,target=foo`.
    fn _as_arg(&self) -> String {
        let mut s = format!(
            "type={},source={},target={}",
            self._type,
            self._source.display(),
            self._destination.display()
        );
        if self._read_only {
            s.push_str(",readonly");
        }
        s
    }
}

