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
    pub(crate) async fn execute(self) -> Result<()> {
        let mut args = vec!["build".to_string()];
        if let Some(dockerfile) = self.dockerfile.as_ref() {
            args.push("--file".to_string());
            args.push(dockerfile.display().to_string());
        }
        if let Some(tag) = self.tag.as_ref() {
            args.push("--tag".to_string());
            args.push(tag.uri());
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

#[derive(Debug, Copy, Clone, Serialize)]
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
#[derive(Debug, Default, Clone)]
pub(crate) struct Mount {
    pub(crate) type_: MountType,
    pub(crate) source: PathBuf,
    pub(crate) destination: PathBuf,
    pub(crate) read_only: bool,
}

impl Mount {
    fn as_arg(&self) -> String {
        let mut s = format!(
            "type={},source={},target={}",
            self.type_,
            self.source.display(),
            self.destination.display()
        );
        if self.read_only {
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
pub(crate) struct DockerRun {
    _env: HashMap<String, String>,
    mounts: Vec<Mount>,
    name: Option<String>,
    remove: bool,
    user: Option<String>,
    workdir: Option<PathBuf>,
    image: ImageUri,
    command_args: Vec<String>,
}

impl DockerRun {
    pub(crate) fn new(image: ImageUri) -> Self {
        Self {
            _env: HashMap::new(),
            mounts: Vec::new(),
            name: None,
            remove: false,
            user: None,
            workdir: None,
            image,
            command_args: Vec::new(),
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
    pub(crate) fn mount(mut self, mount: Mount) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Give the container a name, for example `--name my-container`.
    pub(crate) fn name<S>(mut self, name: S) -> Self
    where
        S: Into<String>,
    {
        self.name = Some(name.into());
        self
    }

    /// Enable the `--rm` command to remove the container when it exits.
    pub(crate) fn remove(mut self) -> Self {
        self.remove = true;
        self
    }

    /// Set `--user`.
    pub(crate) fn user<S>(mut self, user: S) -> Self
    where
        S: Into<String>,
    {
        self.user = Some(user.into());
        self
    }

    /// Set `--workdir`.
    pub(crate) fn workdir<P>(mut self, workdir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.workdir = Some(workdir.into());
        self
    }

    /// Add an argument to the end of the `docker run` command to be executed in the container.
    /// For example in `docker run hello-world bash`, bash is the first `command_arg`.
    pub(crate) fn command_arg<S>(mut self, arg: S) -> Self
    where
        S: Into<String>,
    {
        self.command_args.push(arg.into());
        self
    }

    /// Add multiple build args, where `("KEY", value)` becomes `--build-arg=KEY=value`.
    pub(crate) fn _command_args<I: IntoIterator<Item = String>>(mut self, command_args: I) -> Self {
        self.command_args.extend(command_args.into_iter());
        self
    }

    /// Run the `docker run` command.
    pub(crate) async fn execute(self) -> Result<()> {
        let mut args = vec!["run".to_string()];
        for mount in &self.mounts {
            args.push("--mount".to_string());
            args.push(mount.as_arg())
        }
        if let Some(name) = self.name.as_ref() {
            args.push("--name".to_string());
            args.push(name.to_string());
        }
        if self.remove {
            args.push("--rm".to_string());
        }
        if let Some(user) = &self.user {
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

        args.push(self.image.uri());
        args.extend(self.command_args.iter().map(|s| s.to_string()));
        exec(Command::new("docker").args(args.into_iter())).await
    }
}
