use crate::{
    deployer::StaticSiteDeployer,
    ingress::{
        self,
        dns::{DnsService, NoOpDnsService},
        static_site::NginxStaticSiteIngressService,
    },
    DeploymentManager,
};
use figment::Figment;
use std::error;

#[derive(Debug)]
pub enum ConfigurationError {
    MissingConfigurationValue(String),
    Other(Box<dyn error::Error + Send + Sync>),
}

impl From<figment::Error> for ConfigurationError {
    fn from(error: figment::Error) -> Self {
        match error.kind {
            figment::error::Kind::MissingField(cow) => {
                Self::MissingConfigurationValue(cow.to_string())
            }
            _ => Self::Other(Box::new(error)),
        }
    }
}

const ROOT_DOMAIN_NAME: &str = "root_domain_name";

const SCRIPTS_LOCATION: &str = "scripts_location";
const DEFAULT_SCRIPTS_LOCATION: &str = "./scripts";

fn figment_default_values() -> Figment {
    ingress::dns::cloudflare::CloudflareDnsService::figment_default_values()
        .join(NginxStaticSiteIngressService::figment_default_values())
}

pub fn manager(figment: &Figment) -> Result<DeploymentManager, ConfigurationError> {
    let figment = figment.clone().join(figment_default_values());
    let domain_name: String = figment
        .extract_inner(ROOT_DOMAIN_NAME)
        .map_err(|_e| ConfigurationError::MissingConfigurationValue(ROOT_DOMAIN_NAME.into()))?;
    let mut result = DeploymentManager::new(domain_name);
    configure_default_deployers(&mut result, &figment)?;
    Ok(result)
}

fn configure_default_deployers(
    manager: &mut DeploymentManager,
    figment: &Figment,
) -> Result<(), ConfigurationError> {
    let ingress_service = ingress_manager(figment)?;
    let scripts_path: String = figment
        .extract_inner(SCRIPTS_LOCATION)
        .unwrap_or(DEFAULT_SCRIPTS_LOCATION.to_owned());
    let static_site_deployer = StaticSiteDeployer::new(scripts_path, Box::new(ingress_service));
    manager.register_deployer(static_site_deployer);
    Ok(())
}

fn configure_dns_service(
    figment: &Figment,
) -> Result<Box<dyn DnsService + Send + Sync + 'static>, ConfigurationError> {
    let result: Box<dyn DnsService + Send + Sync> =
        match ingress::dns::cloudflare::CloudflareDnsService::configure(figment)? {
            Some(service) => Box::new(service),
            None => Box::new(NoOpDnsService),
        };
    Ok(result)
}

fn ingress_manager(figment: &Figment) -> Result<NginxStaticSiteIngressService, ConfigurationError> {
    let dns_service = configure_dns_service(figment)?;

    NginxStaticSiteIngressService::configure(figment, dns_service)
}

#[cfg(test)]
mod test {
    use figment::providers::Serialized;

    use super::*;

    #[test]
    fn test_load_default_values_work() {
        let required_values = Serialized::globals(serde_json::json!({
            ROOT_DOMAIN_NAME: "example.com",
        }));
        let figment = figment_default_values().merge(required_values);
        let manager = manager(&figment);
        assert!(manager.is_ok());
    }
}
