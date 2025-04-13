mod authorizer;
mod constant_authorizer;
mod fga_authorizer;

use std::sync::Arc;

pub use authorizer::*;
use axum::{
    body::Body,
    extract::{FromRef, FromRequestParts},
    response::IntoResponse,
};
use axum_client_ip::ClientIp;
pub use constant_authorizer::*;

use fga_authorizer::FgaAuthorizer;
use futures_util::StreamExt;
use http::{request::Parts, uri::PathAndQuery, Response};
use serde::{Deserialize, Serialize};

use crate::middleware::{JwtMiddlewareState, OptJwtClaim};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Authorizer {
    #[serde(untagged)]
    Fga(FgaAuthorizer),
    #[serde(untagged)]
    Constant(ConstantAuthorizer),
}


#[derive(Debug, Clone)]
pub struct AuthorizerEngine {
    pub config: Arc<Authorizer>,
}

impl AuthorizerEngine {
    pub fn new(config: Option<Authorizer>) -> Self {
        Self {
            config: Arc::new(config.unwrap_or(Authorizer::Constant(ConstantAuthorizer::default()))),
        }
    }

    pub async fn check(&self, request: AuthorizerRequest) -> AuthorizerResponse {
        match &*self.config {
            Authorizer::Fga(_) => self.check_fga(request).await,
            Authorizer::Constant(constant) => self.check_constant(constant, request).await,
        }
    }

    async fn check_fga(&self, request: AuthorizerRequest) -> AuthorizerResponse {
        todo!("unimplemented")
    }

    async fn check_constant(&self, config :&ConstantAuthorizer,request: AuthorizerRequest) -> AuthorizerResponse {
        let blacklist = config.blacklist(&request);
        futures_util::pin_mut!(blacklist);
        while let Some(x) = blacklist.next().await {
            return AuthorizerResponse::Deny(x);
        }

        let whitelist = config.whitelist(&request);
        futures_util::pin_mut!(whitelist);
        while let Some(x) = whitelist.next().await {
            return AuthorizerResponse::Allow(x);
        }
        
        AuthorizerResponse::Deny(AuthorizerResponseDeny{
            authorizer: "default".to_string(),
            reason: Some("No matching whitelist or blacklist".to_string()),
        }) 
    }
}

pub struct CheckAuthorizer(pub AuthorizerResponse);

impl<S> FromRequestParts<S> for CheckAuthorizer
where
    AuthorizerEngine: FromRef<S>,
    JwtMiddlewareState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let authorizer = AuthorizerEngine::from_ref(state);
        let ClientIp(conn) =
            ClientIp::from_request_parts(parts, state).await.map_err(|e| e.into_response())?;
        let OptJwtClaim(jwt) =
            OptJwtClaim::from_request_parts(parts, state).await.map_err(|e| e.into_response())?;

        let request: AuthorizerRequest = AuthorizerRequest {
            ip: conn,
            method: parts.method.clone(),
            path: parts
                .uri
                .path_and_query()
                .cloned()
                .unwrap_or_else(|| PathAndQuery::from_static("/")),
            headers: parts.headers.clone(),
            jwt: jwt.map(|jwt| jwt.claims),
        };
        
        Ok(CheckAuthorizer(authorizer.check(request).await))
    }
}
