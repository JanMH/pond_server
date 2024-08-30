#[macro_use]
extern crate rocket;


mod config;
mod deployment;
mod http;
mod message;

use std::sync::Arc;

use config::{AuthorizationConfig, Configuration};
use http::deployment_routes::deploy;
use deployment::DeploymentService;
use deployment::ingress::NginxStaticSiteIngressService;
use rocket::fairing::AdHoc;

#[launch]
fn rocket() -> _ {
    let figment = config::figment();
    let configuration: Configuration = figment.extract().unwrap();
    let ingress_service = NginxStaticSiteIngressService::new();
    let deployer = DeploymentService::new(&configuration.root_domain_name, configuration.scripts_path, Box::new(ingress_service));

    rocket::custom(figment)
        .mount("/", routes![deploy])
        .manage(Arc::new(deployer))
        .attach(AdHoc::config::<AuthorizationConfig>())
}

#[cfg(test)]
fn rocket_test() -> rocket::Rocket<rocket::Build> {
    std::env::set_var("POND_PROFILE", "test");
    rocket()
}
