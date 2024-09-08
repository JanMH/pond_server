use std::{
    io, path::Path
};

pub(crate) mod handle;
pub use handle::{DeploymentLogs, DeploymentHandle};
pub use handle::MutexVecDequeRead as LogStream;

use crate::Manifest;

mod static_site;

pub(crate) use static_site::StaticSiteDeployer;


pub trait Deployer {
    fn deploy(&self, manifest: Manifest, artifact_location: &Path, deployment_handle: DeploymentHandle) -> io::Result<()>;
}
