use std::{str::FromStr, time::Duration};

use axum::{
    body::Body,
    debug_handler,
    extract::{Request, State},
    response::{
        sse::Event, Response, Sse,
    },
};
use eventsource_client::{Client, SSE};
use futures_util::{Stream, StreamExt, TryStreamExt};
use http::{Method, StatusCode, Uri};
use url::Url;

use crate::{
    authorizer::{AuthorizerResponse, CheckAuthorizer},
    utils::{HttpComponent, PassthroughState},
};

use super::AppState;

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

    let (src, _) = req.into_parts();
    let mut dst_builder = PassthroughState::new(src);
    for passthrough in &state.config.application.passthrough {
        dst_builder = dst_builder.passing(passthrough).map_err(|e| {
            tracing::error!("failed to passthrough: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
    }
    let dst = dst_builder
        .empty_end(
            Method::GET,
            Url::from_str(state.config.server.upstream.as_str()).unwrap(),
        )
        .map_err(|e| {
            tracing::error!("failed to passthrough: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;

    let mut client =
        eventsource_client::ClientBuilder::for_url(&dst.uri().to_string()).map_err(|e| {
            tracing::error!("failed to create eventsource client: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })?;
    if let Some(last_event_id) = dst.headers().get("last-event-id") {
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

    for (name, value) in dst.headers().iter() {
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
                let url = match Url::parse(&event.data) {
                    Ok(url) => url,
                    Err(url::ParseError::RelativeUrlWithoutBase) => {
                        match hostname.join(&event.data) {
                            Ok(url) => url,
                            Err(err) => {
                                tracing::error!(
                                    data = event.data,
                                    error = ?err,
                                    "failed to join url"
                                );
                                return Ok(response.event(event.event_type).data(event.data));
                            }
                        }
                    }
                    Err(_) => {
                        tracing::error!("failed to parse url: {}", event.data);
                        return Ok(response.event(event.event_type).data(event.data));
                    }
                };
                let mut endpoint = hostname.clone();
                endpoint.set_path(url.path());
                endpoint.set_query(url.query());
                if let Some((apikey, HttpComponent::Query { name })) = &meta.apikey_from {
                    endpoint.query_pairs_mut().append_pair(name, apikey);
                }
                let uri = Uri::from_str(endpoint.as_str()).unwrap();
                return Ok(response.event(event.event_type).data(uri.path_and_query().unwrap().to_string()));
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
