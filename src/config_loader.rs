use anyhow::{Ok, Result};
use jsonwebtoken::jwk::JwkSet;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, TokenUrl,
};
use openidconnect::{core::CoreProviderMetadata, IssuerUrl};
use url::Url;

use crate::config::{IdpClientConfig, IdpConfig, JwtValidatorConfig, JwtVerifierConfig};

impl JwtVerifierConfig {
    pub async fn load_validator(
        &self,
        reqwest_client: &reqwest::Client,
    ) -> Result<Option<(JwkSet, JwtValidatorConfig)>> {
        match self {
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
}

impl IdpConfig {
    pub async fn load(
        &self,
        reqwest_client: &reqwest::Client,
    ) -> Result<(
        Url,
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>,
        Option<(JwkSet, JwtValidatorConfig)>,
        IdpClientConfig,
    )> {
        match self {
            IdpConfig::Oauth2 {
                jwt,
                client,
                auth_url,
                token_url,
                issuer,
            }
            | IdpConfig::Oidc {
                jwt,
                client,
                auth_url,
                token_url,
                issuer,
            } => {
                let oauth_client = BasicClient::new(ClientId::new(client.client_id.clone()))
                    .set_client_secret(ClientSecret::new(
                        client.client_secret.expose_secret().to_string(),
                    ))
                    .set_auth_uri(AuthUrl::from_url(auth_url.clone()))
                    .set_token_uri_option(Some(TokenUrl::from_url(token_url.clone())));
                match jwt {
                    JwtVerifierConfig::EmbededJwk { .. } | JwtVerifierConfig::JwkUrl { .. } => {
                        Ok((
                            issuer.clone(),
                            oauth_client,
                            jwt.load_validator(reqwest_client).await?,
                            client.clone(),
                        ))
                    }
                    JwtVerifierConfig::NoCheck(_) => {
                        Ok((issuer.clone(), oauth_client, None, client.clone()))
                    }
                }
            }
            IdpConfig::OidcDiscovery {
                jwt,
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
                let oauth_client = BasicClient::new(ClientId::new(client.client_id.clone()))
                    .set_client_secret(ClientSecret::new(
                        client.client_secret.expose_secret().to_string(),
                    ))
                    .set_auth_uri(provider_metadata.authorization_endpoint().clone())
                    .set_token_uri_option(provider_metadata.token_endpoint().cloned());
                Ok((
                    result_issuer_url,
                    oauth_client,
                    Some((jwk, jwt.clone())),
                    client.clone(),
                ))
            }
        }
    }
}
