
use axum::{extract::State, Extension, Json};
use overlay_mcp_auth::Authn;
use overlay_mcp_core::{Config, GeneralAuthn};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Response {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub token_endpoint: String,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub registration_endpoint: String,
}

pub(crate) async fn handler(
    State(config): State<Config>,
    Extension(authn): Extension<Authn>,
) -> Json<Response> {
    let issuer = authn.issuer_url();
    let mut hostname = config.server.hostname.clone();
    hostname.set_path("/");
    hostname.set_query(None);
    hostname.set_fragment(None);

    let authorization_endpoint = hostname.join("/authorize").unwrap();
    let token_endpoint = hostname.join("/token").unwrap();
    let registration_endpoint = hostname.join("/register").unwrap();

    Json(Response {
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
