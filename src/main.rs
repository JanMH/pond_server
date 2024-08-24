#[macro_use] extern crate rocket;

mod config;
mod deployment_routes;
mod deployment_service;
mod auth;


use config::{AuthorizationConfig, Configuration};
use deployment_routes::deploy;
use deployment_service::DeploymentService;
use rocket::{fairing::AdHoc, Config};

#[launch]
fn rocket() -> _ {
    let configuration: Configuration = Config::figment()
        .extract()
        .unwrap();
    let deployer = DeploymentService::new(configuration.scripts_path);
    
    rocket::build()
        .mount("/", routes![deploy])
        .manage(deployer)
        .attach(AdHoc::config::<AuthorizationConfig>())
}

#[cfg(test)]
fn rocket_test() -> rocket::Rocket<rocket::Build> {
    std::env::set_var("ROCKET_PROFILE", "test");
    rocket()
}