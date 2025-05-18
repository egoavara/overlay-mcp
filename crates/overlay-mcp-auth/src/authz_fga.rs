use std::sync::Arc;

use openfga::{CheckBody, CheckResponse, ContextualTuple, Openfga, Tuple};
use overlay_mcp_core::{
    auth::{
        ApikeyTupleConfig, AuthorizerFgaConfig, FgaCheckConfig, JwtContextPointerType,
        JwtTupleConfig,
    },
    Authentication, AuthorizationResult, Error, Error401, FatalError, GeneralAuthz,
};
use rmcp::model::{ClientJsonRpcMessage, ClientRequest, JsonRpcRequest, ServerJsonRpcMessage};

#[derive(Clone)]
pub struct OpenfgaAuthz {
    openfga: Openfga,
    config: Arc<OpenfgaAuthzConfig>,
}
pub struct OpenfgaAuthzConfig {
    check: FgaCheckConfig,
    apikey: ApikeyTupleConfig,
    jwt: JwtTupleConfig,
}

impl OpenfgaAuthz {
    pub async fn new(config: &AuthorizerFgaConfig) -> Result<Self, Error> {
        let mut builder = Openfga::build(config.url.clone(), config.store.clone());
        for (key, value) in config.headers.iter() {
            builder = builder.with_header(key, value);
        }
        let openfga = builder.connect().await?;
        Ok(Self {
            openfga,
            config: Arc::new(OpenfgaAuthzConfig {
                check: config.check.clone(),
                apikey: config.apikey.clone(),
                jwt: config.jwt.clone(),
            }),
        })
    }

    fn field_as_str<'a>(&self, field_value: &'a serde_json::Value) -> Result<&'a str, Error> {
        field_value
            .as_str()
            .ok_or(Error::Fatal(FatalError::UnexpectedClaimType(
                JwtContextPointerType::String,
            )))
    }

    fn field_as_str_array<'a>(
        &self,
        field_value: &'a serde_json::Value,
    ) -> Result<Vec<&'a str>, Error> {
        field_value
            .as_array()
            .ok_or(Error::Fatal(FatalError::UnexpectedClaimType(
                JwtContextPointerType::StringArray,
            )))
            .and_then(|v| {
                v.iter()
                    .map(|v| self.field_as_str(v))
                    .collect::<Result<Vec<_>, _>>()
            })
    }

    fn to_authz_result(res: &CheckResponse) -> AuthorizationResult {
        if res.allowed {
            AuthorizationResult::Allow
        } else {
            AuthorizationResult::Deny
        }
    }

    fn build_tuple_user(
        &self,
        user: &Authentication,
        object_value: &str,
    ) -> Result<CheckBody, Error> {
        match user {
            Authentication::ApiKey { apikey, .. } => {
                let user = format!("{}:{}", self.config.apikey.group, apikey);
                Ok(CheckBody {
                    tuple_key: Tuple {
                        user,
                        relation: self.config.check.relation.clone(),
                        object: format!("{}:{}", self.config.check.group, object_value),
                    },
                    contextual_tuples: ContextualTuple { tuple_keys: vec![] },
                    consistency: None,
                })
            }
            Authentication::Jwt { jwt } => {
                let field_value =
                    self.config
                        .jwt
                        .claim_path
                        .resolve(&jwt.claims)
                        .map_err(|_| {
                            Error::Fatal(FatalError::ClaimPathNotFound(
                                self.config.jwt.claim_path.clone(),
                            ))
                        })?;
                let field_valus_str = self.field_as_str(field_value)?;
                let user = format!("{}:{}", self.config.jwt.group, field_valus_str);
                let mut context: Vec<Tuple> = Vec::new();
                for field in self.config.jwt.context_fields.iter() {
                    match field.r#type {
                        JwtContextPointerType::String => {
                            let field_value = field.path.resolve(&jwt.claims).map_err(|_| {
                                Error::Fatal(FatalError::ClaimPathNotFound(field.path.clone()))
                            })?;
                            let field_valus_str = self.field_as_str(field_value)?;
                            context.push(Tuple {
                                user: user.clone(),
                                relation: field.relation.clone(),
                                object: format!("{}:{}", field.group, field_valus_str),
                            });
                        }
                        JwtContextPointerType::StringArray => {
                            let field_value = field.path.resolve(&jwt.claims).map_err(|_| {
                                Error::Fatal(FatalError::ClaimPathNotFound(field.path.clone()))
                            })?;
                            let field_valus_arr_str = self.field_as_str_array(field_value)?;
                            for field_valus_str in field_valus_arr_str {
                                context.push(Tuple {
                                    user: user.clone(),
                                    relation: field.relation.clone(),
                                    object: format!("{}:{}", field.group, field_valus_str),
                                });
                            }
                        }
                    }
                }

                Ok(CheckBody {
                    tuple_key: Tuple {
                        user,
                        relation: self.config.check.relation.clone(),
                        object: format!("{}:{}", self.config.check.group, object_value),
                    },
                    contextual_tuples: ContextualTuple {
                        tuple_keys: context,
                    },
                    consistency: None,
                })
            }
            Authentication::NoAuth => Err(Error::Unauthorized(Error401::AuthenticationFailed)),
        }
    }
}

impl GeneralAuthz for OpenfgaAuthz {
    async fn authorize_enter(&self, target: &Authentication) -> Result<AuthorizationResult, Error> {
        let tuple = self.build_tuple_user(target, ".system/enter")?;
        let check_resp = self.openfga.check(None, tuple).await?;
        Ok(Self::to_authz_result(&check_resp))
    }

    async fn authorize_client_message(
        &self,
        target: &Authentication,
        message: &ClientJsonRpcMessage,
    ) -> Result<AuthorizationResult, Error> {
        match message {
            ClientJsonRpcMessage::Request(JsonRpcRequest {
                request: ClientRequest::CallToolRequest(tool_req),
                ..
            }) => {
                let tuple =
                    self.build_tuple_user(target, &format!("tools/call/{}", tool_req.params.name))?;
                let check_resp = self.openfga.check(None, tuple).await?;
                Ok(Self::to_authz_result(&check_resp))
            }
            _ => Ok(AuthorizationResult::Allow),
        }
    }
    async fn authorize_server_message(
        &self,
        _target: &Authentication,
        _message: &ServerJsonRpcMessage,
    ) -> Result<AuthorizationResult, Error> {
        // TODO: Implement server message authorization
        Ok(AuthorizationResult::Allow)
    }
}
