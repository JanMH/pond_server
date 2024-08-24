use std::{path::{Path, PathBuf}, process::Command};

pub struct DeploymentService {
    scripts_path: PathBuf
}
const ARTIFACT_LOCATION: &str = "ARTIFACT_LOCATION";
const DEPLOYMENT_NAME: &str = "DEPLOYMENT_NAME";

impl DeploymentService {
    pub fn new(path: impl AsRef<Path>) -> DeploymentService {
        DeploymentService {
            scripts_path: path.as_ref().to_owned()
        }
    }
    
    pub fn deploy_static(&self, deployment: &Deployment) -> anyhow::Result<String> {
        let result = Command::new(self.scripts_path.join("static_site.sh"))
            .env(DEPLOYMENT_NAME, &deployment.name)
            .env(ARTIFACT_LOCATION, &deployment.path)
            .output()?;
        assert!(result.status.success());
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    }
}

pub struct Deployment {
    pub name: String,
    pub path: PathBuf,
}
