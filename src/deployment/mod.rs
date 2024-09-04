use std::{
    path::{Path, PathBuf},
    process::Command,
};
pub mod ingress;

mod helpers;

use crate::message::MessageSender;
use ingress::StaticSiteIngressService;

pub struct DeploymentService {
    root_domain_name: String,
    scripts_path: PathBuf,
    ingress_service: Box<dyn StaticSiteIngressService + 'static + Send + Sync>,
}

const ARTIFACT_LOCATION: &str = "ARTIFACT_LOCATION";
const DEPLOYMENT_NAME: &str = "DEPLOYMENT_NAME";

impl DeploymentService {
    pub fn new(
        root_domain_name: &str,
        scripts_path: impl AsRef<Path>,
        ingress_service: Box<dyn StaticSiteIngressService + 'static + Send + Sync>,
    ) -> DeploymentService {
        DeploymentService {
            root_domain_name: root_domain_name.to_owned(),
            scripts_path: scripts_path.as_ref().to_owned(),
            ingress_service,
        }
    }

    pub fn deploy_static(
        &self,
        deployment: &Deployment,
        message_stream: MessageSender,
    ) -> anyhow::Result<String> {
        let script_location = self.scripts_path.join("static_site.sh");
        info!("Launching command {:?}", script_location);

        let result = Command::new(script_location)
            .env(DEPLOYMENT_NAME, &deployment.name)
            .env(ARTIFACT_LOCATION, &deployment.artifact_path)
            .output()?;

        assert!(result.status.success());
        let domain_name = deployment.name.clone() + "." + &self.root_domain_name;
        info!(
            "Adding static site with domain {}, and path {:?}",
            domain_name, &deployment.artifact_path
        );

        self.ingress_service
            .add_static_site_ingress(
                &deployment.name,
                &deployment.artifact_path,
                &[&domain_name],
                message_stream,
            )
            .inspect_err(|e| {
                error!(
                    "Failed to add ingress for deployment {}. Error: {}",
                    deployment.name, e
                )
            })?;
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    }
}

pub struct Deployment {
    pub name: String,
    pub artifact_path: PathBuf,
    pub deployment_type: String,
    pub domain_names: Option<Vec<String>>,
}
