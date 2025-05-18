use axum::http::{header, request};
use httpbuilder::http_reference::HttpReference;
use jsonwebtoken::{jwk::JwkSet, DecodingKey, Validation};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, Scope, TokenUrl,
};
use openidconnect::{core::CoreProviderMetadata, IssuerUrl};
use overlay_mcp_core::{
    auth::{
        AuthenticaterConfig, AuthenticaterJwtConfig, IdpClientConfig, JwtAudConfig,
        JwtValidatorConfig, JwtVerifierConfig,
    },
    Authentication, Error, Error400, GeneralAuthn,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::Arc,
};
use url::Url;

#[derive(Clone)]
pub struct AuthnBasic(pub(crate) Arc<InnerAuthn>);

impl Deref for AuthnBasic {
    type Target = InnerAuthn;

    fn deref(&self) -> &Self::Target {
        Arc::as_ref(&self.0)
    }
}

pub struct InnerAuthn {
    pub(crate) apikey_from: Vec<HttpReference>,
    pub(crate) issuer: Url,
    pub(crate) oauth_client:
        BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>,
    pub(crate) kid_map: HashMap<String, DecodingKey>,
    pub(crate) no_kid_keys: Vec<DecodingKey>,
    pub(crate) empty_key: DecodingKey,
    pub(crate) jwt_validator: Option<JwtValidatorConfig>,
    pub(crate) client_config: IdpClientConfig,
}

impl AuthnBasic {
    pub async fn new(config: &AuthenticaterConfig) -> Result<Self, Error> {
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

        Ok(AuthnBasic(Arc::new(InnerAuthn {
            apikey_from: config.apikey.key_from.clone(),
            issuer,
            oauth_client: client,
            kid_map,
            no_kid_keys,
            empty_key: DecodingKey::from_secret(&[]),
            jwt_validator,
            client_config,
        })))
    }

    async fn load_idp(
        config: &AuthenticaterJwtConfig,
        reqwest_client: &reqwest::Client,
    ) -> Result<
        (
            Url,
            BasicClient<
                EndpointSet,
                EndpointNotSet,
                EndpointNotSet,
                EndpointNotSet,
                EndpointMaybeSet,
            >,
            Option<(JwkSet, JwtValidatorConfig)>,
            IdpClientConfig,
        ),
        Error,
    > {
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
                        .map_err(
                            |err: openidconnect::DiscoveryError<
                                oauth2::HttpClientError<reqwest::Error>,
                            >| {
                                tracing::error!("Failed to discover OIDC metadata: {}", err);
                                err
                            },
                        )?;
                // Ensure token endpoint exists before creating MCPAuthClient
                if provider_metadata.token_endpoint().is_none() {
                    return Err(Error::NoTokenEndpoint);
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

    async fn load_validator(
        config: &JwtVerifierConfig,
        reqwest_client: &reqwest::Client,
    ) -> Result<Option<(JwkSet, JwtValidatorConfig)>, Error> {
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
}
impl InnerAuthn {
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

    fn prepare_validator(&self, header: &jsonwebtoken::Header) -> Validation {
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

impl GeneralAuthn for InnerAuthn {
    fn create_oauth_client(
        &self,
    ) -> BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>
    {
        self.oauth_client.clone()
    }
    fn issuer_url(&self) -> Url {
        self.issuer.clone()
    }

    fn scopes(&self) -> Vec<Scope> {
        let scopes = self.client_config.scopes.clone();
        scopes.into_iter().map(Scope::new).collect()
    }
    async fn authenticate(&self, target: &request::Parts) -> Result<Authentication, Error> {
        for http_ref in &self.apikey_from {
            let Some(apikey_value) = httpbuilder::resolve(http_ref, target) else {
                continue;
            };

            return Ok(Authentication::ApiKey {
                apikey: apikey_value,
                apikey_from: http_ref.clone(),
            });
        }
        if let Some(authorization) = target.headers.get(header::AUTHORIZATION) {
            let data = authorization.to_str().map_err(|_| {
                Error::BadRequest(Error400::InvalidHeaderString(header::AUTHORIZATION))
            })?;
            let mut data_splited = data.split_whitespace();
            let first_data = data_splited.next().unwrap_or_default();
            let second_data = data_splited.next().unwrap_or_default();
            match (
                first_data.trim().to_lowercase().as_str(),
                second_data.trim(),
            ) {
                ("bearer", token) => {
                    let header = jsonwebtoken::decode_header(token).map_err(|_| {
                        Error::BadRequest(Error400::InvalidToken("Invalid token header"))
                    })?;
                    let validator = self.prepare_validator(&header);
                    let mut failures = Vec::new();
                    for key in self.pick_jwtkey_by_jwtheader(&header) {
                        let token =
                            jsonwebtoken::decode::<serde_json::Value>(token, key, &validator);
                        match token {
                            Ok(data) => return Ok(Authentication::Jwt { jwt: data }),
                            Err(e) => failures.push(e),
                        }
                    }
                    tracing::info!("Failed to validate token: {:#?}", &failures);
                    return Err(Error::BadRequest(Error400::InvalidToken(
                        "No valid key for jwt token",
                    )));
                }
                (authn_type, _) => {
                    return Err(Error::BadRequest(Error400::BearerTokenExpected(
                        authn_type.to_string(),
                    )));
                }
            }
        }
        Ok(Authentication::NoAuth)
    }
}
