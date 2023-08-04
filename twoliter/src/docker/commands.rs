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
    pub(crate) fn _dockerfile<P: AsRef<Path>>(mut self, dockerfile: P) -> Self {
        self.dockerfile = Some(dockerfile.as_ref().into());
        self
    }

    /// Required: the directory to be passed to the build as the context.
    pub(crate) fn _context_dir<P: AsRef<Path>>(mut self, context_dir: P) -> Self {
        self.context_dir = context_dir.as_ref().into();
        self
    }

    /// Add a value for the `--tag` argument.
    pub(crate) fn _tag<T: Into<ImageUri>>(mut self, tag: T) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Add a build arg, where `("KEY", value)` becomes `--build-arg=KEY=value`.
    pub(crate) fn _build_arg<S1, S2>(mut self, key: S1, value: S2) -> Self
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
    pub(crate) async fn _execute(self) -> Result<()> {
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

/// Execute a `docker run` command. This follows the builder pattern, for example:
///
/// ```
/// let image = ImageUri::new(None, "twoliter", "latest");
/// let build = DockerRun::new(image).name("container-instance").command_arg("bash").execute().await?;
/// ```
#[derive(Debug, Clone)]
pub(crate) struct _DockerRun {
    _env: HashMap<String, String>,
    _mounts: Vec<_Mount>,
    _name: Option<String>,
    _remove: bool,
    _user: Option<String>,
    _workdir: Option<PathBuf>,
    _image: ImageUri,
    _command_args: Vec<String>,
}

impl _DockerRun {
    pub(crate) fn _new(image: ImageUri) -> Self {
        Self {
            _env: HashMap::new(),
            _mounts: Vec::new(),
            _name: None,
            _remove: false,
            _user: None,
            _workdir: None,
            _image: image,
            _command_args: Vec::new(),
        }
    }

    /// Add an environment variable.
    pub(crate) fn _env<S1, S2>(mut self, key: S1, value: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        self._env.insert(key.into(), value.into());
        self
    }

    /// Add a mount with the `--mount` command.
    pub(crate) fn _mount(mut self, mount: _Mount) -> Self {
        self._mounts.push(mount);
        self
    }

    /// Give the container a name, for example `--name my-container`.
    pub(crate) fn _name<S>(mut self, name: S) -> Self
    where
        S: Into<String>,
    {
        self._name = Some(name.into());
        self
    }

    /// Enable the `--rm` command to remove the container when it exits.
    pub(crate) fn _remove(mut self) -> Self {
        self._remove = true;
        self
    }

    /// Set `--user`.
    pub(crate) fn _user<S>(mut self, user: S) -> Self
    where
        S: Into<String>,
    {
        self._user = Some(user.into());
        self
    }

    /// Set `--workdir`.
    pub(crate) fn _workdir<P>(mut self, workdir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self._workdir = Some(workdir.into());
        self
    }

    /// Add an argument to the end of the `docker run` command to be executed in the container.
    /// For example in `docker run hello-world bash`, bash is the first `command_arg`.
    pub(crate) fn _command_arg<S>(mut self, arg: S) -> Self
    where
        S: Into<String>,
    {
        self._command_args.push(arg.into());
        self
    }

    /// Add multiple build args, where `("KEY", value)` becomes `--build-arg=KEY=value`.
    pub(crate) fn _command_args<I: IntoIterator<Item = String>>(mut self, command_args: I) -> Self {
        self._command_args.extend(command_args.into_iter());
        self
    }

    /// Run the `docker run` command.
    pub(crate) async fn _execute(self) -> Result<()> {
        let mut args = vec!["run".to_string()];
        for mount in &self._mounts {
            args.push("--mount".to_string());
            args.push(mount._as_arg())
        }
        if let Some(name) = self._name.as_ref() {
            args.push("--name".to_string());
            args.push(name.to_string());
        }
        if self._remove {
            args.push("--rm".to_string());
        }
        if let Some(user) = &self._user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        // TODO - hacky, make configurable?
        args.push("--group-add".to_string());
        let gid = nix::unistd::Group::from_name("docker")
            .unwrap()
            .unwrap()
            .gid
            .to_string();
        args.push(gid);

        // TODO - configurable?
        args.push("--network=host".to_string());

        // TODO - this crap again
        args.push("--env=GOPROXY=direct".to_string());

        args.push(self._image._uri());
        args.extend(self._command_args.iter().map(|s| s.to_string()));
        exec(Command::new("docker").args(args.into_iter())).await
    }
}
