mod nginx;
pub use nginx::NginxStaticSiteIngressService;


use std::io;
use std::path::Path;

use crate::deployer::DeploymentHandle;

pub trait StaticSiteIngressService {
    fn add_static_site_ingress(
        &self,
        deployment_name: &str,
        disk_location: &Path,
        domain_names: &[String],
        message_stream: DeploymentHandle,
    ) -> io::Result<()>;
}
