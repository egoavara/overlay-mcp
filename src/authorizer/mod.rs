mod authorizer;
mod constant_authorizer;
mod fga_authorizer;

use std::{fmt, sync::Arc};

use anyhow::Context;
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
use http::{request::Parts, uri::PathAndQuery, Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    fga::Fga,
    middleware::{JwtMiddlewareState, OptJwtClaim},
};

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
    pub fga: Option<Arc<Fga>>,
}

impl AuthorizerEngine {
    pub async fn new(config: Option<Authorizer>) -> Self {
        let mut fgaresult = None;
        if let Some(fga) = config.as_ref().and_then(|c| match c {
            Authorizer::Fga(fga) => Some(fga),
            _ => None,
        }) {
            fgaresult = Some(Arc::new(
                Fga::init(fga.openfga.clone(), "overlay-mcp".to_string())
                    .await
                    .unwrap(),
            ));
        }
        Self {
            config: Arc::new(config.unwrap_or(Authorizer::Constant(ConstantAuthorizer::default()))),
            fga: fgaresult,
        }
    }

    pub async fn check(&self, request: AuthorizerRequest) -> AuthorizerResponse {
        match &*self.config {
            Authorizer::Fga(_) => self.check_fga(request).await,
            Authorizer::Constant(constant) => self.check_constant(constant, request).await,
        }
    }

    async fn check_fga(&self, request: AuthorizerRequest) -> AuthorizerResponse {
        let engine = self.fga.clone().expect("FGA must be initialized");
        let mut context = Vec::new();
        context.push((
            "user:temp".to_string(),
            "context".to_string(),
            format!("ip:{}", request.ip),
        ));
        if let Some(jwt) = request.jwt {
            if let Some(email) = jwt.get("email").and_then(|v| v.as_str()) {
                context.push((
                    "user:temp".to_string(),
                    "context".to_string(),
                    format!("jwtclaim:email={}", email),
                ));
            }
            if let Some(group) = jwt.get("group").and_then(|v| v.as_array()) {
                for g in group {
                    if let Some(g) = g.as_str() {
                        context.push((
                            "user:temp".to_string(),
                            "context".to_string(),
                            format!("jwtclaim:group={}", g),
                        ));
                    }
                }
            }
        }
        let deny_result = engine
            .check(
                (
                    "user:temp".to_string(),
                    "deny".to_string(),
                    format!("api:{}_{}", request.method, request.path),
                ),
                context.clone(),
            )
            .await
            .context("auth check failed")
            .unwrap();

        if deny_result {
            return AuthorizerResponse::Deny(AuthorizerResponseDeny {
                authorizer: "fga".to_string(),
                reason: Some("auth check failed".to_string()),
            });
        }

        let allow_result = engine
            .check(
                (
                    "user:temp".to_string(),
                    "allow".to_string(),
                    format!("api:{}_{}", request.method, request.path),
                ),
                context,
            )
            .await
            .context("auth check failed")
            .unwrap();
        if allow_result {
            return AuthorizerResponse::Allow(AuthorizerResponseAllow {
                authorizer: "fga".to_string(),
                reason: None,
            });
        }

        AuthorizerResponse::Deny(AuthorizerResponseDeny {
            authorizer: "fga".to_string(),
            reason: Some("auth check failed".to_string()),
        })
    }

    async fn check_constant(
        &self,
        config: &ConstantAuthorizer,
        request: AuthorizerRequest,
    ) -> AuthorizerResponse {
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

        AuthorizerResponse::Deny(AuthorizerResponseDeny {
            authorizer: "default".to_string(),
            reason: Some("No matching whitelist or blacklist".to_string()),
        })
    }
}

pub struct CheckAuthorizer(pub AuthorizerResponse, pub StatusCode);

impl<S> FromRequestParts<S> for CheckAuthorizer
where
    AuthorizerEngine: FromRef<S>,
    JwtMiddlewareState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let authorizer = AuthorizerEngine::from_ref(state);
        let ClientIp(conn) = ClientIp::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;
        let OptJwtClaim(jwt) = OptJwtClaim::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;

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
        let code = match &request.jwt {
            Some(jwt) => StatusCode::FORBIDDEN,
            None => StatusCode::UNAUTHORIZED,
        };

        Ok(CheckAuthorizer(authorizer.check(request).await, code))
    }
}
