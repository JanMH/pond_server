use rocket::{
    http::Status,
    request::{self, FromRequest, Outcome, Request},
};

use crate::config::AuthorizationConfig;

pub struct AuthenticatedUser;

const AUTHORIZATION: &str = "Authorization";
const AUTHORIZATION_SCHEME_PREFIX: &str = "Bearer ";

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let auth_header = req.headers().get_one(AUTHORIZATION);
        let auth_header = match auth_header {
            Some(auth_header) => auth_header,
            None => return Outcome::Error((Status::Unauthorized, "Invalid access token")),
        };

        if !auth_header.starts_with(AUTHORIZATION_SCHEME_PREFIX) {
            return Outcome::Error((Status::Unauthorized, "Invalid access token"));
        }
        let auth_token = &auth_header[AUTHORIZATION_SCHEME_PREFIX.len()..];

        req.rocket()
            .state::<AuthorizationConfig>()
            .map(move |my_config: &AuthorizationConfig| {
                if my_config.access_token == auth_token {
                    Outcome::Success(AuthenticatedUser {})
                } else {
                    Outcome::Error((Status::Unauthorized, "Incorrect access token"))
                }
            })
            .unwrap_or(Outcome::Forward(Status::InternalServerError))
    }
}

#[cfg(test)]
mod test {
    use crate::http::auth::AUTHORIZATION;
    use crate::{rocket, rocket_test};
    use rocket::http::{Header, Status};
    use rocket::local::blocking::Client;

    use super::AuthenticatedUser;

    #[get("/test_auth")]
    fn test_auth(_user: AuthenticatedUser) -> &'static str {
        "Hello"
    }

    #[test]
    fn test_authentication_fairing_unauthorized() {
        let client = Client::tracked(rocket_test().mount("/", routes![test_auth]))
            .expect("valid rocket instance");
        let response = client.get(uri!(test_auth)).dispatch();
        assert_eq!(response.status(), Status::Unauthorized);
    }

    #[test]
    fn test_authentication_fairing_success() {
        let rocket = rocket_test().mount("/", routes![test_auth]);
        let secret_key = rocket.figment().find_value("access_token").unwrap();
        let client = Client::tracked(rocket).expect("valid rocket instance");
        let auth_header =
            "Bearer ".to_owned() + secret_key.as_str().expect("Could not obtain auth token");
        let response = client
            .get(uri!(test_auth))
            .header(Header::new(AUTHORIZATION, auth_header))
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
    }
}
