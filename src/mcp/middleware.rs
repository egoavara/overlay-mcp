use std::{marker::PhantomData, sync::Arc};

use axum::extract::FromRequestParts;
use http::{request::Parts, StatusCode};
use rmcp::{
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    RoleClient,
};
use tokio::sync::broadcast;
use serde_json::json;

use crate::{
    manager::storage::{ManagerTrait, SessionGuard, StorageManager, StreamGuard},
    reqmodifier::{BaseModifiers, ReqModifier},
    utils::AnyError,
};

use super::specification::MCPSpecification;

pub struct MCPSessionUpstream<Spec: MCPSpecification> {
    pub session_id: Option<String>,
    _marker: PhantomData<Spec>,
}
pub struct MCPSessionDownstream<Spec: MCPSpecification> {
    pub session_id: Option<String>,
    _marker: PhantomData<Spec>,
}

impl<Spec, S> FromRequestParts<S> for MCPSessionUpstream<Spec>
where
    Spec: MCPSpecification,
    S: Send + Sync,
{
    type Rejection = AnyError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let session_id = Spec::extract_session_id(parts);

        Ok(Self {
            session_id,
            _marker: PhantomData,
        })
    }
}

impl<Spec, S> FromRequestParts<S> for MCPSessionDownstream<Spec>
where
    Spec: MCPSpecification,
    S: Send + Sync,
{
    type Rejection = AnyError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let session_id = Spec::extract_session_id(parts);
        Ok(Self {
            session_id,
            _marker: PhantomData,
        })
    }
}

impl<Spec> MCPSessionDownstream<Spec>
where
    Spec: MCPSpecification,
{
    pub async fn connect(
        &self,
        parts: &Parts,
    ) -> Result<
        (
            StreamGuard<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>>,
            SessionGuard,
        ),
        AnyError,
    > {
        let storage_manager = parts.extensions.get::<StorageManager>().unwrap().clone();

        let Some(session_id) = &self.session_id else {
            let base_modifiers = parts
                .extensions
                .get::<Arc<BaseModifiers>>()
                .unwrap()
                .clone();
            let mut req_modifier = ReqModifier::new(parts, json!({}));
            req_modifier.apply(&base_modifiers)?;

            let client = req_modifier.finish_client()?;
            let conn_state = storage_manager.connect(client).await?;

            let session_guard = storage_manager
                .session_guard(conn_state.session_id.clone())
                .await?;
            let downstream_guard = storage_manager
                .take_downstream(conn_state.session_id.clone())
                .await?;

            return Ok((downstream_guard, session_guard));
        };
        let Some(state) = storage_manager.reload(session_id.clone()).await? else {
            return Err(AnyError::http(StatusCode::NOT_FOUND, "session not found"));
        };
        let session_guard = storage_manager.session_guard(session_id.clone()).await?;
        let downstream_guard = storage_manager
            .take_downstream(state.session_id.clone())
            .await?;
        Ok((downstream_guard, session_guard))
    }
}

impl<Spec> MCPSessionUpstream<Spec>
where
    Spec: MCPSpecification,
{
    pub async fn connect(
        &self,
        parts: &Parts,
    ) -> Result<StreamGuard<broadcast::Sender<TxJsonRpcMessage<RoleClient>>>, AnyError> {
        let storage_manager = parts.extensions.get::<StorageManager>().unwrap().clone();

        let Some(session_id) = &self.session_id else {
            return Err(AnyError::http(
                StatusCode::BAD_REQUEST,
                "session id is required",
            ));
        };
        let upstream_guard = storage_manager.take_upstream(session_id.clone()).await?;
        Ok(upstream_guard)
    }

    pub async fn bypass(
        &self,
        parts: &Parts,
    ) -> Result<StreamGuard<broadcast::Sender<RxJsonRpcMessage<RoleClient>>>, AnyError> {
        let storage_manager = parts.extensions.get::<StorageManager>().unwrap().clone();

        let Some(session_id) = &self.session_id else {
            return Err(AnyError::http(
                StatusCode::BAD_REQUEST,
                "session id is required",
            ));
        };
        let bypass_guard = storage_manager
            .take_bypass_downstream(session_id.clone())
            .await?;
        Ok(bypass_guard)
    }
}
