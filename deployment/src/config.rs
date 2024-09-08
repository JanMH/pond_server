use std::{io, path::Path};

use figment::Figment;

use crate::{
    deployers::StaticSiteDeployer, ingress::{NginxStaticSiteIngressService, StaticSiteIngressService}, DeploymentManager
};

#[derive(Debug)]
pub enum ConfigurationError {
    MissingConfigurationValue(String),
}

const ROOT_DOMAIN_NAME: &str = "root_domain_name";
const NGINX_CONFIG_DIR: &str = "nginx_config_dir";
const NGINX_DEFAULT_CONFIG_DIR: &str = "/etc/nginx/";

const SCRIPTS_LOCATION: &str = "scripts_location";
const DEFAULT_SCRIPTS_LOCATION: &str = "scripts_location";


pub fn manager(figment: &Figment) -> Result<DeploymentManager, ConfigurationError> {
    let domain_name: String = figment
        .extract_inner(ROOT_DOMAIN_NAME)
        .map_err(|_e| ConfigurationError::MissingConfigurationValue(ROOT_DOMAIN_NAME.into()))?;
    let mut result = DeploymentManager::new(domain_name);
    configure_default_deployers(&mut result, figment)?;
    Ok(result)
}

fn configure_default_deployers(
    manager: &mut DeploymentManager,
    figment: &Figment,
) -> Result<(), ConfigurationError> {
    let ingress_service = ingress_manager(figment)?;
    let scripts_path: String = figment.extract_inner(SCRIPTS_LOCATION)
        .unwrap_or(DEFAULT_SCRIPTS_LOCATION.to_owned());
    let static_site_deployer = StaticSiteDeployer::new(scripts_path, Box::new(ingress_service));
    manager.register_deployer(static_site_deployer);

    Ok(())
}

fn ingress_manager(figment: &Figment) -> Result<NginxStaticSiteIngressService, ConfigurationError> {
    let nginx_config_dir_path = figment
        .extract_inner(NGINX_CONFIG_DIR)
        .unwrap_or(NGINX_DEFAULT_CONFIG_DIR.to_owned());
    let mut ingress = NginxStaticSiteIngressService::new();
    let nginx_config_dir_path: &Path = nginx_config_dir_path.as_ref();
    ingress.nginx_sites_available = nginx_config_dir_path.join("sites-available");
    ingress.nginx_sites_enabled = nginx_config_dir_path.join("sites-enabled");
    Ok(ingress)
}
