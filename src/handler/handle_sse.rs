use std::str::FromStr;

use anyhow::Context;
use axum::{
    body::Body,
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use http::Uri;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use crate::middleware::OptJwtClaim;

use super::{utils::AnyResult, AppState};

pub(crate) async fn handler(
    State(state): State<AppState>,
    OptJwtClaim(jwt_claim): OptJwtClaim,
    req: Request<Body>,
) -> AnyResult<Response<Body>> {
    let (mut parts, body) = req.into_parts();
    match jwt_claim {
        Some(_) => {
            
        }
        None => {
            return Ok(Response::builder().status(401).body(Body::empty()).unwrap());
        }
    }

    let mut target_part = Uri::from_str(state.config.server.upstream.as_str()).unwrap().into_parts();
    target_part.path_and_query = parts.uri.path_and_query().cloned();
    parts.uri = Uri::from_parts(target_part).unwrap();

    let req = Request::from_parts(parts, body);
    let client = hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
        .build(HttpConnector::new());
    Ok(client
        .request(req)
        .await
        .context("failed to request upstream")?.into_response())
}
