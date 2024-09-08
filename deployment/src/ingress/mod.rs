use handlebars::Handlebars;
use serde::Serialize;
use std::{
    io, path::{Path, PathBuf}, process::Command
};

use crate::deployers::DeploymentHandle;

pub trait StaticSiteIngressService {
    fn add_static_site_ingress(
        &self,
        deployment_name: &str,
        disk_location: &Path,
        domain_names: &[String],
        message_stream: DeploymentHandle,
    ) -> io::Result<()>;
}

pub struct NginxStaticSiteIngressService {
    handlebars: Handlebars<'static>,
    pub certbot_command_name: String,
    pub nginx_sites_available: PathBuf,
    pub nginx_sites_enabled: PathBuf,
}

impl NginxStaticSiteIngressService {
    
    pub fn new() -> NginxStaticSiteIngressService {
        NginxStaticSiteIngressService {
            handlebars: Handlebars::new(),
            certbot_command_name: "certbot".to_owned(),
            nginx_sites_enabled: "/etc/nginx/sites-enabled".into(),
            nginx_sites_available: "/etc/nginx/sites-available".into(),
        }
    }
    fn run_certbot(
        &self,
        domain_names: &[String],
        message_stream: &mut DeploymentHandle,
    ) -> io::Result<()> {
        let mut command = Command::new(&self.certbot_command_name);
        command.args(["--nginx", "-n"]);

        for d in domain_names {
            command.args(["--domain", d]);
        }

        super::helpers::copy_command_results(command, message_stream.clone()).ok();
        Ok(())
    }
}

impl StaticSiteIngressService for NginxStaticSiteIngressService {
    fn add_static_site_ingress(
        &self,
        deployment_name: &str,
        disk_location: &Path,
        domain_names: &[String],
        mut message_stream: DeploymentHandle,
    ) -> io::Result<()> {
        write!(message_stream.info(), "Configuring nginx").ok();

        let data = NginxStaticSiteDeploymentData {
            deployment_name,
            disk_location,
            domain_names: &domain_names.join(" "),
        };

        let config = self.handlebars.render_template(
            include_str!("./static_site_nginx_template.handlebars"),
            &data,
        ).unwrap();

        let sites_available_path = self
            .nginx_sites_available
            .join(deployment_name.to_owned() + ".conf");
        let sites_enabled_path = self
            .nginx_sites_enabled
            .join(deployment_name.to_owned() + ".conf");

        std::fs::write(&sites_available_path, config).inspect_err(|e| {
            write!(
                message_stream.error(),
                "Failed to write file {:?} due to error: {:?}",
                sites_available_path,
                e
            )
            .ok();
        })?;
        write!(message_stream.info(), "Enabling site through symlink").ok();
        if !sites_enabled_path.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(sites_available_path, sites_enabled_path)?;
            #[cfg(not(unix))]
            panic!("Windows not supported");
        }

        write!(message_stream.info(), "Running certbot").ok();
        self.run_certbot(domain_names, &mut message_stream)?;
        Ok(())
    }
}

#[derive(Serialize)]
struct NginxStaticSiteDeploymentData<'a> {
    deployment_name: &'a str,
    disk_location: &'a Path,
    domain_names: &'a str,
}

#[cfg(test)]
mod test {
    use std::io;

    use super::{NginxStaticSiteIngressService, StaticSiteIngressService};

    #[test]
    fn test_happy_path() {
        let (message_stream, mut message_consumer) = crate::deployers::handle::message_channel();

        let mut service = NginxStaticSiteIngressService::new();
        service.nginx_sites_available = std::env::temp_dir().join("sites-available");
        service.nginx_sites_enabled = std::env::temp_dir().join("sites-enabled");

        std::fs::create_dir(&service.nginx_sites_available).ok();
        std::fs::create_dir(&service.nginx_sites_enabled).ok();

        service.certbot_command_name = "echo".to_owned();

        service
            .add_static_site_ingress(
                "test_site",
                "/var/www/test_site".as_ref(),
                &["domain_name".to_owned()],
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

        assert!(command_output.contains("--nginx -n --domain domain_name\n"))
    }
}
