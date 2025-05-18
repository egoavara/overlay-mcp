use std::sync::Arc;

use jsonwebtoken::TokenData;
use overlay_mcp_core::{
    auth::{
        AuthorizerConstantConfig, JwtContextPointerType, JwtWhitelistAndBlacklist,
        WhitelistAndBlacklist,
    },
    Authentication, AuthorizationResult, Error, GeneralAuthz,
};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};

#[derive(Clone)]
pub struct StaticAuthz(pub(crate) Arc<InnerStaticAuthz>);
pub struct InnerStaticAuthz {
    pub apikey: WhitelistAndBlacklist,
    pub jwt: Vec<JwtWhitelistAndBlacklist>,
}

impl GeneralAuthz for StaticAuthz {
    async fn authorize_enter(&self, target: &Authentication) -> Result<AuthorizationResult, Error> {
        match target {
            Authentication::ApiKey { apikey, .. } => self.authorize_apikey(apikey).await,
            Authentication::Jwt { jwt } => self.authorize_jwt(jwt).await,
            Authentication::NoAuth => Ok(AuthorizationResult::Unauthorized),
        }
    }

    async fn authorize_client_message(
        &self,
        _target: &Authentication,
        _message: &ClientJsonRpcMessage,
    ) -> Result<AuthorizationResult, Error> {
        // Static Authz not support client message authorization
        // Static Authz only for enter authorization
        Ok(AuthorizationResult::Allow)
    }

    async fn authorize_server_message(
        &self,
        _target: &Authentication,
        _message: &ServerJsonRpcMessage,
    ) -> Result<AuthorizationResult, Error> {
        // Static Authz not support client message authorization
        // Static Authz only for enter authorization
        Ok(AuthorizationResult::Allow)
    }
}

impl StaticAuthz {
    pub async fn new(config: &AuthorizerConstantConfig) -> Result<Self, Error> {
        let apikey = config.apikey.clone().unwrap_or_default();
        let jwt = config.jwt.clone();
        Ok(Self(Arc::new(InnerStaticAuthz { apikey, jwt })))
    }
    async fn authorize_apikey(&self, apikey: &str) -> Result<AuthorizationResult, Error> {
        if self.0.apikey.blacklist.contains(apikey) {
            Ok(AuthorizationResult::Deny)
        } else if self.0.apikey.whitelist.contains(apikey) {
            Ok(AuthorizationResult::Allow)
        } else {
            Ok(AuthorizationResult::Deny)
        }
    }
    async fn authorize_jwt(
        &self,
        jwt: &TokenData<serde_json::Value>,
    ) -> Result<AuthorizationResult, Error> {
        for jwtconfig in &self.0.jwt {
            let path = jwtconfig.path.clone();
            let path_value = match path.resolve(&jwt.claims) {
                Ok(a) => a,
                Err(_) => {
                    if jwtconfig.required {
                        return Ok(AuthorizationResult::Deny);
                    }
                    continue;
                }
            };
            let vals = match jwtconfig.r#type {
                JwtContextPointerType::String => {
                    path_value.as_str().map(|x| vec![x]).ok_or_else(|| {
                        Error::JwtClaimTypeError {
                            path: path.clone(),
                            expected_type: "string",
                            actual_type: get_actual_type(path_value),
                        }
                    })?
                }
                JwtContextPointerType::StringArray => path_value
                    .as_array()
                    .ok_or_else(|| Error::JwtClaimTypeError {
                        path: path.clone(),
                        expected_type: "string[]",
                        actual_type: get_actual_type(path_value),
                    })?
                    .iter()
                    .map(|x| {
                        x.as_str().ok_or_else(|| {
                            Error::JwtClaimTypeError {
                                path: path.clone(),
                                expected_type: "string",
                                actual_type: get_actual_type(x),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            };
            for val in vals {
                if jwtconfig.blacklist.contains(val) {
                    return Ok(AuthorizationResult::Deny);
                }
                if jwtconfig.whitelist.contains(val) {
                    return Ok(AuthorizationResult::Allow);
                }
            }
        }
        Ok(AuthorizationResult::Deny)
    }
}
fn get_actual_type(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::Array(v) if v.iter().all(|x| x.is_string()) => "string[]".to_string(),
        serde_json::Value::Array(v) if v.iter().all(|x| x.is_number()) => "number[]".to_string(),
        serde_json::Value::Array(v) if v.iter().all(|x| x.is_boolean()) => "boolean[]".to_string(),
        serde_json::Value::Array(v) if v.iter().all(|x| x.is_object()) => "object[]".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
    }
}
