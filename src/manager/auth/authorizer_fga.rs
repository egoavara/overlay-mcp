use anyhow::{Context, Result};
use json_patch::jsonptr::PointerBuf;
use jsonwebtoken::TokenData;
use rmcp::model::{
    ClientJsonRpcMessage, ClientRequest, ConstString, JsonRpcRequest, ServerJsonRpcMessage,
};

use crate::{fga::Fga, utils::AnyError};

use super::{
    authorizer::AuthorizerTrait, Authentication, AuthorizationResult, AuthorizerFgaConfig,
    JwtContext, JwtContextPointerType,
};

pub struct AuthorizerFga {
    pub engine: Fga,

    pub check_group: String,
    pub check_relation: String,

    pub apikey_group: String,

    pub jwt_group: String,
    pub jwt_claim_path: PointerBuf,
    pub jwt_context_fields: Vec<JwtContext>,
}
impl AuthorizerFga {
    pub async fn new(config: &AuthorizerFgaConfig) -> Result<Self> {
        let engine = Fga::init(config.url.clone(), config.store.clone(), &config.headers).await?;
        Ok(Self {
            engine,
            apikey_group: config.apikey.group.clone(),
            jwt_group: config.jwt.group.clone(),
            check_group: config.check.group.clone(),
            check_relation: config.check.relation.clone(),
            jwt_claim_path: config.jwt.claim_path.clone(),
            jwt_context_fields: config.jwt.context_fields.clone(),
        })
    }
}

impl AuthorizerTrait for AuthorizerFga {
    async fn authorize_authentication(
        &self,
        target: &Authentication,
    ) -> Result<AuthorizationResult, AnyError> {
        self.authorize_is_authenticated(target, ".authenticated")
            .await
    }
    async fn authorize_client_message(
        &self,
        target: &Authentication,
        message: &ClientJsonRpcMessage,
    ) -> Result<AuthorizationResult, AnyError> {
        let Some(prepare) = self.build(target)? else {
            return Ok(AuthorizationResult::Unauthorized);
        };
        match message {
            ClientJsonRpcMessage::Request(JsonRpcRequest {
                request: ClientRequest::CallToolRequest(call_tool),
                ..
            }) => {
                let method = to_const_string(&call_tool.method);
                let tool_name = call_tool.params.name.as_ref();

                let target = format!("{}:{}/{}", self.check_group, method, tool_name);
                let allow = self
                    .engine
                    .check(
                        (
                            prepare.user.clone(),
                            self.check_relation.clone(),
                            target.clone(),
                        ),
                        prepare.context.clone(),
                    )
                    .await?;
                match allow {
                    true => Ok(AuthorizationResult::Allow),
                    false => Ok(AuthorizationResult::Deny),
                }
            }
            _ => Ok(AuthorizationResult::Allow),
        }
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

fn to_const_string<M: ConstString>(_: &M) -> &'static str {
    M::VALUE
}

pub struct FgaPrepare {
    pub user: String,
    pub context: Vec<(String, String, String)>,
}

impl AuthorizerFga {
    async fn authorize_is_authenticated(
        &self,
        target: &Authentication,
        check_name: &str,
    ) -> Result<AuthorizationResult, AnyError> {
        let Some(prepare) = self.build(target)? else {
            return Ok(AuthorizationResult::Unauthorized);
        };

        let target = format!("{}:{}", self.check_group, check_name);
        let allow = self
            .engine
            .check(
                (
                    prepare.user.clone(),
                    self.check_relation.clone(),
                    target.clone(),
                ),
                prepare.context.clone(),
            )
            .await?;
        match allow {
            true => Ok(AuthorizationResult::Allow),
            false => Ok(AuthorizationResult::Deny),
        }
    }

    fn build(&self, target: &Authentication) -> Result<Option<FgaPrepare>, AnyError> {
        match target {
            Authentication::Both { apikey, .. } | Authentication::ApiKey { apikey, .. } => {
                Ok(Some(FgaPrepare {
                    user: format!("{}:{}", self.apikey_group, apikey),
                    context: Vec::new(),
                }))
            }
            Authentication::Jwt { jwt } => {
                let user = self.build_jwt_user(jwt)?;
                let context = self.build_jwt_context(&user, jwt)?;
                Ok(Some(FgaPrepare { user, context }))
            }
            Authentication::NoAuth => Ok(None),
        }
    }

    fn build_jwt_user(&self, jwt: &TokenData<serde_json::Value>) -> Result<String, AnyError> {
        let s = self
            .jwt_claim_path
            .resolve(&jwt.claims)
            .context("jwt_id not found")?;
        s.as_str()
            .map(|x| format!("{}:{}", self.jwt_group, x))
            .ok_or_else(|| AnyError::error("jwt_id is not a string"))
    }
    fn build_jwt_context(
        &self,
        user: &str,
        jwt: &TokenData<serde_json::Value>,
    ) -> Result<Vec<(String, String, String)>, AnyError> {
        let mut context = Vec::new();
        for field in &self.jwt_context_fields {
            let value = field
                .path
                .resolve(&jwt.claims)
                .context("jwt_context_field not found")?;
            match field.r#type {
                JwtContextPointerType::String => {
                    let field_value_str = value
                        .as_str()
                        .ok_or_else(|| AnyError::error("jwt_id is not a string"))?;
                    context.push((
                        user.to_string(),
                        field.relation.clone(),
                        format!("{}:{}", field.group, field_value_str),
                    ));
                }
                JwtContextPointerType::StringArray => {
                    let field_values = value
                        .as_array()
                        .context("jwt_context_field is not an array")?;
                    for field_value in field_values {
                        let field_value_str = field_value
                            .as_str()
                            .context("jwt_context_field is not a string")?;
                        context.push((
                            user.to_string(),
                            field.relation.clone(),
                            format!("{}:{}", field.group, field_value_str),
                        ));
                    }
                }
            }
        }
        Ok(context)
    }
}
