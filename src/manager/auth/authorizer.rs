use std::sync::Arc;

use crate::utils::AnyError;
use anyhow::Result;
use axum::extract::Request;
use enum_dispatch::enum_dispatch;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use tower::{Layer, Service};

use super::{
    authorizer_constant::AuthorizerConstant, authorizer_fga::AuthorizerFga, AuthConfig,
    Authentication, AuthorizationResult,
};

#[enum_dispatch]
pub trait AuthorizerTrait {
    async fn authorize_authentication(
        &self,
        target: &Authentication,
    ) -> Result<AuthorizationResult, AnyError>;

    async fn authorize_client_message(
        &self,
        target: &Authentication,
        message: &ClientJsonRpcMessage,
    ) -> Result<AuthorizationResult, AnyError>;

    async fn authorize_server_message(
        &self,
        target: &Authentication,
        message: &ServerJsonRpcMessage,
    ) -> Result<AuthorizationResult, AnyError>;
}

pub type Authorizer = Arc<InnerAuthorier>;

#[enum_dispatch(AuthorizerTrait)]
pub enum InnerAuthorier {
    Constant(AuthorizerConstant),
    Fga(AuthorizerFga),
}

#[derive(Clone)]
pub struct AuthorizerLayer {
    pub(crate) authorizer: Authorizer,
}

#[derive(Clone)]
pub struct AuthorizerMiddleware<S> {
    inner: S,
    pub(crate) authorizer: Authorizer,
}

impl AuthorizerLayer {
    pub async fn new(config: &AuthConfig) -> Result<Self> {
        let authorizer = match config {
            AuthConfig::Constant { constant, .. } => {
                InnerAuthorier::Constant(AuthorizerConstant::new(constant))
            }
            AuthConfig::OpenFga { openfga, .. } => {
                InnerAuthorier::Fga(AuthorizerFga::new(openfga).await?)
            }
        };
        Ok(Self {
            authorizer: Arc::new(authorizer),
        })
    }
}

impl<S> Layer<S> for AuthorizerLayer {
    type Service = AuthorizerMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthorizerMiddleware {
            inner,
            authorizer: self.authorizer.clone(),
        }
    }
}

impl<S> Service<Request> for AuthorizerMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, mut req: Request) -> Self::Future {
        req.extensions_mut().insert(self.authorizer.clone());
        self.inner.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
}
