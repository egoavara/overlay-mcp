mod authn_basic;
mod authz_fga;
mod authz_static;

pub use authn_basic::*;
use authz_fga::OpenfgaAuthz;
pub use authz_static::*;
use axum::http::request;
use overlay_mcp_core::{
    AuthConfig, Authentication, AuthorizationResult, Config, Error, GeneralAuthn, GeneralAuthz,
};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};

#[derive(Clone)]
pub enum Authz {
    Static(StaticAuthz),
    OpenFga(OpenfgaAuthz),
}

#[derive(Clone)]
pub enum Authn {
    Basic(AuthnBasic),
}

impl GeneralAuthz for Authz {
    async fn authorize_enter(&self, target: &Authentication) -> Result<AuthorizationResult, Error> {
        match self {
            Authz::Static(static_authz) => static_authz.authorize_enter(target).await,
            Authz::OpenFga(openfga_authz) => openfga_authz.authorize_enter(target).await,
        }
    }

    async fn authorize_client_message(
        &self,
        target: &Authentication,
        message: &ClientJsonRpcMessage,
    ) -> Result<AuthorizationResult, Error> {
        match self {
            Authz::Static(authz) => authz.authorize_client_message(target, message).await,
            Authz::OpenFga(openfga_authz) => {
                openfga_authz
                    .authorize_client_message(target, message)
                    .await
            }
        }
    }

    async fn authorize_server_message(
        &self,
        target: &Authentication,
        message: &ServerJsonRpcMessage,
    ) -> Result<AuthorizationResult, Error> {
        match self {
            Authz::Static(authz) => authz.authorize_server_message(target, message).await,
            Authz::OpenFga(openfga_authz) => {
                openfga_authz
                    .authorize_server_message(target, message)
                    .await
            }
        }
    }
}

impl GeneralAuthn for Authn {
    fn create_oauth_client(
        &self,
    ) -> oauth2::basic::BasicClient<
        oauth2::EndpointSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointMaybeSet,
    > {
        match self {
            Authn::Basic(authn) => authn.create_oauth_client(),
        }
    }

    fn issuer_url(&self) -> url::Url {
        match self {
            Authn::Basic(authn) => authn.issuer_url(),
        }
    }

    fn scopes(&self) -> Vec<oauth2::Scope> {
        match self {
            Authn::Basic(authn) => authn.scopes(),
        }
    }

    async fn authenticate(&self, target: &request::Parts) -> Result<Authentication, Error> {
        match self {
            Authn::Basic(authn) => authn.authenticate(target).await,
        }
    }
}

impl Authn {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        let authn_config = config.auth.get_authenticater();
        let authn = AuthnBasic::new(authn_config).await?;
        Ok(Authn::Basic(authn))
    }
}

impl Authz {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        match &config.auth {
            AuthConfig::OpenFga { openfga, .. } => {
                let authz = OpenfgaAuthz::new(openfga).await?;
                Ok(Authz::OpenFga(authz))
            }
            AuthConfig::Static { constant, .. } => {
                let authz = StaticAuthz::new(constant).await?;
                Ok(Authz::Static(authz))
            }
        }
    }
}
