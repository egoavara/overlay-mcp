use std::sync::Arc;

use axum::extract::FromRef;
use reqwest::Client;

use crate::{authorizer::AuthorizerEngine, config::Config, middleware::{ApikeyExtractorState, JwtMiddlewareState}};

#[derive(Clone)]
pub struct AppState {
    pub(crate) reqwest: Client,
    pub(crate) jwt_middleware: JwtMiddlewareState,
    pub(crate) api_key_extractor: ApikeyExtractorState,
    pub(crate) authorizer: AuthorizerEngine,
    pub(crate) config: Arc<Config>,
}

impl FromRef<AppState> for Client {
    fn from_ref(input: &AppState) -> Self {
        input.reqwest.clone()
    }
}

impl FromRef<AppState> for JwtMiddlewareState {
    fn from_ref(input: &AppState) -> Self {
        input.jwt_middleware.clone()
    }
}

impl FromRef<AppState> for AuthorizerEngine {
    fn from_ref(input: &AppState) -> Self {
        input.authorizer.clone()
    }
}

impl FromRef<AppState> for ApikeyExtractorState {
    fn from_ref(input: &AppState) -> Self {
        input.api_key_extractor.clone()
    }
}
