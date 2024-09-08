use std::{collections::HashMap, io, path::Path, sync::Arc, thread};

use crate::{deployers::handle::message_channel, Deployer, DeploymentLogs, Manifest};

pub struct DeploymentManager {
    deployers: HashMap<&'static str, Arc<dyn Deployer + Send + Sync>>,
    root_domain_name: String
}

impl DeploymentManager {
    pub fn new(root_domain_name: impl AsRef<str>) -> DeploymentManager {
        DeploymentManager {
            deployers: HashMap::new(),
            root_domain_name: root_domain_name.as_ref().to_owned()
        }
    }

    pub fn deploy(
        &self,
        manifest: &str,
        artifact_location: &Path,
    ) -> Result<DeploymentLogs, DeploymentError> {
        let manifest = self.parse_manifest(manifest)?;
        let deployer = self
            .deployers
            .get(manifest.deployment_type.as_str())
            .ok_or(DeploymentError::UnknownDeploymentType)?
            .clone();
        let artifact_location = artifact_location.to_owned();
        let (mut handle, log) =  message_channel();
        
        thread::spawn(move || {
            match deployer.deploy(manifest, &artifact_location, handle.clone()).map_err(|e| DeploymentError::IOError(e)) {
                Ok(_) => write!(handle.info(), "Deployment succeeded").ok(),
                Err(e) => write!(handle.error(), "Deployment failed: {:?}", e).ok(),
            };
        });
        Ok(log)
    }
    
    fn parse_manifest(&self, manifest: &str) -> Result<Manifest, DeploymentError> {
        let mut manifest: Manifest =
            toml::from_str(manifest).map_err(|_e| DeploymentError::CouldNotParseManifest)?;
        if manifest.domain_names.is_empty() {
            manifest.domain_names.push(format!("{}.{}", manifest.name, self.root_domain_name));
        }
        Ok(manifest)
    }


    pub fn register_deployer<D: RegisterDeployment + Send + Sync + 'static>(
        &mut self,
        deployer: D,
    ) {
        self.deployers
            .insert(D::deployment_type(), Arc::new(deployer));
    }
}


#[derive(Debug)]
pub enum DeploymentError {
    CouldNotParseManifest,
    UnknownDeploymentType,
    IOError(io::Error)
}

pub trait RegisterDeployment: Deployer {
    fn deployment_type() -> &'static str;
}
