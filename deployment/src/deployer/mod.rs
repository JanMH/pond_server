use std::{io, path::Path};

pub(crate) mod handle;
pub use handle::deployment_handle;
pub use handle::MutexVecDequeRead as LogStream;
pub use handle::{DeploymentHandle, DeploymentLogs};

use crate::Manifest;

mod static_site;

pub(crate) use static_site::StaticSiteDeployer;

pub trait Deployer {
    fn deploy(
        &self,
        manifest: Manifest,
        artifact_location: &Path,
        deployment_handle: DeploymentHandle,
    ) -> io::Result<()>;
}
