use axum::extract::Request;
use tower::{Layer, Service};

#[derive(Clone)]
pub struct ReqwestLayer {
    client: reqwest::Client,
}

#[derive(Clone)]
pub struct ReqwestMiddleware<S> {
    inner: S,
    client: reqwest::Client,
}

impl ReqwestLayer {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl<S> Layer<S> for ReqwestLayer {
    type Service = ReqwestMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ReqwestMiddleware {
            inner,
            client: self.client.clone(),
        }
    }
}

impl<S> Service<Request> for ReqwestMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, mut req: Request) -> Self::Future {
        let exts = req.extensions_mut();
        exts.insert(self.client.clone());
        self.inner.call(req)
    }

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
}
