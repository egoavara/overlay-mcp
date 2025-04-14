use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use axum::extract::{FromRef, FromRequestParts};
use http::{header, request::Parts, StatusCode};
use jsonwebtoken::{jwk::JwkSet, TokenData, Validation};
use openidconnect::core::CoreProviderMetadata;

use crate::config::{Config};

#[derive(Clone)]
pub struct JwtMiddlewareState(Arc<JwtMiddlewareStateInner>);

impl JwtMiddlewareState {
    pub async fn load(
        meta: CoreProviderMetadata,
        config: &Config,
        client: &reqwest::Client,
    ) -> Result<Self> {
        let jwks = client
            .get(meta.jwks_uri().url().as_str())
            .send()
            .await?
            .json()
            .await?;
        Ok(Self(Arc::new(JwtMiddlewareStateInner {
            meta,
            jwks,
            allowed_audiences: vec![config.oidc.client_id.to_string()],
        })))
    }
}

impl Deref for JwtMiddlewareState {
    type Target = JwtMiddlewareStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct JwtMiddlewareStateInner {
    pub(crate) meta: CoreProviderMetadata,
    pub(crate) jwks: JwkSet,
    pub(crate) allowed_audiences: Vec<String>,
}

pub struct JwtClaim(pub TokenData<serde_json::Value>);
pub struct OptJwtClaim(pub Option<TokenData<serde_json::Value>>);

impl<S> FromRequestParts<S> for JwtClaim
where
    JwtMiddlewareState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let result = OptJwtClaim::from_request_parts(parts, state).await?;
        match result.0 {
            Some(data) => Ok(JwtClaim(data)),
            None => Err((StatusCode::UNAUTHORIZED, "No token provided")),
        }
    }
}

impl<S> FromRequestParts<S> for OptJwtClaim
where
    JwtMiddlewareState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let JwtMiddlewareState(state) = JwtMiddlewareState::from_ref(state);
        let auth_header = match parts.headers.get(header::AUTHORIZATION) {
            Some(header) => header.to_str().map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Authorization header is not a string",
                )
            })?,
            None => return Ok(OptJwtClaim(None)),
        };

        match auth_header.split_once(" ") {
            Some((typ, val)) => {
                if typ.to_lowercase() != "bearer" {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "Authorization header is not a Bearer token",
                    ));
                }
                let token = val.trim();
                let header = jsonwebtoken::decode_header(token)
                    .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid token"))?;
                let keys = match header.kid {
                    Some(kid) => vec![state
                        .jwks
                        .find(&kid)
                        .ok_or((StatusCode::BAD_REQUEST, "Invalid token"))?],
                    None => state.jwks.keys.iter().collect(),
                };
                let mut failures = Vec::new();
                for key in keys {
                    let dec_key = jsonwebtoken::DecodingKey::from_jwk(key)
                        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid JWK"))?;
                    let mut validation = Validation::new(header.alg);
                    validation.set_audience(&state.allowed_audiences);
                    
                    let token = jsonwebtoken::decode::<serde_json::Value>(
                        token,
                        &dec_key,
                        &validation,
                    );
                    match token {
                        Ok(data) => return Ok(OptJwtClaim(Some(data))),
                        Err(e) => failures.push(e),
                    }
                }
                tracing::info!("Failed to validate token: {:#?}", &failures);
                Err((StatusCode::UNAUTHORIZED, "Invalid token"))
            }
            None => Err((
                StatusCode::BAD_REQUEST,
                "Authorization header is not a string",
            )),
        }
    }
}
