use axum::{
    extract::{FromRef, State},
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::middleware::JwtMiddlewareState;

use super::AppState;

#[derive(Debug, Serialize)]
pub struct WellKnownResponse {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub token_endpoint: String,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub registration_endpoint: String,
}

pub(crate) async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    let jwt_middleware = JwtMiddlewareState::from_ref(&state);
    let issuer = jwt_middleware.issuer.clone();
    let mut hostname = state.config.server.hostname.clone();
    hostname.set_path("/");
    hostname.set_query(None);
    hostname.set_fragment(None);

    let authorization_endpoint = hostname.join("/authorize").unwrap();
    let token_endpoint = hostname.join("/token").unwrap();
    let registration_endpoint = hostname.join("/register").unwrap();

    Json(WellKnownResponse {
        issuer: issuer.to_string(),
        authorization_endpoint: authorization_endpoint.to_string(),
        response_types_supported: vec!["code".to_string()],
        code_challenge_methods_supported: vec!["S256".to_string()],
        token_endpoint: token_endpoint.to_string(),
        token_endpoint_auth_methods_supported: vec!["client_secret_basic".to_string()],
        grant_types_supported: vec!["authorization_code".to_string()],
        registration_endpoint: registration_endpoint.to_string(),
    })
}
