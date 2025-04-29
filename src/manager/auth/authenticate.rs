use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Result;
use axum::extract::{FromRequestParts, Request};
use http::{header, request::Parts, StatusCode};
use jsonwebtoken::{jwk::JwkSet, DecodingKey, TokenData, Validation};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, TokenUrl,
};
use openidconnect::{core::CoreProviderMetadata, IssuerUrl};
use tower::{Layer, Service};
use url::Url;

use crate::{reqmodifier::reference::HttpPartReference, utils::AnyError};

use super::{
    AuthenticaterConfig, AuthenticaterJwtConfig, Authentication, IdpClientConfig, JwtAudConfig,
    JwtValidatorConfig, JwtVerifierConfig,
};

#[derive(Clone)]
pub struct AuthenticaterLayer {
    pub(crate) authenticater: Authenticater,
}

#[derive(Clone)]
pub struct AuthenticaterMiddleware<S> {
    inner: S,
    pub(crate) authenticater: Authenticater,
}

pub type Authenticater = Arc<InnerAuthenticater>;

pub struct InnerAuthenticater {
    pub(crate) apikey_from: Vec<HttpPartReference>,
    pub(crate) issuer: Url,
    pub(crate) oauth_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>,
    pub(crate) kid_map: HashMap<String, DecodingKey>,
    pub(crate) no_kid_keys: Vec<DecodingKey>,
    pub(crate) empty_key: DecodingKey,
    pub(crate) jwt_validator: Option<JwtValidatorConfig>,
    pub(crate) client_config: IdpClientConfig,
}

impl AuthenticaterLayer {
    pub async fn new(config: &AuthenticaterConfig) -> Result<Self> {
        let reqwest_client = reqwest::Client::new();
        let (issuer, client, jwt_validator, client_config) =
            Self::load_idp(&config.jwt, &reqwest_client).await?;
        let (kid_map, no_kid_keys, jwt_validator) = match jwt_validator {
            Some((jwk, validator)) => {
                let mut kid_map = HashMap::new();
                let mut no_kid_keys = Vec::new();
                for key in &jwk.keys {
                    if let Some(kid) = &key.common.key_id {
                        kid_map.insert(kid.clone(), DecodingKey::from_jwk(key).unwrap());
                    } else {
                        no_kid_keys.push(DecodingKey::from_jwk(key).unwrap());
                    }
                }

                (kid_map, no_kid_keys, Some(validator))
            }
            None => (HashMap::new(), Vec::new(), None),
        };

        Ok(Self {
            authenticater: Arc::new(InnerAuthenticater {
                apikey_from: config.apikey.key_from.clone(),
                issuer,
                oauth_client: client,
                kid_map,
                no_kid_keys,
                empty_key: DecodingKey::from_secret(&[]),
                jwt_validator,
                client_config,
            }),
        })
    }

    async fn load_validator(
        config: &JwtVerifierConfig,
        reqwest_client: &reqwest::Client,
    ) -> Result<Option<(JwkSet, JwtValidatorConfig)>> {
        match config {
            JwtVerifierConfig::EmbededJwk { jwk, validator } => {
                Ok(Some((jwk.clone(), validator.clone())))
            }
            JwtVerifierConfig::JwkUrl { jwk_url, validator } => {
                let jwks = reqwest_client
                    .get(jwk_url.as_str())
                    .send()
                    .await?
                    .json()
                    .await?;
                Ok(Some((jwks, validator.clone())))
            }
            JwtVerifierConfig::NoCheck(_) => Ok(None),
        }
    }
    async fn load_idp(
        config: &AuthenticaterJwtConfig,
        reqwest_client: &reqwest::Client,
    ) -> Result<(
        Url,
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>,
        Option<(JwkSet, JwtValidatorConfig)>,
        IdpClientConfig,
    )> {
        match config {
            AuthenticaterJwtConfig::Oauth2 {
                verifier,
                client,
                auth_url,
                token_url,
                issuer,
            }
            | AuthenticaterJwtConfig::Oidc {
                verifier,
                client,
                auth_url,
                token_url,
                issuer,
            } => {
                let oauth_client = BasicClient::new(ClientId::new(client.id.clone()))
                    .set_client_secret(ClientSecret::new(client.secret.expose_secret().to_string()))
                    .set_auth_uri(AuthUrl::from_url(auth_url.clone()))
                    .set_token_uri_option(Some(TokenUrl::from_url(token_url.clone())));
                match verifier {
                    JwtVerifierConfig::EmbededJwk { .. } | JwtVerifierConfig::JwkUrl { .. } => {
                        Ok((
                            issuer.clone(),
                            oauth_client,
                            Self::load_validator(verifier, reqwest_client).await?,
                            client.clone(),
                        ))
                    }
                    JwtVerifierConfig::NoCheck(_) => {
                        Ok((issuer.clone(), oauth_client, None, client.clone()))
                    }
                }
            }
            AuthenticaterJwtConfig::OidcDiscovery {
                verifier,
                client,
                issuer,
            } => {
                // OIDC Discovery 수행 (config 사용)
                let issuer_url = IssuerUrl::new(issuer.clone())?;
                let result_issuer_url = issuer_url.url().clone();
                let provider_metadata =
                    CoreProviderMetadata::discover_async(issuer_url, reqwest_client)
                        .await
                        .map_err(|err| {
                            tracing::error!("Failed to discover OIDC metadata: {}", err);
                            err
                        })?;
                // Ensure token endpoint exists before creating MCPAuthClient
                if provider_metadata.token_endpoint().is_none() {
                    return Err(anyhow::anyhow!("Token endpoint not found in OIDC metadata"));
                }
                tracing::info!(url = ?issuer, "OIDC Discovered");

                let jwk = reqwest_client
                    .get(provider_metadata.jwks_uri().to_string())
                    .send()
                    .await?
                    .json()
                    .await?;
                let oauth_client = BasicClient::new(ClientId::new(client.id.clone()))
                    .set_client_secret(ClientSecret::new(client.secret.expose_secret().to_string()))
                    .set_auth_uri(provider_metadata.authorization_endpoint().clone())
                    .set_token_uri_option(provider_metadata.token_endpoint().cloned());
                Ok((
                    result_issuer_url,
                    oauth_client,
                    Some((jwk, verifier.clone())),
                    client.clone(),
                ))
            }
        }
    }
}

impl<S> Layer<S> for AuthenticaterLayer {
    type Service = AuthenticaterMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthenticaterMiddleware {
            inner,
            authenticater: self.authenticater.clone(),
        }
    }
}

impl<S> Service<Request> for AuthenticaterMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, mut req: Request) -> Self::Future {
        req.extensions_mut().insert(self.authenticater.clone());
        self.inner.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
}

impl<S> FromRequestParts<S> for Authentication
where
    S: Send + Sync + 'static,
{
    type Rejection = AnyError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let authenticater = parts
            .extensions
            .get::<Authenticater>()
            .expect("Authenticater not found");
        let jwt = authenticater.pick_jwt(parts)?;
        let apikey = authenticater.pick_apikey(parts)?;
        match (jwt, apikey) {
            (Some(jwt), Some((apikey, apikey_from))) => Ok(Authentication::Both {
                jwt,
                apikey,
                apikey_from,
            }),
            (Some(jwt), None) => Ok(Authentication::Jwt { jwt }),
            (None, Some((apikey, apikey_from))) => Ok(Authentication::ApiKey {
                apikey,
                apikey_from,
            }),
            (None, None) => Ok(Authentication::NoAuth),
        }
    }
}

impl InnerAuthenticater {
    pub fn create_oauth_client(
        &self,
    ) -> BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>
    {
        self.oauth_client.clone()
    }

    fn pick_apikey(&self, parts: &Parts) -> Result<Option<(String, HttpPartReference)>, AnyError> {
        if self.apikey_from.is_empty() {
            return Ok(None);
        }
        Ok(self
            .apikey_from
            .iter()
            .flat_map(|r| {
                r.resolve_http_part(parts)
                    .map(|v| (r.clone(), v.to_string()))
            })
            .map(|(r, v)| (v.to_string(), r))
            .next())
    }

    fn pick_jwt(&self, parts: &Parts) -> Result<Option<TokenData<serde_json::Value>>, AnyError> {
        let auth_header = match parts.headers.get(header::AUTHORIZATION) {
            Some(header) => header.to_str().map_err(|_| {
                AnyError::http(
                    StatusCode::BAD_REQUEST,
                    "Authorization header is not a string",
                )
            })?,
            None => return Ok(None),
        };

        match auth_header.split_once(" ") {
            Some((typ, val)) => {
                if typ.to_lowercase() != "bearer" {
                    return Err(AnyError::http(
                        StatusCode::BAD_REQUEST,
                        "Authorization header is not a Bearer token",
                    ));
                }
                let token = val.trim();
                let header = jsonwebtoken::decode_header(token)
                    .map_err(|_| AnyError::http(StatusCode::BAD_REQUEST, "Invalid token"))?;
                let validator = self.prepare_validator(&header);
                let mut failures = Vec::new();
                for key in self.pick_jwtkey_by_jwtheader(&header) {
                    let token = jsonwebtoken::decode::<serde_json::Value>(token, key, &validator);
                    match token {
                        Ok(data) => return Ok(Some(data)),
                        Err(e) => failures.push(e),
                    }
                }
                tracing::info!("Failed to validate token: {:#?}", &failures);
                Err(AnyError::http(StatusCode::BAD_REQUEST, "Invalid token"))
            }
            None => Err(AnyError::http(
                StatusCode::BAD_REQUEST,
                "Authorization header is not a string",
            )),
        }
    }

    fn pick_jwtkey_by_jwtheader<'a>(
        &self,
        header: &'a jsonwebtoken::Header,
    ) -> Box<dyn Iterator<Item = &DecodingKey> + '_> {
        if self.jwt_validator.is_none() {
            return Box::new(std::iter::once(&self.empty_key));
        }
        if let Some(kid) = &header.kid {
            if let Some(dec_key) = self.kid_map.get(kid) {
                return Box::new(std::iter::once(dec_key));
            }
        }
        Box::new(self.no_kid_keys.iter().chain(self.kid_map.values()))
    }

    pub(self) fn prepare_validator(&self, header: &jsonwebtoken::Header) -> Validation {
        let mut validator: Validation = Validation::new(header.alg);

        if let Some(config) = &self.jwt_validator {
            validator.set_required_spec_claims(&config.required_spec_claims);
            validator.leeway = config.leeway;
            validator.reject_tokens_expiring_in_less_than =
                config.reject_tokens_expiring_in_less_than;
            validator.validate_exp = config.validate_exp;
            validator.validate_nbf = config.validate_nbf;
            match &config.aud {
                JwtAudConfig::NoCheck => {
                    validator.validate_aud = false;
                }
                JwtAudConfig::ClientId => {
                    validator.set_audience(&[&self.client_config.id]);
                }
                JwtAudConfig::Audience(auds) => {
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
}
