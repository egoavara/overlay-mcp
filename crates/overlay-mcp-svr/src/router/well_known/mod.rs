pub mod oauth_authorization_server;


use axum::{routing::get, Router};
use overlay_mcp_core::Config;

pub fn router(_config: &Config) -> Router<Config> {
    Router::new().route(
        "/oauth-authorization-server",
        get(oauth_authorization_server::handler),
    )
}
