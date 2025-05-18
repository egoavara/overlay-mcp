use axum::{
    routing::{get, post},
    Extension, Router,
};
use overlay_mcp_auth::{Authn, Authz};
use overlay_mcp_core::{Config, Error};
use overlay_mcp_resolver::Resolver;
use overlay_mcp_session_manager::SessionManager;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

use crate::middlewares::{
    trace_layer, AuthLayer, ReqwestLayer, ResolverLayer, SessionManagerLayer,
};

pub mod meta;
pub mod well_known;

pub mod authorize;
pub mod message;
pub mod register;
pub mod sse;
pub mod token;

pub async fn router(cancel: CancellationToken, config: Config) -> Result<Router, Error> {
    let authn = Authn::new(&config).await?;
    let authz = Authz::new(&config).await?;

    let router = Router::new()
        .route("/authorize", get(authorize::handler))
        .route("/register", post(register::handler))
        .route("/token", post(token::handler))
        .route("/sse", get(sse::handler))
        .route("/message", post(message::handler))
        // TODO: .route("/mcp", get(mcp::handler).post(mcp::handler)) // for MCP 20250326 spec
        .nest("/.well-known", well_known::router(&config))
        .nest("/.meta", meta::router(&config))
        .layer(Extension(cancel.clone()))
        .layer(ResolverLayer::new(Resolver::new(cancel.clone(), &config)?))
        .layer(ReqwestLayer::new(reqwest::Client::new()))
        .layer(AuthLayer::new(authz, authn))
        .layer(SessionManagerLayer::new(
            SessionManager::new(cancel.clone(), &config).await?,
        ))
        .layer(trace_layer())
        .layer(CorsLayer::permissive())
        .with_state(config);
    Ok(router)
}
