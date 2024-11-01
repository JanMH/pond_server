use figment::{providers::Serialized, Figment};
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use super::StaticSiteIngressService;
use crate::{config::ConfigurationError, deployer::DeploymentHandle, ingress::dns::DnsService};

pub struct NginxStaticSiteIngressService {
    handlebars: Handlebars<'static>,
    pub certbot_command_name: String,
    pub nginx_sites_available: PathBuf,
    pub nginx_sites_enabled: PathBuf,
    pub dns_service: Box<dyn DnsService + 'static + Send + Sync>,
    pub ip_v4_address: Option<Ipv4Addr>,
    pub ip_v6_address: Option<Ipv6Addr>,
    pub dns_wait_timeout: std::time::Duration,
    pub dns_fixed_wait_timeout: std::time::Duration,
    pub dns_use_fixed_wait_timeout: bool,
}

impl NginxStaticSiteIngressService {
    pub fn configure(
        figment: &Figment,
        dns_service: Box<dyn DnsService + 'static + Send + Sync>,
    ) -> Result<Self, ConfigurationError> {
        let handlebars = Handlebars::new();
        let config: NginxIngressConfig = figment.extract_inner("nginx_ingress")?;

        Ok(NginxStaticSiteIngressService {
            handlebars,
            dns_service,
            certbot_command_name: config.certbot_command_name,
            nginx_sites_available: config.sites_available_path,
            nginx_sites_enabled: config.sites_enabled_path,
            ip_v4_address: config.ip_v4_address,
            ip_v6_address: config.ip_v6_address,
            dns_wait_timeout: Duration::from_secs(config.dns_wait_timeout_seconds),
            dns_fixed_wait_timeout: Duration::from_secs(config.dns_fixed_wait_timeout_seconds),
            dns_use_fixed_wait_timeout: config.dns_use_fixed_wait_timeout,
        })
    }

    pub fn figment_default_values() -> Figment {
        Figment::from(Serialized::defaults(serde_json::json!({
            "nginx_ingress":{
                "certbot_command_name": "certbot",
                "sites_available_path": "/etc/nginx/sites-available",
                "sites_enabled_path": "/etc/nginx/sites-enabled",
                "dns_use_fixed_wait_timeout": true,
                "dns_fixed_wait_timeout_seconds": 10,
                "dns_wait_timeout_seconds": 30
            }
        })))
    }

    fn run_certbot(
        &self,
        domain_names: &[String],
        deployment_handle: &mut DeploymentHandle,
    ) -> io::Result<()> {
        let mut command = Command::new(&self.certbot_command_name);
        command.args(["--nginx", "-n", "--expand"]);

        for d in domain_names {
            command.args(["--domain", d]);
        }

        crate::helpers::run_command(command, deployment_handle.clone())?;
        Ok(())
    }

    fn set_dns_records(
        &self,
        deployment_handle: &mut DeploymentHandle,
        domain_name: &str,
    ) -> anyhow::Result<()> {
        if let Some(ip_address) = self.ip_v4_address {
            writeln!(
                deployment_handle.info(),
                "Setting DNS record for domain {} to IPv4 address {}",
                domain_name,
                ip_address
            )
            .ok();
            self.dns_service
                .set_dns_record(domain_name, IpAddr::V4(ip_address))?;
        }

        if let Some(ip_address) = self.ip_v6_address {
            writeln!(
                deployment_handle.info(),
                "Setting DNS record for domain {} to IPv6 address {}",
                domain_name,
                ip_address
            )
            .ok();
            self.dns_service
                .set_dns_record(domain_name, IpAddr::V6(ip_address))?;
        }

        if self.ip_v4_address.is_none() && self.ip_v6_address.is_none() {
            writeln!(
                deployment_handle.info(),
                "No IP addresses configured. Not setting any records"
            )
            .ok();
        }

        Ok(())
    }

    fn wait_for_dns_records(&self, domain_name: &str) -> anyhow::Result<()> {
        let mut records: Vec<_> = vec![];
        if let Some(add) = self.ip_v4_address {
            records.push(IpAddr::V4(add));
        }
        if let Some(add) = self.ip_v6_address {
            records.push(IpAddr::V6(add));
        }

        if records.is_empty() {
            return Ok(());
        }
        if self.dns_use_fixed_wait_timeout {
            thread::sleep(self.dns_fixed_wait_timeout);
            Ok(())
        } else {
            crate::ingress::dns::wait_for_dns_records(
                domain_name,
                records.into_iter(),
                self.dns_wait_timeout,
            )
        }
    }

    fn configure_nginx(
        &self,
        data: NginxStaticSiteDeploymentData<'_>,
        deployment_handle: &mut DeploymentHandle,
    ) -> Result<(), io::Error> {
        let config = self
            .handlebars
            .render_template(
                include_str!("./static_site_nginx_template.handlebars"),
                &data,
            )
            .unwrap();
        let sites_available_path = self
            .nginx_sites_available
            .join(data.deployment_name.to_owned() + ".conf");
        let sites_enabled_path = self
            .nginx_sites_enabled
            .join(data.deployment_name.to_owned() + ".conf");
        std::fs::write(&sites_available_path, config).inspect_err(|e| {
            writeln!(
                deployment_handle.error(),
                "Failed to write file {:?} due to error: {:?}",
                sites_available_path,
                e
            )
            .ok();
        })?;
        writeln!(deployment_handle.info(), "Enabling site through symlink").ok();
        Ok(if !sites_enabled_path.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(sites_available_path, sites_enabled_path)?;
            #[cfg(not(unix))]
            panic!("Windows not supported");
        })
    }
}

impl StaticSiteIngressService for NginxStaticSiteIngressService {
    fn add_static_site_ingress(
        &self,
        deployment_name: &str,
        disk_location: &Path,
        domain_names: &[String],
        mut deployment_handle: DeploymentHandle,
    ) -> io::Result<()> {
        for domain_name in domain_names {
            self.set_dns_records(&mut deployment_handle, domain_name)
                .map_err(io::Error::other)?;
        }

        writeln!(deployment_handle.info(), "Waiting for DNS records").ok();
        for domain_name in domain_names {
            self.wait_for_dns_records(domain_name)
                .map_err(io::Error::other)?;
        }

        let data = NginxStaticSiteDeploymentData {
            deployment_name,
            disk_location,
            domain_names: &domain_names.join(" "),
        };
        write!(deployment_handle.info(), "Configuring nginx").ok();
        self.configure_nginx(data, &mut deployment_handle)?;

        writeln!(deployment_handle.info(), "Running certbot").ok();
        self.run_certbot(domain_names, &mut deployment_handle)?;
        writeln!(deployment_handle.info(), "Completed running certbot").ok();

        Ok(())
    }
}

#[derive(Serialize)]
struct NginxStaticSiteDeploymentData<'a> {
    deployment_name: &'a str,
    disk_location: &'a Path,
    domain_names: &'a str,
}

#[derive(Deserialize, Serialize)]
struct NginxIngressConfig {
    certbot_command_name: String,
    sites_available_path: PathBuf,
    sites_enabled_path: PathBuf,
    ip_v4_address: Option<Ipv4Addr>,
    ip_v6_address: Option<Ipv6Addr>,
    dns_wait_timeout_seconds: u64,
    dns_fixed_wait_timeout_seconds: u64,
    dns_use_fixed_wait_timeout: bool,
}

impl Default for NginxIngressConfig {
    fn default() -> Self {
        NginxIngressConfig {
            certbot_command_name: "certbot".to_owned(),
            sites_available_path: "/etc/nginx/sites-available".into(),
            sites_enabled_path: "/etc/nginx/sites-enabled".into(),
            ip_v4_address: None,
            ip_v6_address: None,
            dns_wait_timeout_seconds: 30,
            dns_fixed_wait_timeout_seconds: 10,
            dns_use_fixed_wait_timeout: true,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::ingress::dns::MockDnsService;

    use super::{NginxStaticSiteIngressService, StaticSiteIngressService};
    use std::{io, net::Ipv4Addr};

    fn test_nginx_ingress_service(dns_service: MockDnsService) -> NginxStaticSiteIngressService {
        NginxStaticSiteIngressService {
            handlebars: handlebars::Handlebars::new(),
            certbot_command_name: "echo".to_owned(),
            nginx_sites_available: std::env::temp_dir().join("sites-available"),
            nginx_sites_enabled: std::env::temp_dir().join("sites-enabled"),
            dns_service: Box::new(dns_service),
            ip_v4_address: Some(Ipv4Addr::new(127, 0, 0, 1)),
            ip_v6_address: None,
            dns_wait_timeout: std::time::Duration::from_secs(1),
            dns_fixed_wait_timeout: std::time::Duration::from_secs(0),
            dns_use_fixed_wait_timeout: true,
        }
    }

    #[test]
    fn test_happy_path() {
        let (message_stream, mut message_consumer) = crate::deployer::deployment_handle();
        let mut dns_service = MockDnsService::new();
        dns_service
            .expect_set_dns_record()
            .times(1)
            .returning(|_, addr| {
                assert_eq!(
                    addr,
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
                );
                Ok(())
            });

        let mut service = test_nginx_ingress_service(dns_service);

        service.nginx_sites_available = std::env::temp_dir().join("sites-available");
        service.nginx_sites_enabled = std::env::temp_dir().join("sites-enabled");

        std::fs::create_dir(&service.nginx_sites_available).ok();
        std::fs::create_dir(&service.nginx_sites_enabled).ok();

        service.certbot_command_name = "echo".to_owned();

        service
            .add_static_site_ingress(
                "test_site",
                "/var/www/test_site".as_ref(),
                &["localhost".to_owned()],
                message_stream,
            )
            .unwrap();
        let file_name = "test_site.conf";
        let site_file_path = service.nginx_sites_available.join(file_name);
        let site_symlink_path = service.nginx_sites_enabled.join(file_name);

        assert!(site_file_path.exists());
        assert!(site_symlink_path.exists());
        assert!(site_symlink_path.is_symlink());
        let command_output = io::read_to_string(message_consumer.info()).unwrap();

        assert!(command_output.contains("--nginx -n --expand --domain localhost\n"))
    }
}
