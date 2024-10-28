use lazy_static::lazy_static;
use pond_deployment::DeploymentManager;
use rand::distributions::DistString;
use rand::thread_rng;
use regex::Regex;
use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::State;

use crate::message::AsyncLogStream;

use super::auth::AuthenticatedUser;

lazy_static! {
    static ref NAME_VALIDATION_REGEX: Regex = Regex::new("[a-zA-Z-]{3,50}").unwrap();
}

#[derive(Debug, FromForm)]
pub struct DeploymentRequest<'r> {
    manifest: &'r str,
    artifact: TempFile<'r>,
}

#[post("/deploy", data = "<request>")]
pub async fn deploy<'r>(
    _user: AuthenticatedUser,
    mut request: Form<DeploymentRequest<'r>>,
    deployment_service: &State<DeploymentManager>,
) -> Result<AsyncLogStream, Custom<String>> {
    let artifact_location = std::env::temp_dir().join(format!(
        "artifact-{}.tar.gz",
        rand::distributions::Alphanumeric.sample_string(&mut thread_rng(), 5)
    ));

    request
        .artifact
        .persist_to(&artifact_location)
        .await
        .map_err(|e| {
            Custom(
                Status::InternalServerError,
                format!("Failed to save artifact: {:?}", e),
            )
        })?;

    let result = deployment_service
        .deploy(request.manifest, &artifact_location)
        .map_err(|e| {
            Custom(
                Status::InternalServerError,
                format!("Failed to start deployment {:?}", e),
            )
        })?;

    Ok(AsyncLogStream::from_deployment_logs(result))
}
