use httpbuilder::http_reference::HttpReference;
use jsonwebtoken::TokenData;

use crate::{Error, Error401, Error403};

#[derive(Debug, Clone)]
pub enum Authentication {
    ApiKey {
        apikey: String,
        apikey_from: HttpReference,
    },
    Jwt {
        jwt: TokenData<serde_json::Value>,
    },
    NoAuth,
}

pub enum AuthorizationResult {
    Allow,
    Deny,
    Unauthorized,
}

impl AuthorizationResult {
    pub fn to_err_response(&self) -> Result<(), Error> {
        match self {
            AuthorizationResult::Allow => Ok(()),
            AuthorizationResult::Deny => Err(Error::Forbidden(Error403::AuthorizationFailed)),
            AuthorizationResult::Unauthorized => {
                Err(Error::Unauthorized(Error401::AuthenticationFailed))
            }
        }
    }
}
