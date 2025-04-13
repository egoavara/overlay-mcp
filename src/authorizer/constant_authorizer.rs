use super::authorizer::{
    AuthorizerComponent, AuthorizerRequest, AuthorizerResponseAllow, AuthorizerResponseDeny,
};
use futures_util::Stream;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_with::{formats::PreferOne, serde_as, OneOrMany};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConstantAuthorizer {
    pub ip: Option<IpAuthorizer>,
    pub jwt: Option<JwtAuthorizer>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpAuthorizer {
    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub whitelist: Vec<IpNet>,
    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub blacklist: Vec<IpNet>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtAuthorizer {
    #[serde(default)]
    pub required: bool,

    #[serde(default = "default_true")]
    pub allow_all: bool,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub fields: Vec<JwtByField>,

    pub group: Option<JwtByGroup>,
}
fn default_true() -> bool {
    true
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtByField {
    #[serde(default = "default::jwt_by_field")]
    pub field: json_patch::jsonptr::PointerBuf,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub whitelist: Vec<String>,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub blacklist: Vec<String>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtByGroup {
    #[serde(default = "default::jwt_by_group_field")]
    pub field: json_patch::jsonptr::PointerBuf,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub whitelist: Vec<String>,

    #[serde(default)]
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub blacklist: Vec<String>,
}

pub(crate) mod default {
    use std::str::FromStr;

    pub(crate) fn jwt_by_group_field() -> json_patch::jsonptr::PointerBuf {
        json_patch::jsonptr::PointerBuf::from_str("/groups").unwrap()
    }
    pub(crate) fn jwt_by_field() -> json_patch::jsonptr::PointerBuf {
        json_patch::jsonptr::PointerBuf::from_str("/sub").unwrap()
    }
}

impl AuthorizerComponent for ConstantAuthorizer {
    fn whitelist(
        &self,
        request: &AuthorizerRequest,
    ) -> impl Stream<Item = AuthorizerResponseAllow> {
        async_stream::stream! {
            if let Some(ip) = &self.ip {
                for await allow in ip.whitelist(request) {
                    yield allow;
                }
            }
            if let Some(jwt) = &self.jwt {
                for await allow in jwt.whitelist(request) {
                    yield allow;
                }
            }
        }
    }

    fn blacklist(&self, request: &AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseDeny> {
        async_stream::stream! {
            if let Some(ip) = &self.ip {
                for await deny in ip.blacklist(request) {
                    yield deny;
                }
            }
            if let Some(jwt) = &self.jwt {
                for await deny in jwt.blacklist(request) {
                    yield deny;
                }
            }
        }
    }
}

impl AuthorizerComponent for IpAuthorizer {
    fn whitelist(
        &self,
        request: &AuthorizerRequest,
    ) -> impl Stream<Item = AuthorizerResponseAllow> {
        async_stream::stream! {
            for ip in self.whitelist.iter() {
                if ip.contains(&request.ip) {
                    yield AuthorizerResponseAllow {
                        authorizer: "ip".to_string(),
                        reason: Some(format!("IP {} is whitelisted, caused by {}", request.ip, ip)),
                    };
                }
            }
        }
    }

    fn blacklist(&self, request: &AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseDeny> {
        async_stream::stream! {
            for ip in self.blacklist.iter() {
                if ip.contains(&request.ip) {
                    yield AuthorizerResponseDeny {
                        authorizer: "ip".to_string(),
                        reason: Some(format!("IP {} is blacklisted, caused by {}", request.ip, ip)),
                    };
                }
            }
        }
    }
}

impl AuthorizerComponent for JwtAuthorizer {
    fn whitelist(
        &self,
        request: &AuthorizerRequest,
    ) -> impl Stream<Item = AuthorizerResponseAllow> {
        async_stream::stream! {
            if self.allow_all && request.jwt.is_some() {
                yield AuthorizerResponseAllow {
                    authorizer: "jwt".to_string(),
                    reason: Some("JWT is allowed".to_string()),
                };
            }
            for field in &self.fields {
                for await allow in field.whitelist(request) {
                    yield allow;
                }
            }
            if let Some(group) = &self.group {
                for await allow in group.whitelist(request) {
                    yield allow;
                }
            }
        }
    }

    fn blacklist(&self, request: &AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseDeny> {
        async_stream::stream! {
            if self.required && request.jwt.is_none() {
                yield AuthorizerResponseDeny {
                    authorizer: "jwt".to_string(),
                    reason: Some("JWT must be provided".to_string()),
                };
            }
            for field in &self.fields {
                for await deny in field.blacklist(request) {
                    yield deny;
                }
            }
            if let Some(group) = &self.group {
                for await deny in group.blacklist(request) {
                    yield deny;
                }
            }
        }
    }
}

impl AuthorizerComponent for JwtByField {
    fn whitelist(
        &self,
        request: &AuthorizerRequest,
    ) -> impl Stream<Item = AuthorizerResponseAllow> {
        async_stream::stream! {
            if let Some(value) = &request.jwt {
                match self.field.resolve(value) {
                    Ok(serde_json::Value::String(val)) => {
                        if self.whitelist.contains(val) {
                            yield AuthorizerResponseAllow {
                                authorizer: format!("jwt[{}]", self.field),
                                reason: Some(format!("JWT ({}) is whitelisted, caused by {}",self.field, val)),
                            };
                        }
                    }
                    Ok(val) => {
                        tracing::debug!("Expected string, but got {:?} by {}", val, self.field);
                    }
                    Err(err) => {
                        tracing::debug!("Error resolving JWT field: {}", err);
                    }
                }
            }
        }
    }

    fn blacklist(&self, request: &AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseDeny> {
        async_stream::stream! {
            if let Some(value) = &request.jwt {
                match self.field.resolve(value) {
                    Ok(serde_json::Value::String(val)) => {
                        if self.blacklist.contains(val) {
                            yield AuthorizerResponseDeny {
                                authorizer: format!("jwt[{}]", self.field),
                                reason: Some(format!("JWT ({}) is blacklisted, caused by {}",self.field, val)),
                            };
                        }
                    }
                    Ok(val) => {
                        tracing::debug!("Expected string, but got {:?} by {}", val, self.field);
                    }
                    Err(err) => {
                        tracing::debug!("Error resolving JWT field: {}", err);
                    }
                }
            }
        }
    }
}

impl AuthorizerComponent for JwtByGroup {
    fn whitelist(
        &self,
        request: &AuthorizerRequest,
    ) -> impl Stream<Item = AuthorizerResponseAllow> {
        async_stream::stream! {
            if let Some(value) = &request.jwt {
                match self.field.resolve(value) {
                    Ok(serde_json::Value::Array(val)) => {
                        for elem in val {
                            match elem {
                                serde_json::Value::String(val) => {
                                    if self.whitelist.contains(val) {
                                        yield AuthorizerResponseAllow {
                                            authorizer: "jwt-group".to_string(),
                                            reason: Some(format!(
                                                "JWT ({}) is whitelisted, caused by {}",
                                                self.field, val
                                            )),
                                        };
                                    }
                                }
                                _ => {
                                    tracing::debug!(
                                        "Expected string, but got {:?} by {}",
                                        elem,
                                        self.field
                                    );
                                }
                            }
                        }
                    }
                    Ok(val) => {
                        tracing::debug!("Expected string, but got {:?} by {}", val, self.field);
                    }
                    Err(err) => {
                        tracing::debug!("Error resolving JWT field: {}", err);
                    }
                }
            }
        }
    }
    fn blacklist(&self, request: &AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseDeny> {
        async_stream::stream! {
            if let Some(value) = &request.jwt {
                match self.field.resolve(value) {
                    Ok(serde_json::Value::Array(val)) => {
                        for elem in val {
                            match elem {
                                serde_json::Value::String(val) => {
                                    if self.blacklist.contains(val) {
                                        yield AuthorizerResponseDeny {
                                            authorizer: "jwt-group".to_string(),
                                            reason: Some(format!(
                                                "JWT ({}) is blacklisted, caused by {}",
                                                self.field, val
                                            )),
                                        };
                                    }
                                }
                                _ => {
                                    tracing::debug!(
                                        "Expected string, but got {:?} by {}",
                                        elem,
                                        self.field
                                    );
                                }
                            }
                        }
                    }
                    Ok(val) => {
                        tracing::debug!("Expected string, but got {:?} by {}", val, self.field);
                    }
                    Err(err) => {
                        tracing::debug!("Error resolving JWT field: {}", err);
                    }
                }
            }
        }
    }
}
