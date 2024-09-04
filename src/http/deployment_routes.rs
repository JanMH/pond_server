use std::sync::Arc;

use lazy_static::lazy_static;
use regex::Regex;
use rocket::form::Error;
use rocket::form::{self, Form};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::tokio::task;
use rocket::State;
use serde::Deserialize;

use super::auth::AuthenticatedUser;
use crate::deployment::{Deployment, DeploymentService};
use crate::message::{message_channel, MessageReceiver};

lazy_static! {
    static ref NAME_VALIDATION_REGEX: Regex = Regex::new("[a-zA-Z-]{3,50}").unwrap();
}

fn validate_regex<'a>(
    value: &'a str,
    regex: &Regex,
    error_message: &'static str,
) -> form::Result<'a, ()> {
    if regex.is_match(value) {
        Ok(())
    } else {
        Err(Error::validation(error_message))?
    }
}

#[derive(Debug, FromForm)]
pub struct DeploymentRequest<'r> {
    manifest: &'r str,
    artifact: TempFile<'r>,
}

#[derive(Debug, Deserialize)]
struct DeploymentInformation {
    name: String,
    deployment_type: String,
    domain_names: Option<Vec<String>>,
}

#[post("/deploy", data = "<request>")]
pub fn deploy<'r>(
    _user: AuthenticatedUser,
    request: Form<DeploymentRequest<'r>>,
    deployment_service: &State<Arc<DeploymentService>>,
) -> Result<MessageReceiver, Custom<String>> {
    let req = request.manifest;

    let deployment_information: DeploymentInformation =
        toml::from_str::<DeploymentInformation>(request.manifest).map_err(|e| {
            Custom(
                Status::BadRequest,
                format!("Invalid manifest file: {:?}", e),
            )
        })?;

    let (stream, receiver) = message_channel();
    let path = request.artifact.path().unwrap().to_owned();
    match deployment_information.deployment_type.as_str() {
        "static" => {}
        _ => Err(Custom(
            Status::BadRequest,
            "Unknown deployment type: ".to_owned() + &deployment_information.deployment_type,
        ))?,
    }
    let deployment_service: Arc<DeploymentService> = Arc::clone(deployment_service);
    let deployment = Deployment {
        name: deployment_information.name.to_owned(),
        deployment_type: deployment_information.deployment_type.to_owned(),
        domain_names: deployment_information.domain_names.clone(),
        artifact_path: path,
    };
    task::spawn_blocking::<_, Result<(), anyhow::Error>>(move || {
        let result = deployment_service.deploy_static(&deployment, stream);
        info!("Deployment finished with result {:?}", result);
        Ok(())
    });
    Ok(receiver)
}
