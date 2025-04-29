use std::collections::{HashMap, HashSet};

use json_patch::jsonptr::PointerBuf;
use jsonwebtoken::jwk::JwkSet;
use redact::Secret;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, OneOrMany};
use url::Url;

use crate::reqmodifier::reference::HttpPartReference;

use super::{JwtContext, JwtContextPointerType};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AuthConfig {
    OpenFga {
        authn: Box<AuthenticaterConfig>,
        openfga: Box<AuthorizerFgaConfig>,
    },
    Constant {
        authn: Box<AuthenticaterConfig>,
        constant: Box<AuthorizerConstantConfig>,
    },
}
impl AuthConfig {
    pub fn get_authenticater(&self) -> &AuthenticaterConfig {
        match self {
            AuthConfig::OpenFga {
                authn: authenticater,
                ..
            } => authenticater,
            AuthConfig::Constant {
                authn: authenticater,
                ..
            } => authenticater,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthenticaterConfig {
    pub apikey: AuthenticaterApikeyConfig,
    pub jwt: AuthenticaterJwtConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde_as]
pub struct AuthenticaterApikeyConfig {
    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub key_from: Vec<HttpPartReference>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum AuthenticaterJwtConfig {
    #[serde(rename = "oauth2")]
    Oauth2 {
        issuer: Url,
        auth_url: Url,
        token_url: Url,

        verifier: JwtVerifierConfig,

        client: IdpClientConfig,
    },
    #[serde(rename = "oidc")]
    Oidc {
        issuer: Url,
        auth_url: Url,
        token_url: Url,

        verifier: JwtVerifierConfig,

        client: IdpClientConfig,
    },
    #[serde(rename = "oidc-discovery")]
    OidcDiscovery {
        issuer: String,

        #[serde(default)]
        verifier: JwtValidatorConfig,

        client: IdpClientConfig,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizerFgaConfig {
    pub url: Url,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub store: String,

    pub check: FgaCheckConfig,
    pub apikey: ApikeyTupleConfig,
    pub jwt: JwtTupleConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FgaCheckConfig {
    pub group: String,
    pub relation: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApikeyTupleConfig {
    pub group: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtTupleConfig {
    pub group: String,
    pub claim_path: PointerBuf,
    pub context_fields: Vec<JwtContext>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizerConstantConfig {
    pub apikey: Option<WhitelistAndBlacklist>,

    #[serde_as(as = "OneOrMany<_>")]
    pub jwt: Vec<JwtWhitelistAndBlacklist>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtWhitelistAndBlacklist {
    pub path: PointerBuf,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "default_type")]
    pub r#type: JwtContextPointerType,
    #[serde(default)]
    pub whitelist: HashSet<String>,
    #[serde(default)]
    pub blacklist: HashSet<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhitelistAndBlacklist {
    #[serde(default)]
    pub whitelist: HashSet<String>,
    #[serde(default)]
    pub blacklist: HashSet<String>,
}

fn default_type() -> JwtContextPointerType {
    JwtContextPointerType::String
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IdpClientConfig {
    pub id: String,
    #[serde(serialize_with = "redact::serde::redact_secret")]
    pub secret: Secret<String>,
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

fn default_reject_tokens_expiring_in_less_than() -> u64 {
    0
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
