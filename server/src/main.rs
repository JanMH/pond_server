#[macro_use]
extern crate rocket;

mod config;
mod http;
mod message;

use std::sync::Arc;

use config::AuthorizationConfig;
use http::deployment_routes::deploy;
use rocket::fairing::AdHoc;

#[launch]
fn rocket() -> _ {
    let figment = config::figment();
    let deployment_manager = match pond_deployment::config::manager(&figment) {
        Ok(manager) => manager,
        Err(e) => {
            error!("Failed to create deployment manager: {:?}", e);
            std::process::exit(1);
        }
    };

    rocket::custom(figment)
        .mount("/", routes![deploy])
        .manage(deployment_manager)
        .attach(AdHoc::config::<AuthorizationConfig>())
}

#[cfg(test)]
fn rocket_test() -> rocket::Rocket<rocket::Build> {
    std::env::set_var("POND_PROFILE", "test");
    rocket()
}
