use std::{collections::HashSet, ops::Deref, sync::Arc};

use anyhow::Result;
use axum::extract::{FromRef, FromRequestParts};
use http::{header, request::Parts, StatusCode};
use jsonwebtoken::{jwk::JwkSet, Algorithm, DecodingKey, TokenData, Validation};
use oauth2::{basic::BasicClient, EndpointMaybeSet, EndpointNotSet, EndpointSet};
use url::Url;

use crate::config::{IdpClientConfig, JwtValidatorConfig};

#[derive(Clone)]
pub struct JwtMiddlewareState(Arc<JwtMiddlewareStateInner>);

impl JwtMiddlewareState {
    pub fn new(
        issuer: Url,
        oauth_client: BasicClient<
            EndpointSet,
            EndpointNotSet,
            EndpointNotSet,
            EndpointNotSet,
            EndpointMaybeSet,
        >,
        valiator_set: Option<(JwkSet, JwtValidatorConfig)>,
        client_config: IdpClientConfig,
    ) -> Result<Self> {
        let (jwk, jwt_validator) = match valiator_set {
            Some((jwk, valid_config)) => (jwk, Some(valid_config)),
            None => {
                let empty_jwk = JwkSet { keys: vec![] };
                (empty_jwk, None)
            }
        };
        Ok(Self(Arc::new(JwtMiddlewareStateInner {
            issuer,
            oauth_client,
            jwk,
            jwt_validator,
            client_config,
        })))
    }

    pub(self) fn prepare_validator(&self, alg: Algorithm) -> Validation {
        let mut validator = Validation::new(alg);

        if let Some(config) = &self.jwt_validator {
            validator.set_required_spec_claims(&config.required_spec_claims);
            validator.leeway = config.leeway;
            validator.reject_tokens_expiring_in_less_than =
                config.reject_tokens_expiring_in_less_than;
            validator.validate_exp = config.validate_exp;
            validator.validate_nbf = config.validate_nbf;
            match &config.aud {
                crate::config::JwtAudConfig::NoCheck => {
                    validator.validate_aud = false;
                }
                crate::config::JwtAudConfig::ClientId => {
                    validator.set_audience(&[&self.client_config.client_id]);
                }
                crate::config::JwtAudConfig::Audience(auds) => {
                    validator.set_audience(auds);
                }
            }
            if let Some(iss) = &config.iss {
                validator.set_issuer(iss);
            }
        } else {
            validator.required_spec_claims = HashSet::new();
            
            validator.validate_exp = false;
            validator.validate_nbf = false;
            validator.validate_aud = false;

            validator.insecure_disable_signature_validation();
        }
        validator
    }
    pub(self) fn prepare_decoding_key(&self, kid: Option<String>) -> Vec<DecodingKey> {
        if self.jwt_validator.is_none() {
            return vec![DecodingKey::from_secret(&[])];
        }
        if let Some(kid) = kid {
            match self.jwk.find(&kid) {
                Some(key) => vec![DecodingKey::from_jwk(key).unwrap()],
                None => vec![],
            }
        } else {
            self.jwk
                .keys
                .iter()
                .map(DecodingKey::from_jwk)
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        }
    }
}

impl Deref for JwtMiddlewareState {
    type Target = JwtMiddlewareStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct JwtMiddlewareStateInner {
    pub(crate) issuer: Url,
    pub(crate) oauth_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>,
    pub(crate) jwk: JwkSet,
    pub(crate) jwt_validator: Option<JwtValidatorConfig>,
    pub(crate) client_config: IdpClientConfig,
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
        let state = JwtMiddlewareState::from_ref(state);
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
                let keys = state.prepare_decoding_key(header.kid);
                let validator = state.prepare_validator(header.alg);
                let mut failures = Vec::new();
                for key in keys {
                    let token = jsonwebtoken::decode::<serde_json::Value>(token, &key, &validator);
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
