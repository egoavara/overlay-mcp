use axum_client_ip::ClientIpSource;
use hiqlite::NodeConfig;
use jsonwebtoken::jwk::JwkSet;
use redact::Secret;
use serde::{Deserialize, Serialize};
use serde_with::{formats::PreferOne, serde_as, OneOrMany};
use std::net::SocketAddr;
use url::Url;

use crate::{
    authorizer::Authorizer,
    utils::{HttpComponent, Passthrough},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub application: ApplicationConfig,
    pub server: ServerConfig,
    pub upstream: UpstreamConfig,
    pub idp: IdpConfig,
    pub authorizer: Option<Authorizer>,
    pub otel: Option<OpenTelemetryConfig>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationConfig {
    pub log_filter: Option<String>,
    pub ip_extract: Option<ClientIpSource>,
    pub prometheus: bool,
    pub health_check: bool,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub apikey: Vec<HttpComponent>,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub passthrough: Vec<Passthrough>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum UpstreamConfig {
    Static(StaticUpstream),
    HeadlessDiscovery(HeadlessDiscoveryUpstream),
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticUpstream {
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub urls: Vec<Url>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeadlessDiscoveryUpstream {
    pub discovery: Url,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenTelemetryConfig {
    pub endpoint: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub hostname: Url,
    #[serde(default)]
    pub cluster: ClusterConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ClusterConfig {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "raft")]
    Raft(RaftConfig),
    // TODO: Rdbms{}
}
impl Default for ClusterConfig {
    fn default() -> Self {
        Self::None {}
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RaftConfig {
    pub id: u64,
    pub secret: String,
    #[serde(default)]
    pub cluster: hiqlite::RaftConfig,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: u64,
    pub api: SocketAddr,
    pub raft: SocketAddr,
    #[serde(default = "default_read_pool_size")]
    pub read_pool_size: usize,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum IdpConfig {
    #[serde(rename = "oauth2")]
    Oauth2 {
        issuer: Url,
        auth_url: Url,
        token_url: Url,

        jwt: JwtVerifierConfig,

        #[serde(flatten)]
        client: IdpClientConfig,
    },
    #[serde(rename = "oidc")]
    Oidc {
        issuer: Url,
        auth_url: Url,
        token_url: Url,

        jwt: JwtVerifierConfig,

        #[serde(flatten)]
        client: IdpClientConfig,
    },
    #[serde(rename = "oidc-discovery")]
    OidcDiscovery {
        issuer: String,

        #[serde(default)]
        jwt: JwtValidatorConfig,

        #[serde(flatten)]
        client: IdpClientConfig,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IdpClientConfig {
    pub client_id: String,
    #[serde(serialize_with = "redact::serde::redact_secret")]
    pub client_secret: Secret<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum JwtVerifierConfig {
    EmbededJwk {
        jwk: JwkSet,
        #[serde(flatten)]
        validator: JwtValidatorConfig,
    },
    JwkUrl {
        jwk_url: Url,
        #[serde(flatten)]
        validator: JwtValidatorConfig,
    },
    NoCheck(NoCheck),
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum NoCheck {
    #[serde(rename = "no-check")]
    NoCheck,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtValidatorConfig {
    /// Which claims are required to be present before starting the validation.
    /// This does not interact with the various `validate_*`. If you remove `exp` from that list, you still need
    /// to set `validate_exp` to `false`.
    /// The only value that will be used are "exp", "nbf", "aud", "iss", "sub". Anything else will be ignored.
    ///
    /// Defaults to `{"exp"}`
    #[serde(default = "default_required_spec_claims")]
    pub required_spec_claims: Vec<String>,
    /// Add some leeway (in seconds) to the `exp` and `nbf` validation to
    /// account for clock skew.
    ///
    /// Defaults to `60`.
    #[serde(default = "default_leeway")]
    pub leeway: u64,
    /// Reject a token some time (in seconds) before the `exp` to prevent
    /// expiration in transit over the network.
    ///
    /// The value is the inverse of `leeway`, subtracting from the validation time.
    ///
    /// Defaults to `0`.
    #[serde(default = "default_reject_tokens_expiring_in_less_than")]
    pub reject_tokens_expiring_in_less_than: u64,
    /// Whether to validate the `exp` field.
    ///
    /// It will return an error if the time in the `exp` field is past.
    ///
    /// Defaults to `true`.
    #[serde(default = "default_true")]
    pub validate_exp: bool,
    /// Whether to validate the `nbf` field.
    ///
    /// It will return an error if the current timestamp is before the time in the `nbf` field.
    ///
    /// Validation only happens if `nbf` claim is present in the token.
    /// Adding `nbf` to `required_spec_claims` will make it required.
    ///
    /// Defaults to `false`.
    #[serde(default = "default_false")]
    pub validate_nbf: bool,

    /// Validation will check that the `aud` field is a member of the
    /// audience provided and will error otherwise.
    /// Use `set_audience` to set it
    ///
    /// Validation only happens if `aud` claim is present in the token.
    /// Adding `aud` to `required_spec_claims` will make it required.
    ///
    /// Defaults to `None`.
    #[serde(default)]
    pub aud: JwtAudConfig,
    /// If it contains a value, the validation will check that the `iss` field is a member of the
    /// iss provided and will error otherwise.
    /// Use `set_issuer` to set it
    ///
    /// Validation only happens if `iss` claim is present in the token.
    /// Adding `iss` to `required_spec_claims` will make it required.
    ///
    /// Defaults to `None`.
    #[serde(default)]
    pub iss: Option<Vec<String>>,
}

impl Default for JwtValidatorConfig {
    fn default() -> Self {
        Self {
            required_spec_claims: default_required_spec_claims(),
            leeway: default_leeway(),
            reject_tokens_expiring_in_less_than: default_reject_tokens_expiring_in_less_than(),
            validate_exp: default_true(),
            validate_nbf: default_false(),
            aud: JwtAudConfig::default(),
            iss: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum JwtAudConfig {
    #[serde(rename = "none")]
    NoCheck,
    #[serde(rename = "client_id")]
    #[default]
    ClientId,
    #[serde(rename = "audience")]
    Audience(Vec<String>),
}

fn default_required_spec_claims() -> Vec<String> {
    vec!["exp".to_string()]
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_leeway() -> u64 {
    60
}
fn default_read_pool_size() -> usize {
    10
}

fn default_reject_tokens_expiring_in_less_than() -> u64 {
    0
}
