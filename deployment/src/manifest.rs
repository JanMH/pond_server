use serde::Deserialize;

#[derive(Deserialize)]
pub struct Manifest {
    pub name: String,
    pub deployment_type: String,
    #[serde(default)]
    pub domain_names: Vec<String>,
}
