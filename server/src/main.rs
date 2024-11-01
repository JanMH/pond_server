#[macro_use]
extern crate rocket;

mod config;
mod http;
mod message;

use config::AuthorizationConfig;
use http::deployment_routes::deploy;
use rocket::fairing::AdHoc;

#[launch]
fn rocket() -> _ {
    let figment = config::figment();
    let deployment_manager = match pond_deployment::config::manager(&figment) {
        Ok(manager) => manager,
        Err(e) => {
            panic!("Failed to create deployment manager: {:?}", e);
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
    std::env::set_var("POND_ROOT_DOMAIN_NAME", "example.com");
    std::env::set_var("POND_ACCESS_TOKEN", "test_access_token");
    rocket()
}
