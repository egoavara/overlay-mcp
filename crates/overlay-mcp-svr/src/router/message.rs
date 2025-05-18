use axum::Extension;
use http::StatusCode;
use overlay_mcp_auth::Authz;
use overlay_mcp_core::{
    AuthorizationResult, Error, Error404, GeneralAuthz, GeneralSession, GeneralSessionManager,
    MCP20241105,
};
use overlay_mcp_session_manager::SessionManager;
use rmcp::model::{
    ClientJsonRpcMessage, ErrorCode, ErrorData, JsonRpcError, JsonRpcMessage, JsonRpcVersion2_0,
    NumberOrString, ServerJsonRpcMessage,
};

use crate::{
    middlewares::{HttpAuthentication, HttpSessionId},
    utils::JsonRequest,
};

pub async fn handler(
    HttpAuthentication(authn): HttpAuthentication,
    session_id: HttpSessionId<MCP20241105>,
    Extension(session_manager): Extension<SessionManager>,
    Extension(authz): Extension<Authz>,
    req: JsonRequest<ClientJsonRpcMessage>,
) -> Result<StatusCode, Error> {
    let result = authz.authorize_client_message(&authn, &req.json).await?;
    let Some(session) = session_manager.find(session_id.as_str()).await? else {
        return Err(Error::NotFound(Error404::SessionNotFound {
            session_id: session_id.to_string(),
        }));
    };
    session.ensure_started(&req.parts).await?;
    match result {
        AuthorizationResult::Allow => {}
        AuthorizationResult::Deny => {
            let bypass = session.guard_bypass_downstream().await?;
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
                .await?;
        }
        result @ AuthorizationResult::Unauthorized => {
            result.to_err_response()?;
        }
    }
    let send = session.guard_upstream().await?;
    tracing::info!(
        "upstream \n{}",
        serde_json::to_string_pretty(&req.json).unwrap()
    );

    send.send(req.json).await?;
    Ok(StatusCode::ACCEPTED)
}

fn extract_json_rpc_id<Req, Res, Noti>(
    message: &JsonRpcMessage<Req, Res, Noti>,
) -> Option<NumberOrString> {
    match message {
        JsonRpcMessage::Request(req) => Some(req.id.clone()),
        JsonRpcMessage::Response(resp) => Some(resp.id.clone()),
        JsonRpcMessage::Notification(_) => None,
        JsonRpcMessage::Error(err) => Some(err.id.clone()),
    }
}
