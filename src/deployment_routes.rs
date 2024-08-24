use lazy_static::lazy_static;
use regex::Regex;
use rocket::form::Error;
use rocket::form::{self, Form};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::tokio::task;
use rocket::State;

use crate::auth::AuthenticatedUser;
use crate::deployment_service::{Deployment, DeploymentService};

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
    deployment_information: DeploymentInformation<'r>,
    artifact: TempFile<'r>,
}

#[derive(Debug, FromForm)]
struct DeploymentInformation<'r> {
    #[field(validate = validate_regex(&NAME_VALIDATION_REGEX, "Name must match"))]
    name: &'r str,
    deployment_type: &'r str,
}

#[post("/deploy", data = "<request>")]
pub async fn deploy(
    _user: AuthenticatedUser,
    request: Form<DeploymentRequest<'_>>,
    deployment_service: &State<DeploymentService>,
) -> Result<String, Custom<String>> {
    let path = request.artifact.path().unwrap().to_owned();
    match request.deployment_information.deployment_type {
        "static" => {}
        _ => Err(Custom(
            Status::BadRequest,
            "Unknown deployment type: ".to_owned() + request.deployment_information.deployment_type,
        ))?,
    }

    let result = task::block_in_place::<_, Result<String, anyhow::Error>>(move || {
        let deployment = Deployment {
            name: request.deployment_information.name.to_owned(),
            path,
        };
        deployment_service.deploy_static(&deployment)
    })
    .map_err(|e| {
        Custom(
            Status::InternalServerError,
            "Deployment command failed  ".to_owned() + &e.to_string(),
        )
    })?;

    Ok(result)
}
