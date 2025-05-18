use axum::{
    body::Body,
    extract::Request,
    response::{sse::Event, Sse},
    Extension,
};
use futures::StreamExt;
use futures_util::Stream;
use httpbuilder::http_reference::HttpReference;
use overlay_mcp_auth::Authz;
use overlay_mcp_core::{
    Authentication, Error, Error404, GeneralAuthz, GeneralResolver, GeneralSession,
    GeneralSessionManager, MCP20241105,
};
use overlay_mcp_resolver::Resolver;
use overlay_mcp_session_manager::SessionManager;
use url::form_urlencoded;

use crate::middlewares::{HttpAuthentication, HttpSessionId};

pub async fn handler(
    HttpAuthentication(authn): HttpAuthentication,
    session_id: Option<HttpSessionId<MCP20241105>>,
    Extension(resolver): Extension<Resolver>,
    Extension(session_manager): Extension<SessionManager>,
    Extension(authz): Extension<Authz>,
    req: Request<Body>,
) -> Result<Sse<impl Stream<Item = Result<Event, Error>>>, Error> {
    let (parts, _) = req.into_parts();

    authz.authorize_enter(&authn).await?.to_err_response()?;
    let session = match session_id {
        Some(session_id) => {
            tracing::info!(session_id = session_id.as_str(), "sse connection");
            session_manager
                .find(session_id.as_str())
                .await?
                .ok_or(Error::NotFound(Error404::SessionNotFound {
                    session_id: session_id.to_string(),
                }))?
        }
        None => {
            tracing::info!("sse connection without session id");
            let upstream_url = resolver.resolve(&parts).await?;
            session_manager.create(upstream_url).await?
        }
    };
    session.ensure_started(&parts).await?;
    let session_guard = session.guard_close().await?;
    let downstream_guard = session.guard_downstream().await?;

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("session_id", session_guard.session_id());
    if let Authentication::ApiKey {
        apikey,
        apikey_from: HttpReference::Query(query),
    } = authn
    {
        serializer.append_pair(query.as_str(), apikey.as_str());
    }

    let query_str = serializer.finish();

    let recv_stream = async_stream::stream! {
        let mut recv = downstream_guard;
        let _guard = session_guard;
        loop {
            let message = match recv.recv().await {
                Ok(message) => message,
                Err(_) => break,
            };
            let data = serde_json::to_string(&message).expect("failed to serialize message");
            yield Ok(Event::default().event("message").data(&data));
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
