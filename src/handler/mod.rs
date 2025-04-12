mod handle_oauth2_auth;
mod handle_oauth2_client;
mod handle_oauth2_token;
mod handle_sse;
mod handle_message;
mod handle_wellknown_oauth2authorizationserver;
mod state;

pub mod utils;
use axum::{
    routing::{get, post},
    Router,
};
pub use state::*;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sse", get(handle_sse::handler))
        .route("/message", get(handle_message::handler))
        .route("/oauth2/auth", get(handle_oauth2_auth::handler))
        .route("/oauth2/token", post(handle_oauth2_token::handler))
        .route("/oauth2/client", post(handle_oauth2_client::handler))
        .route(
            "/.well-known/oauth-authorization-server",
            get(handle_wellknown_oauth2authorizationserver::handler),
        )
}
