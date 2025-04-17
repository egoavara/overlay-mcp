mod core;
mod constant_authorizer;
mod fga_authorizer;

use std::{str::FromStr, sync::Arc};

pub use core::*;
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
use valuable::Valuable;

use crate::{
    fga::Fga,
    middleware::{ApikeyExtractorState, JwtMiddlewareState, OptApikey, OptJwtClaim},
    utils::HttpComponent,
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
                Fga::init(
                    (*fga.openfga).clone(),
                    "overlay-mcp".to_string(),
                    &fga.headers,
                )
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
            Authorizer::Fga(fga) => {
                if let Some(engine) = &self.fga {
                    fga.check_fga(engine.clone(), request).await
                } else {
                    tracing::error!(config = fga.as_value(), "FGA not initialized");
                    AuthorizerResponse::Deny(AuthorizerResponseDeny {
                        authorizer: "fga".to_string(),
                        reason: Some("FGA not initialized".to_string()),
                    })
                }
            }
            Authorizer::Constant(constant) => self.check_constant(constant, request).await,
        }
    }

    async fn check_constant(
        &self,
        config: &ConstantAuthorizer,
        request: AuthorizerRequest,
    ) -> AuthorizerResponse {
        let blacklist = config.blacklist(&request);
        futures_util::pin_mut!(blacklist);
        #[allow(clippy::never_loop)]
        while let Some(x) = blacklist.next().await {
            return AuthorizerResponse::Deny(x);
        }

        let whitelist = config.whitelist(&request);
        futures_util::pin_mut!(whitelist);
        
        #[allow(clippy::never_loop)]
        while let Some(x) = whitelist.next().await {
            return AuthorizerResponse::Allow(x);
        }

        AuthorizerResponse::Deny(AuthorizerResponseDeny {
            authorizer: "default".to_string(),
            reason: Some("No matching whitelist or blacklist".to_string()),
        })
    }
}

pub struct CheckAuthorizer(pub AuthorizerResponse, pub CheckAuthorizerMetadata);
pub struct CheckAuthorizerMetadata {
    pub expected_status_code: StatusCode,
    pub apikey_from: Option<(String, HttpComponent)>,
}
impl<S> FromRequestParts<S> for CheckAuthorizer
where
    AuthorizerEngine: FromRef<S>,
    JwtMiddlewareState: FromRef<S>,
    ApikeyExtractorState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let authorizer = AuthorizerEngine::from_ref(state);
        let OptApikey(api_key) = OptApikey::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;
        let ClientIp(conn) = ClientIp::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;
        let OptJwtClaim(jwt) = OptJwtClaim::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;

        let request: AuthorizerRequest = AuthorizerRequest {
            ip: conn,
            method: parts.method.clone(),
            path: PathAndQuery::from_str(parts.uri.path()).unwrap(),
            headers: parts.headers.clone(),
            jwt: jwt.map(|jwt| jwt.claims),
            apikey: api_key.clone().map(|(apikey, _)| apikey),
        };
        let code = if request.jwt.is_some() || request.apikey.is_some() {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::UNAUTHORIZED
        };

        Ok(CheckAuthorizer(
            authorizer.check(request).await,
            CheckAuthorizerMetadata {
                expected_status_code: code,
                apikey_from: api_key,
            },
        ))
    }
}
