use crate::fga::Fga;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeAs, DisplayFromStr};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use url::Url;
use valuable::{Valuable, Value, Visit};

use super::{
    AuthorizerRequest, AuthorizerResponse, AuthorizerResponseAllow, AuthorizerResponseDeny,
};

#[derive(Debug, Clone, Deserialize, Serialize, Valuable)]
pub struct FgaAuthorizer {
    pub openfga: UrlValue,

    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl FgaAuthorizer {
    pub(crate) async fn check_fga(
        &self,
        engine: Arc<Fga>,
        request: AuthorizerRequest,
    ) -> AuthorizerResponse {
        let mut context = Vec::new();
        context.push((
            "user:temp".to_string(),
            "context".to_string(),
            format!("ip:{}", request.ip),
        ));
        if let Some(api_key) = request.apikey {
            context.push((
                "user:temp".to_string(),
                "context".to_string(),
                format!("apikey:{}", api_key),
            ));
        }
        if let Some(jwt) = request.jwt {
            if let Some(sub) = jwt.get("sub").and_then(|v| v.as_str()) {
                context.push((
                    "user:temp".to_string(),
                    "context".to_string(),
                    format!("jwtclaim:sub={}", sub),
                ));
            }
            if let Some(email) = jwt.get("email").and_then(|v| v.as_str()) {
                context.push((
                    "user:temp".to_string(),
                    "context".to_string(),
                    format!("jwtclaim:email={}", email),
                ));
            }
            if let Some(group) = jwt.get("groups").and_then(|v| v.as_array()) {
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
}

#[serde_as]
#[derive(Debug, Clone, Serialize)]
pub struct UrlValue(pub Url);

impl Deref for UrlValue {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for UrlValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'de> Deserialize<'de> for UrlValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let url = DisplayFromStr::deserialize_as::<D>(deserializer)?;
        Ok(UrlValue(url))
    }
}

impl Valuable for UrlValue {
    fn as_value(&self) -> Value<'_> {
        Value::String(self.0.as_str())
    }

    fn visit(&self, visit: &mut dyn Visit) {
        visit.visit_value(self.as_value());
    }
}
