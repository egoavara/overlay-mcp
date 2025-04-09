use anyhow::{Context, Result};
use axum::http::{header, Request, Response, StatusCode};
use axum_extra::extract::cookie::Cookie;
use http::request::Parts;
use jsonwebtoken::{jwk::JwkSet, Algorithm, DecodingKey, TokenData, Validation};
use oauth2::{
    basic::{BasicClient, BasicTokenResponse},
    AuthorizationCode, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use openidconnect::core::CoreProviderMetadata;
use openidconnect::ClientId as OidcClientId;
use serde::{de::value, Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use std::{borrow::Cow, vec};

use crate::Args;

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPAuthorizationRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPTokenRequest {
    pub grant_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code: String,
    pub code_verifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct MCPAuthClient {
    pub(crate) provider_metadata: CoreProviderMetadata,
    oauth_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>,
    scopes: Vec<String>,
    jwks: JwkSet,
}

impl MCPAuthClient {
    pub fn new(
        provider_metadata: CoreProviderMetadata,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    ) -> Result<Self> {
        let auth_url = provider_metadata.authorization_endpoint().clone();
        let token_url = provider_metadata.token_endpoint().cloned();

        let oauth_client = BasicClient::new(OidcClientId::new(client_id))
            .set_client_secret(ClientSecret::new(client_secret))
            .set_auth_uri(auth_url)
            .set_token_uri_option(token_url);

        let jwks = serde_json::to_string_pretty(provider_metadata.jwks()).unwrap();
        let jwks: JwkSet = serde_json::from_str::<JwkSet>(&jwks).unwrap();

        tracing::info!("JWKS length: {}", jwks.keys.len());
        for key in &jwks.keys {
            tracing::info!("JWKS key: alg={:?}, kid={:?}", key.common.key_algorithm, key.common.key_id);
        }

        Ok(Self {
            provider_metadata,
            oauth_client,
            scopes,
            jwks,
        })
    }

    pub fn get_auth_url(
        &self,
        code_challenge: PkceCodeChallenge,
        redirect_uri: String,
    ) -> Result<String, anyhow::Error> {
        let client_for_auth = self
            .oauth_client
            .clone()
            .set_redirect_uri(RedirectUrl::new(redirect_uri)?);

        let mut auth_request = client_for_auth
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(code_challenge);

        for scope in &self.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, _csrf_token) = auth_request.url();

        Ok(auth_url.to_string())
    }

    pub async fn exchange_code(
        &self,
        reqwest_client: &reqwest::Client,
        code: &str,
        code_verifier: PkceCodeVerifier,
        redirect_uri: String,
    ) -> Result<BasicTokenResponse> {
        let redirect_url_obj = RedirectUrl::new(redirect_uri)?;
        let token_request = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code.to_string()))?
            .set_pkce_verifier(code_verifier)
            .set_redirect_uri(Cow::Borrowed(&redirect_url_obj));

        let token_response = token_request.request_async(reqwest_client).await?;

        Ok(token_response)
    }

    fn get_validator(&self, jwt: &str) -> Result<(Vec<DecodingKey>, Validation)> {
        let header = jsonwebtoken::decode_header(jwt).context("Failed to decode header")?;
        let mut validator = jsonwebtoken::Validation::new(header.alg);
        validator.set_audience(&[self.oauth_client.client_id().to_string()]);
        let kid = header.kid;

        if let Some(kid) = kid {
            let key = self.jwks.find(&kid).unwrap();
            let decoder = DecodingKey::from_jwk(key).context("Failed to create decoding key")?;
            Ok((vec![decoder], validator))
        } else {
            let deckeys: Vec<DecodingKey> = self
                .jwks
                .keys
                .iter()
                .map(|key| DecodingKey::from_jwk(key).context("Failed to create decoding key"))
                .collect::<Result<Vec<_>>>()?;
            Ok((deckeys, validator))
        }
    }
    pub async fn handle_mcp_auth(
        &self,
        req: &Parts,
    ) -> Result<TokenData<Value>, Response<axum::body::Body>> {
        let token = requires_auth(req, self).map_err(|e| {
            tracing::info!("Failed to validate JWT: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-store")
                .body(axum::body::Body::from("Failed to validate JWT"))
                .unwrap()
        })?;
        if let Some(claims) = token {
            Ok(claims)
        } else {
            let err = Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::WWW_AUTHENTICATE, format!("Bearer error=\"invalid_token\", error_description=\"인증이 필요합니다.\", auth_url=\"{}\"", self.provider_metadata.authorization_endpoint().to_string()))    
            .body(axum::body::Body::from("Failed to validate JWT"))
            .map_err(|e| {
                tracing::error!("Failed to build response: {}", e);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .body(axum::body::Body::from("Failed to build response"))
                    .unwrap()
            })?;
            Err(err)
        }
    }
}

fn requires_auth(req: &Parts, auth_client: &MCPAuthClient) -> Result<Option<TokenData<Value>>> {
    if let Some(jwt) = extract_jwt(req)? {
        tracing::info!("JWT: {}", jwt);
        let (deckeys, validator) = auth_client.get_validator(&jwt)?;
        let mut success = None;
        let mut failures = Vec::new();
        for decoder in deckeys {
            let result = jsonwebtoken::decode::<serde_json::Value>(&jwt, &decoder, &validator);
            match result {
                Ok(token_data) => {
                    success.replace(token_data);
                    break;
                }
                Err(e) => {
                    failures.push(e);
                }
            }
        }
        success
            .ok_or_else(|| anyhow::anyhow!("{:?}", failures))
            .map(Some)
    } else {
        Ok(None)
    }
}

fn extract_jwt(req: &Parts) -> Result<Option<String>> {
    if let Some(header_value) = req.headers.get(header::AUTHORIZATION) {
        let header_value_str = header_value
            .to_str()
            .context("Authorization header is not a string")?;
        if header_value_str.starts_with("Bearer ") {
            let token = header_value_str.split("Bearer ").nth(1).unwrap();
            Ok(Some(token.to_string()))
        } else {
            Err(anyhow::anyhow!(
                "Authorization header is not a Bearer token"
            ))
        }
    } else {
        Ok(None)
    }
}
