mod handle_mcp;
mod handle_oauth2_auth;
mod handle_oauth2_client;
mod handle_oauth2_token;
mod handle_wellknown_oauth2authorizationserver;
mod state;

use axum::{
    response::IntoResponse,
    routing::{get, post},
    Extension, Router,
};
pub use state::*;

use crate::{
    manager::{Manager, ManagerTrait},
    utils::AnyError,
};

pub fn router() -> Router<AppState> {
    let mut router = Router::new();
    router = router
        .route("/test", get(test))
        .route(
            "/mcp",
            get(handle_mcp::handle_entrypoint::handle_get)
                .post(handle_mcp::handle_entrypoint::handle_post),
        )
        // https://modelcontextprotocol.io/specification/2025-03-26/basic/authorization#2-3-3-fallbacks-for-servers-without-metadata-discovery
        .route("/authorize", get(handle_oauth2_auth::handler))
        .route("/token", post(handle_oauth2_token::handler))
        .route("/register", post(handle_oauth2_client::handler))
        // https://modelcontextprotocol.io/specification/2025-03-26/basic/authorization#2-3-2-authorization-base-url
        .route(
            "/.well-known/oauth-authorization-server",
            get(handle_wellknown_oauth2authorizationserver::handler),
        );
    router
}

async fn test(Extension(mut session_manager): Extension<Manager>) -> Result<String, AnyError> {
    let url = session_manager.route().await?;
    Ok(url.to_string())
}
