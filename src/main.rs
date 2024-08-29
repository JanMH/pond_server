#[macro_use]
extern crate rocket;


mod auth;
mod config;
mod deployment_routes;
mod deployment;

use config::{AuthorizationConfig, Configuration};
use deployment_routes::deploy;
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
        .manage(deployer)
        .attach(AdHoc::config::<AuthorizationConfig>())
}

#[cfg(test)]
fn rocket_test() -> rocket::Rocket<rocket::Build> {
    std::env::set_var("POND_PROFILE", "test");
    rocket()
}
