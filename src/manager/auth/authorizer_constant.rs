use std::collections::HashSet;

use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};

use crate::utils::AnyError;

use super::{
    authorizer::AuthorizerTrait, Authentication, AuthorizationResult, AuthorizerConstantConfig,
    JwtContextPointerType, JwtWhitelistAndBlacklist, WhitelistAndBlacklist,
};

pub struct AuthorizerConstant {
    pub apikey: WhitelistAndBlacklist,
    pub jwt: Vec<JwtWhitelistAndBlacklist>,
}

impl AuthorizerConstant {
    pub fn new(config: &AuthorizerConstantConfig) -> Self {
        Self {
            apikey: config.apikey.clone().unwrap_or(WhitelistAndBlacklist {
                whitelist: HashSet::new(),
                blacklist: HashSet::new(),
            }),
            jwt: config.jwt.clone(),
        }
    }
}
impl AuthorizerTrait for AuthorizerConstant {
    async fn authorize_authentication(
        &self,
        target: &Authentication,
    ) -> Result<AuthorizationResult, AnyError> {
        self.authorize_is_authenticated(target)
    }
    async fn authorize_client_message(
        &self,
        _target: &Authentication,
        _message: &ClientJsonRpcMessage,
    ) -> Result<AuthorizationResult, AnyError> {
        // TODO: implement
        Ok(AuthorizationResult::Allow)
    }

    async fn authorize_server_message(
        &self,
        _target: &Authentication,
        _message: &ServerJsonRpcMessage,
    ) -> Result<AuthorizationResult, AnyError> {
        // TODO: implement
        Ok(AuthorizationResult::Allow)
    }
}

impl AuthorizerConstant {
    fn authorize_is_authenticated(
        &self,
        target: &Authentication,
    ) -> Result<AuthorizationResult, AnyError> {
        match target {
            Authentication::Both { apikey, .. } | Authentication::ApiKey { apikey, .. } => {
                if self.apikey.blacklist.contains(apikey) {
                    return Ok(AuthorizationResult::Deny);
                }
                if self.apikey.whitelist.contains(apikey) {
                    return Ok(AuthorizationResult::Allow);
                }
                Ok(AuthorizationResult::Deny)
            }
            Authentication::Jwt { jwt } => {
                for wandb in &self.jwt {
                    let a = match wandb.path.resolve(&jwt.claims) {
                        Ok(a) => a,
                        Err(_) => {
                            if wandb.required {
                                return Ok(AuthorizationResult::Deny);
                            }
                            continue;
                        }
                    };
                    let vals = match wandb.r#type {
                        JwtContextPointerType::String => {
                            a.as_str().map(|x| vec![x]).ok_or_else(|| {
                                AnyError::error(format!("jwt `{}` is not a string", wandb.path))
                            })?
                        }
                        JwtContextPointerType::StringArray => a
                            .as_array()
                            .ok_or_else(|| {
                                AnyError::error(format!("jwt `{}` is not an array", wandb.path))
                            })?
                            .iter()
                            .map(|x| {
                                x.as_str().ok_or_else(|| {
                                    AnyError::error(format!(
                                        "jwt `{}` contains non-string element",
                                        wandb.path
                                    ))
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    };
                    for val in vals {
                        if wandb.blacklist.contains(val) {
                            return Ok(AuthorizationResult::Deny);
                        }
                        if wandb.whitelist.contains(val) {
                            return Ok(AuthorizationResult::Allow);
                        }
                    }
                }
                Ok(AuthorizationResult::Deny)
            }
            Authentication::NoAuth => Ok(AuthorizationResult::Unauthorized),
        }
    }
}
