use crate::{
    Authentication, AuthorizationResult, BypassDownstream, Downstream, Error, SessionGuard,
    StreamGuard, Upstream,
};
use oauth2::{basic::BasicClient, EndpointMaybeSet, EndpointNotSet, EndpointSet, Scope};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use std::{borrow::Cow, future::Future};
use url::Url;

pub trait GeneralSession: Sync {
    fn session_id(&self) -> Cow<str>;

    fn upstream_url(&self) -> Cow<Url>;

    fn guard_upstream(&self) -> impl Future<Output = Result<StreamGuard<Upstream>, Error>> + Send;
    fn guard_downstream(
        &self,
    ) -> impl Future<Output = Result<StreamGuard<Downstream>, Error>> + Send;
    fn guard_bypass_downstream(
        &self,
    ) -> impl Future<Output = Result<StreamGuard<BypassDownstream>, Error>> + Send;
    fn guard_close(&self) -> impl Future<Output = Result<SessionGuard, Error>> + Send;

    fn is_started(&self) -> impl Future<Output = bool> + Send;
    fn start(
        &self,
        original_request: &http::request::Parts,
    ) -> impl Future<Output = Result<(), Error>> + Send;
    fn stop(&self) -> impl Future<Output = Result<(), Error>> + Send;
    fn close(&self) -> impl Future<Output = Result<(), Error>> + Send;

    fn ensure_started(
        &self,
        original_request: &http::request::Parts,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            if self.is_started().await {
                Ok(())
            } else {
                self.start(original_request).await
            }
        }
    }
}

pub trait GeneralSessionManager {
    type Session: GeneralSession;

    // session
    fn create(
        &self,
        upstream_url: Url,
    ) -> impl Future<Output = Result<Self::Session, Error>> + Send;
    fn find(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<Option<Self::Session>, Error>> + Send;
}

pub trait GeneralAuthn {
    fn create_oauth_client(
        &self,
    ) -> BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet>;

    fn issuer_url(&self) -> Url;

    fn scopes(&self) -> Vec<Scope>;

    fn authenticate(
        &self,
        target: &http::request::Parts,
    ) -> impl Future<Output = Result<Authentication, Error>>;
}

pub trait GeneralAuthz {
    fn authorize_enter(
        &self,
        target: &Authentication,
    ) -> impl Future<Output = Result<AuthorizationResult, Error>>;

    fn authorize_client_message(
        &self,
        target: &Authentication,
        message: &ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<AuthorizationResult, Error>>;

    fn authorize_server_message(
        &self,
        target: &Authentication,
        message: &ServerJsonRpcMessage,
    ) -> impl Future<Output = Result<AuthorizationResult, Error>>;
}

pub trait GeneralResolver {
    // route
    fn resolve(
        &self,
        target: &http::request::Parts,
    ) -> impl Future<Output = Result<Url, Error>> + Send;
}
