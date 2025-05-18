
use axum::{
    body::Body,
    extract::{FromRequestParts, Request},
    response::Response,
};
use http::{request::Parts, StatusCode};
use overlay_mcp_auth::{Authn, Authz};
use overlay_mcp_core::{Authentication, GeneralAuthn};
use tower::{Layer, Service};

pub struct HttpAuthentication(pub Authentication);

impl<S> FromRequestParts<S> for HttpAuthentication
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let authn = parts.extensions.get::<Authn>().ok_or_else(|| {
            tracing::error!("auth middleware not found");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
        let authn = authn.authenticate(parts).await.map_err(|err| {
            tracing::error!(error = ?err, "http authentication failed");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
        Ok(Self(authn))
    }
}

#[derive(Clone)]
pub struct AuthLayer {
    pub(crate) authz: Authz,
    pub(crate) authn: Authn,
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    pub(crate) authz: Authz,
    pub(crate) authn: Authn,
}

impl AuthLayer {
    pub fn new(authz: Authz, authn: Authn) -> Self {
        Self { authz, authn }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            authz: self.authz.clone(),
            authn: self.authn.clone(),
        }
    }
}

impl<S> Service<Request> for AuthMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, mut req: Request) -> Self::Future {
        let exts = req.extensions_mut();
        exts.insert(self.authz.clone());
        exts.insert(self.authn.clone());
        self.inner.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
}
