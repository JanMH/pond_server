use rocket::{
    figment::{
        providers::{Env, Format, Toml},
        Figment,
    },
    serde::Deserialize,
    Config,
};

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Configuration {
    #[serde(default = "scripts")]
    pub scripts_path: String,

    pub root_domain_name: String,
}

fn scripts() -> String {
    "./scripts".to_owned()
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AuthorizationConfig {
    pub access_token: String,
}

pub fn figment() -> Figment {
    let default_config_path = option_env!("POND_CONFIG_DEFAULT_PATH").unwrap_or("./pond.toml");
    let result = Figment::from(Config::default())
        .merge(Toml::file(Env::var_or("POND_CONFIG", default_config_path)).nested())
        .merge(Env::prefixed("POND_"));
    result.select(profile_name())
}

fn profile_name() -> String {
    if let Ok(profile) = std::env::var("POND_PROFILE") {
        profile
    } else if cfg!(test) {
        "test".to_owned()
    } else if cfg!(debug_assertions) {
        "debug".to_owned()
    } else {
        "release".to_owned()
    }
}
