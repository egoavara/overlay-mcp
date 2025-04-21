use std::{str::FromStr, time::Duration};

use anyhow::Context;
use axum::{
    body::Body,
    extract::{Request, State},
    response::{sse::Event, IntoResponse, Response, Sse},
    Extension,
};
use eventsource_client::{Client, SSE};
use futures_util::{Stream, StreamExt, TryStreamExt};
use http::{Method, StatusCode, Uri};

use crate::{
    authorizer::{AuthorizerResponse, CheckAuthorizer},
    handler::AppState,
    manager::storage::{ConnectionStateCreate, ManagerTrait, StorageManager},
    middleware::{MCPProtocolVersion, MCPSessionId},
    utils::{join_endpoint, AnyError, HttpComponent, PassthroughState, ReqwestResponse},
};

pub(crate) async fn handler_upstream(
    State(state): State<AppState>,
    MCPSessionId(session_id): MCPSessionId,
    CheckAuthorizer(authorizer, meta): CheckAuthorizer,
    Extension(session_manager): Extension<StorageManager>,
    req: Request<Body>,
) -> Result<Response<Body>, AnyError> {
    match authorizer {
        AuthorizerResponse::Allow(_) => {}
        AuthorizerResponse::Deny(deny) => {
            tracing::info!("{}", deny.reason.unwrap_or("No reason".to_string()));
            return Err(Response::builder()
                .status(meta.expected_status_code)
                .body(Body::empty())
                .unwrap()
                .into());
        }
    }

    let (mut parts, body) = req.into_parts();
    let session_data = session_manager.get(session_id).await?;
    let Some(session_data) = session_data else {
        return Err(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap()
            .into());
    };

    parts.uri = Uri::from_str(session_data.upstream.as_str()).unwrap();
    let headers = parts.headers;
    tracing::info!("session_data: {:?}", parts.uri);

    let response = state
        .reqwest
        .post(session_data.upstream)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .headers(headers)
        .send()
        .await
        .context("failed to request upstream")?;
    Ok(ReqwestResponse(response).into_response())
}

pub(crate) async fn handler_downstream(
    State(state): State<AppState>,
    CheckAuthorizer(authorizer, meta): CheckAuthorizer,
    Extension(mut session_manager): Extension<StorageManager>,
    req: Request<Body>,
) -> Result<Sse<impl Stream<Item = Result<Event, anyhow::Error>>>, AnyError> {
    match authorizer {
        AuthorizerResponse::Allow(_) => {}
        AuthorizerResponse::Deny(deny) => {
            let reason = deny.reason.unwrap_or("No reason".to_string());
            tracing::info!("{}", reason);
            return Err(Response::builder()
                .status(meta.expected_status_code)
                .body(Body::empty())
                .unwrap()
                .into());
        }
    }
    let upstream = session_manager.route().await?;

    let (src, _) = req.into_parts();
    let mut dst_builder = PassthroughState::new(src);
    for passthrough in &state.config.application.passthrough {
        dst_builder = dst_builder
            .passing(passthrough)
            .context("failed to passthrough")?;
    }
    let dst = dst_builder
        .empty_end(Method::GET, upstream.clone())
        .context("failed to create passthrough")?;

    let mut client = eventsource_client::ClientBuilder::for_url(&dst.uri().to_string())
        .context("failed to create eventsource client")?;
    if let Some(last_event_id) = dst.headers().get("last-event-id") {
        client = client.last_event_id(
            last_event_id
                .to_str()
                .context("failed to convert last-event-id value to string")?
                .to_string(),
        );
    }
    client = client
        .header(
            "MCP-Protocol-Version",
            MCPProtocolVersion::V20241105.as_header_value().unwrap(),
        )
        .unwrap();
    for (name, value) in dst.headers().iter() {
        if name.as_str() == "last-event-id" {
            continue;
        }
        let value = value
            .to_str()
            .context("failed to convert header value to string")?;
        client = client
            .header(name.as_str(), value)
            .context("failed to set header")?;
    }
    let mut downstream = client.build().stream();
    let _ = event::until_connect(downstream.by_ref()).await?;
    let message = event::until_endpoint(downstream.by_ref()).await?;
    let upstream_message = join_endpoint(upstream, &message).context("failed to join url")?;
    let response = Sse::new(async_stream::stream! {
        let session_id = upstream_message
            .query_pairs()
            .find(|x| x.0 == "session_id")
            .map(|(_, v)| v.to_string())
            .unwrap_or_else(|| "".to_string());
        let session_data = session_manager
            .create(ConnectionStateCreate {
                upstream: upstream_message,
                upstream_session_id: session_id,
            })
            .await?;
        let guard = session_manager.guard(session_data.session_id);
        let mut endpoint = state
            .config
            .server
            .hostname
            .join("/mcp")
            .context("failed to join url")?;
        endpoint
            .query_pairs_mut()
            .append_pair("session_id", guard.session_id());
        if let Some((apikey, HttpComponent::Query { name })) = &meta.apikey_from {
            endpoint.query_pairs_mut().append_pair(name, apikey);
        }

        yield Ok(Event::default().event("endpoint").data(&endpoint));
        let mut remote = downstream.map(move |event| match event {
            Result::Ok(SSE::Event(event)) => {
                tracing::info!("event: {:?}", event);
                let mut response = Event::default();
                if let Some(id) = &event.id {
                    response = response.id(id);
                }
                if let Some(retry) = event.retry {
                    response = response.retry(Duration::from_millis(retry));
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
        while let Some(event) = remote.try_next().await? {
            yield Ok(event);
        }
        drop(guard);
    });
    Result::Ok(response)
}

mod event {
    use anyhow::{anyhow, Result};
    use axum::{body::Body, response::Response};
    use eventsource_client::SSE;
    use futures_util::{Stream, StreamExt, TryFutureExt, TryStreamExt};
    use http::StatusCode;
    use tokio::{
        pin,
        time::{timeout, Duration},
    };

    use crate::utils::AnyError;

    pub async fn until_connect<S: Stream<Item = Result<SSE, eventsource_client::Error>>>(
        stream: S,
    ) -> Result<eventsource_client::Response, AnyError> {
        pin!(stream);
        let conn = stream
            .map(|event| match event {
                Result::Ok(SSE::Connected(event)) => {
                    tracing::info!("connected to {:?}", &event);
                    Ok(event.response().clone())
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
                tracing::error!("no event");
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            })?;
        Ok(conn)
    }

    pub async fn until_endpoint<S: Stream<Item = Result<SSE, eventsource_client::Error>>>(
        stream: S,
    ) -> Result<String, anyhow::Error> {
        pin!(stream);

        // 스트림에서 "endpoint" 타입의 이벤트를 찾을 때까지 기다립니다.
        let endpoint_event_result = timeout(
            Duration::from_secs(5),
            stream
                .try_filter(|event| match event {
                    SSE::Event(event) => {
                        std::future::ready(event.event_type.as_str() == "endpoint")
                    }
                    _ => std::future::ready(false),
                })
                .try_next() // 필터를 통과하는 첫 번째 아이템 (또는 에러)을 가져옵니다.
                .map_ok(move |x| match x {
                    Some(SSE::Event(event)) => {
                        tracing::info!("endpoint event: {:?}", event);
                        Some(event.data)
                    }
                    _ => {
                        tracing::error!("No endpoint event found");
                        None
                    }
                }),
        )
        .await; // 스트림 처리를 기다립니다. 에러가 발생하면 `?` 연산자가 에러를 반환합니다.

        // 결과 처리:
        match endpoint_event_result {
            // 타임아웃 없이 결과 수신
            Ok(Ok(Some(event))) => Ok(event), // "endpoint" 이벤트를 성공적으로 찾은 경우
            Ok(Ok(None)) => {
                tracing::warn!("Stream ended before endpoint event was received");
                Err(anyhow!("Stream ended before endpoint event was received"))
            } // 스트림 종료
            Ok(Err(e)) => {
                tracing::error!("Error waiting for endpoint event: {}", e);
                Err(e.into())
            }
            Err(_) => {
                tracing::error!("Timeout waiting for endpoint event");
                Err(anyhow!("Timeout waiting for endpoint event"))
            } // 타임아웃 발생
        }
    }
}
