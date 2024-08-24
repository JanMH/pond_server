#[macro_use]
extern crate rocket;

mod auth;
mod config;
mod deployment_routes;
mod deployment_service;

use config::{AuthorizationConfig, Configuration};
use deployment_routes::deploy;
use deployment_service::DeploymentService;
use rocket::fairing::AdHoc;

#[launch]
fn rocket() -> _ {
    let figment = config::figment();
    let configuration: Configuration = figment.extract().unwrap();
    let deployer = DeploymentService::new(configuration.scripts_path);

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
