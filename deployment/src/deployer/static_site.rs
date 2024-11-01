use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    helpers::run_command, ingress::static_site::StaticSiteIngressService,
    manager::RegisterDeployment,
};

use super::{Deployer, DeploymentHandle};

pub struct StaticSiteDeployer {
    scripts_path: PathBuf,
    ingress_service: Box<dyn StaticSiteIngressService + 'static + Send + Sync>,
}

const ARTIFACT_LOCATION: &str = "ARTIFACT_LOCATION";
const DEPLOYMENT_NAME: &str = "DEPLOYMENT_NAME";

impl StaticSiteDeployer {
    pub fn new(
        scripts_path: impl AsRef<Path>,
        ingress_service: Box<dyn StaticSiteIngressService + 'static + Send + Sync>,
    ) -> StaticSiteDeployer {
        StaticSiteDeployer {
            scripts_path: scripts_path.as_ref().to_owned(),
            ingress_service,
        }
    }
}

impl Deployer for StaticSiteDeployer {
    fn deploy(
        &self,
        manifest: crate::Manifest,
        artifact_location: &Path,
        deployment_handle: DeploymentHandle,
    ) -> io::Result<()> {
        let script_location = self.scripts_path.join("static_site.sh");
        info!("Launching command {:?}", script_location);
        let mut script_command = Command::new(script_location);
        script_command
            .env(DEPLOYMENT_NAME, &manifest.name)
            .env(ARTIFACT_LOCATION, artifact_location);

        let exit_status = run_command(script_command, deployment_handle.clone())
            .inspect_err(|e| error!("Failed to run static site deployment script. Error: {}", e))?;

        if !exit_status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to deploy static site. Command exited with status {}",
                    exit_status
                ),
            ));
        }
        self.ingress_service
            .add_static_site_ingress(
                &manifest.name,
                artifact_location,
                &manifest.domain_names,
                deployment_handle,
            )
            .inspect_err(|e| {
                error!(
                    "Failed to add ingress for deployment {}. Error: {}",
                    manifest.name, e
                )
            })?;
        Ok(())
    }
}

impl RegisterDeployment for StaticSiteDeployer {
    fn deployment_type() -> &'static str {
        "static-site"
    }
}
