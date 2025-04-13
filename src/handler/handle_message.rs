use std::str::FromStr;

use anyhow::Context;
use axum::{body::Body, extract::{Request, State}, response::{IntoResponse, Response}};
use http::Uri;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use crate::authorizer::{AuthorizerResponse, CheckAuthorizer};

use super::{utils::AnyResult, AppState};

pub(crate) async fn handler(
    State(state): State<AppState>,
    CheckAuthorizer(authorizer, code): CheckAuthorizer,
    req: Request<Body>,
) -> AnyResult<Response<Body>> {
    match authorizer {
        AuthorizerResponse::Allow(_) => {}
        AuthorizerResponse::Deny(deny) => {
            tracing::info!("{}", deny.reason.unwrap_or("No reason".to_string()));
            return Ok(Response::builder().status(code).body(Body::empty()).unwrap());
        }
    }

    let (mut parts, body) = req.into_parts();
    let mut target_part = Uri::from_str(state.config.server.upstream.as_str()).unwrap().into_parts();
    target_part.path_and_query = parts.uri.path_and_query().cloned();

    // 원본 요청에서 파싱한 요청 정보를 사용하여 새로운 Uri 생성
    parts.uri = Uri::from_parts(target_part).unwrap();

    let req = Request::from_parts(parts, body);
    let client = hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
        .build(HttpConnector::new());
    Ok(client
        .request(req)
        .await
        .context("failed to request upstream")?
        .into_response())
}
