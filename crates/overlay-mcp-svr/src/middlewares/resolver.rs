use axum::extract::Request;
use overlay_mcp_resolver::Resolver;
use tower::{Layer, Service};

#[derive(Clone)]
pub struct ResolverLayer {
    resolver: Resolver,
}

#[derive(Clone)]
pub struct ResolverMiddleware<S> {
    inner: S,
    resolver: Resolver,
}

impl ResolverLayer {
    pub fn new(resolver: Resolver) -> Self {
        Self { resolver }
    }
}

impl<S> Layer<S> for ResolverLayer {
    type Service = ResolverMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ResolverMiddleware {
            inner,
            resolver: self.resolver.clone(),
        }
    }
}

impl<S> Service<Request> for ResolverMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, mut req: Request) -> Self::Future {
        let exts = req.extensions_mut();
        exts.insert(self.resolver.clone());
        self.inner.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
}
