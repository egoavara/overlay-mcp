use std::{path::PathBuf, sync::Arc};

use axum::extract::FromRef;
use oauth2::{
    basic::BasicClient, ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet,
};
use openidconnect::core::CoreProviderMetadata;
use reqwest::Client;

use crate::{config::{Config}, middleware::JwtMiddlewareState};

#[derive(Clone)]
pub struct AppState {
    pub(crate) reqwest: Client,
    pub(crate) jwt_middleware: JwtMiddlewareState,
    pub(crate) configfile: Arc<Option<PathBuf>>,
    pub(crate) config: Arc<Config>,
}

impl AppState {
    pub fn get_oauth_client(
        &self,
    ) -> BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>
    {
        let oauth_client = BasicClient::new(ClientId::new(self.config.oidc.client_id.clone()))
            .set_client_secret(ClientSecret::new(self.config.oidc.client_secret.expose_secret().to_string()))
            .set_auth_uri(self.jwt_middleware.meta.authorization_endpoint().clone())
            .set_token_uri_option(self.jwt_middleware.meta.token_endpoint().cloned());
        oauth_client
    }
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

impl FromRef<AppState> for CoreProviderMetadata {
    fn from_ref(input: &AppState) -> Self {
        input.jwt_middleware.meta.clone()
    }
}
