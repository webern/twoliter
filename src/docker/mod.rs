mod docker_build;
mod image;
mod twoliter;

pub(crate) use self::docker_build::DockerBuild;
pub(crate) use self::image::{ImageArchUri, ImageUri};
pub(crate) use self::twoliter::create_twoliter_image_if_not_exists;
use crate::common::DEFAULT_ARCH;

pub(super) const DEFAULT_REGISTRY: &str = "public.ecr.aws/bottlerocket";
pub(super) const DEFAULT_SDK_NAME: &str = "bottlerocket-sdk";
// TODO - get this from lock file: https://github.com/bottlerocket-os/twoliter/issues/11
pub(super) const DEFAULT_SDK_VERSION: &str = "v0.32.0";

pub(crate) fn default_sdk() -> ImageArchUri {
    ImageArchUri::new(
        Some(DEFAULT_REGISTRY.into()),
        DEFAULT_SDK_NAME,
        DEFAULT_ARCH,
        DEFAULT_SDK_VERSION,
    )
}
