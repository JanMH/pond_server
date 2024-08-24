use rocket::serde::Deserialize;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Configuration {
    #[serde(default = "scripts")]
    pub scripts_path: String
}

fn scripts() -> String {
    "./scripts".to_owned()
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AuthorizationConfig {
    pub access_token: String
}
