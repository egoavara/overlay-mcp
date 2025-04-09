use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
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
