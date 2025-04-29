use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::Request,
    response::{sse::Event, Sse},
    Extension,
};
use futures::SinkExt;
use futures_util::{Stream, StreamExt};
use http::StatusCode;
use rmcp::model::{
    ClientJsonRpcMessage, ErrorCode, ErrorData, JsonRpcError, JsonRpcVersion2_0, NumberOrString,
    ServerJsonRpcMessage,
};
use tokio::sync::broadcast::error::RecvError;

use crate::{
    manager::{
        auth::{
            authorizer::{Authorizer, AuthorizerTrait},
            Authentication,
        },
        storage::ManagerTrait,
    },
    mcp::{
        middleware::{MCPSessionDownstream, MCPSessionUpstream},
        specification::MCP20241105,
    },
    reqmodifier::reference::HttpPartReference,
    utils::{extract_json_rpc_id, AnyError, JsonRequest},
};

pub(crate) async fn handler_upstream(
    session: MCPSessionUpstream<MCP20241105>,
    authn: Authentication,
    Extension(authorizer): Extension<Authorizer>,
    req: JsonRequest<ClientJsonRpcMessage>,
) -> Result<StatusCode, AnyError> {
    authorizer
        .authorize_authentication(&authn)
        .await?
        .to_err_response()?;

    let authz = authorizer;
    let is_allowed = authz
        .authorize_client_message(&authn, &req.json)
        .await?
        .to_is_allowed()?;
    if !is_allowed {
        let bypass = session.bypass(&req.parts).await?;
        let id = extract_json_rpc_id(&req.json).unwrap_or(NumberOrString::Number(0));
        tracing::error!("unauthorized");
        bypass
            .send(ServerJsonRpcMessage::Error(JsonRpcError {
                jsonrpc: JsonRpcVersion2_0,
                id,
                error: ErrorData {
                    code: ErrorCode::INVALID_REQUEST,
                    message: "This method is not allowed".into(),
                    data: None,
                },
            }))
            .context("failed to send message")?;
        return Ok(StatusCode::ACCEPTED);
    }

    let send = session.connect(&req.parts).await?;
    tracing::info!(
        "upstream \n{}",
        serde_json::to_string_pretty(&req.json).unwrap()
    );

    send.send(req.json).context("failed to send message")?;
    Ok(StatusCode::ACCEPTED)
}

pub(crate) async fn handler_downstream(
    authn: Authentication,
    session: MCPSessionDownstream<MCP20241105>,
    Extension(authorizer): Extension<Authorizer>,
    req: Request<Body>,
) -> Result<Sse<impl Stream<Item = Result<Event, anyhow::Error>>>, AnyError> {
    let (parts, _) = req.into_parts();
    authorizer
        .authorize_authentication(&authn)
        .await?
        .to_err_response()?;
    tracing::info!(session_id = &session.session_id, "sse connection");

    let (upstream_guard, session_guard) = session.connect(&parts).await?;

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("session_id", session_guard.session_id());
    if let Some((apikey, HttpPartReference::Query(name))) = authn.apikey() {
        serializer.append_pair(name.as_str(), apikey.as_str());
    }

    let query_str = serializer.finish();

    let recv_stream = async_stream::stream! {
        let mut recv = upstream_guard;
        let _guard = session_guard;
        loop {
            let message = match recv.recv().await {
                Ok(message) => message,
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(lagged)) => {
                    tracing::warn!(lagged = lagged, "receiver lagged");
                    continue;
                }
            };
            match serde_json::to_string(&message).context("failed to serialize message") {
                Ok(bytes) => {
                    yield Ok(Event::default().event("message").data(&bytes));
                }
                Err(e) => {
                    tracing::error!(error = ?e, "failed to serialize message");
                    yield Err(e);
                }
            }
        }
    };

    let stream = futures::stream::once(futures::future::ok(
        Event::default()
            .event("endpoint")
            .data(format!("/message?{}", query_str)),
    ))
    .chain(recv_stream);
    Ok(Sse::new(stream))
}
//
