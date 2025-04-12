
use axum::{extract::State, Json};
use chrono::{Duration, Utc};
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

use super::AppState;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct ClientRegisterRequest {
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: String,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    client_name: String,
    client_uri: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClientRegisterResponse {
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
    client_id_issued_at: i64,
    client_secret_expires_at: i64,
}

pub(crate) async fn handler(
    State(_): State<AppState>,
    Json(value): Json<ClientRegisterRequest>,
) -> Json<ClientRegisterResponse> {
    let raw_client_secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .collect::<Vec<_>>();
    let client_secret = String::from_utf8_lossy(&raw_client_secret).to_string();
    tracing::info!("Client register with secret '{}': {:?}", client_secret, value);

    let now = Utc::now();
    let issued_at = now.timestamp();
    let expires_at = (now + Duration::hours(1)).timestamp();

    Json(ClientRegisterResponse {
        client_id: uuid::Uuid::new_v4().to_string(),
        client_secret,
        redirect_uris: value.redirect_uris,
        client_id_issued_at: issued_at,
        client_secret_expires_at: expires_at,
    })
}
