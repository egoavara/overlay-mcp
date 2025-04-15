use std::{collections::HashMap, convert::Infallible, str::FromStr, time::Duration};

use anyhow::Context;
use axum::{
    body::Body,
    debug_handler,
    extract::{Query, Request, State},
    response::{
        self,
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
};
use axum_extra::either::Either;
use eventsource_client::{Client, SSE};
use futures_util::{Stream, StreamExt, TryStreamExt};
use http::{header, HeaderName, StatusCode, Uri};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use serde::Deserialize;
use url::Url;

use crate::{
    authorizer::{AuthorizerResponse, CheckAuthorizer},
    middleware::ApikeyExtractor,
};

use super::{utils::AnyResult, AppState};

#[debug_handler]
pub(crate) async fn handler(
    State(state): State<AppState>,
    CheckAuthorizer(authorizer, meta): CheckAuthorizer,
    req: Request<Body>,
) -> Result<Sse<impl Stream<Item = Result<Event, anyhow::Error>>>, Response<Body>> {
    match authorizer {
        AuthorizerResponse::Allow(_) => {}
        AuthorizerResponse::Deny(deny) => {
            tracing::info!("{}", deny.reason.unwrap_or("No reason".to_string()));
            return Err(Response::builder()
                .status(meta.expected_status_code)
                .body(Body::empty())
                .unwrap());
        }
    }

    let hostname = state.config.server.hostname.clone();

    let (mut parts, _) = req.into_parts();
    let mut target_uri = Url::from_str(state.config.server.upstream.as_str()).unwrap();
    target_uri.set_path(parts.uri.path());
    parts.uri = Uri::from_str(&target_uri.to_string()).unwrap();

    if let Some((_, extractor)) = &meta.apikey_from {
        parts = extractor.destruct(parts);
    } else {
        target_uri.set_query(parts.uri.query());
    }

    let mut client =
        eventsource_client::ClientBuilder::for_url(&parts.uri.to_string()).map_err(|e| {
            tracing::error!("failed to create eventsource client: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
    if let Some(last_event_id) = parts.headers.get("last-event-id") {
        client = client.last_event_id(
            last_event_id
                .to_str()
                .map_err(|err| {
                    tracing::error!("failed to get last-event-id: {}", err);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })?
                .to_string(),
        );
    }
    for (name, value) in parts.headers.iter() {
        if name.as_str() == "last-event-id" {
            continue;
        }
        let value = value.to_str().map_err(|e| {
            tracing::error!("failed to convert header value to string: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
        client = client.header(name.as_str(), value).map_err(|e| {
            tracing::error!("failed to set header: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
    }
    let mut downstream = client.build().stream();

    let conn = downstream
        .by_ref()
        .map(|event| match event {
            Result::Ok(SSE::Connected(event)) => {
                tracing::info!("connected to {:?}", &event);
                Ok(event)
            }
            Result::Ok(else_event) => {
                tracing::error!("comment: {:?}", else_event);
                Err(anyhow::anyhow!("comment: {:?}", else_event))
            }
            Result::Err(err) => {
                tracing::error!("error: {:?}", err);
                Err(anyhow::anyhow!("error: {:?}", err))
            }
        })
        .try_next()
        .await
        .map_err(|err| {
            tracing::error!("error: {:?}", err);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?
        .ok_or_else(|| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
    let host = parts
        .headers
        .get(header::HOST)
        .map(|x| x.to_str())
        .transpose()
        .map_err(|err| {
            tracing::error!("failed to get host: {}", err);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?
        .ok_or_else(|| {
            tracing::error!("failed to get host, not exists");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?
        .to_string();

    tracing::info!(host = ?parts.headers.get(header::HOST));
    let left_stream = downstream.map(move |event| match event {
        Result::Ok(SSE::Event(event)) => {
            tracing::info!("event: {:?}", event);
            let mut response = Event::default();
            if let Some(id) = &event.id {
                response = response.id(id);
            }
            if let Some(retry) = event.retry {
                response = response.retry(Duration::from_millis(retry));
            }
            if event.event_type == "endpoint" {
                let mut url = Url::parse(&event.data).unwrap();
                let mut endpoint = hostname.clone();
                endpoint.set_path(url.path());
                endpoint.set_query(url.query());
                if let Some((apikey, ApikeyExtractor::Query { name })) = &meta.apikey_from {
                    endpoint.query_pairs_mut().append_pair(name, apikey);
                }
                return Ok(response.event(event.event_type).data(endpoint));
            }
            Ok(response.event(event.event_type).data(event.data))
        }
        Result::Ok(SSE::Comment(comment)) => {
            tracing::info!("comment: {}", comment);
            Ok(Event::default().comment(comment))
        }
        Result::Ok(SSE::Connected(conn)) => {
            tracing::warn!("unreachable event: {:?}", conn);
            Ok(Event::default().comment("unreachable"))
        }
        Result::Err(err) => {
            tracing::error!("error: {:?}", err);
            Err(anyhow::anyhow!("error: {:?}", err))
        }
    });
    let response = Sse::new(left_stream);
    Result::Ok(response)
}
