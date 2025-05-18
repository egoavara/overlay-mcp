use axum::{
    body::Body,
    extract::{FromRequestParts, OptionalFromRequestParts, Request},
    response::Response,
};
use http::{request::Parts, StatusCode};
use overlay_mcp_core::{MCP};
use overlay_mcp_session_manager::SessionManager;

use std::{fmt::Display, marker::PhantomData};
use tower::{Layer, Service};

pub struct HttpSessionId<Spec: MCP> {
    pub session_id: String,
    _phantom: PhantomData<Spec>,
}

impl<Spec: MCP> HttpSessionId<Spec> {
    pub fn as_str(&self) -> &str {
        &self.session_id
    }
}

impl<Spec: MCP> Display for HttpSessionId<Spec> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.session_id)
    }
}

impl<Spec, S> FromRequestParts<S> for HttpSessionId<Spec>
where
    Spec: MCP,
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let Some(session_id) = Spec::pick_session_id(parts) else {
            return Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("no session id"))
                .unwrap());
        };
        Ok(Self {
            session_id: session_id.to_string(),
            _phantom: PhantomData,
        })
    }
}

impl<Spec, S> OptionalFromRequestParts<S> for HttpSessionId<Spec>
where
    Spec: MCP,
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Option<Self>, Self::Rejection> {
        let Some(session_id) = Spec::pick_session_id(parts) else {
            return Ok(None);
        };
        Ok(Some(Self {
            session_id: session_id.to_string(),
            _phantom: PhantomData,
        }))
    }
}

#[derive(Clone)]
pub struct SessionManagerLayer {
    session_manager: SessionManager,
}

#[derive(Clone)]
pub struct SessionManagerMiddleware<S> {
    inner: S,
    session_manager: SessionManager,
}

impl SessionManagerLayer {
    pub fn new(session_manager: SessionManager) -> Self {
        Self { session_manager }
    }
}

impl<S> Layer<S> for SessionManagerLayer {
    type Service = SessionManagerMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionManagerMiddleware {
            inner,
            session_manager: self.session_manager.clone(),
        }
    }
}

impl<S> Service<Request> for SessionManagerMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, mut req: Request) -> Self::Future {
        let exts = req.extensions_mut();
        exts.insert(self.session_manager.clone());
        self.inner.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
}
