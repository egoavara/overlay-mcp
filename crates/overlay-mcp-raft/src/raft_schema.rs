use rmcp::{
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    serde_json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, hiqlite::EnumIter, hiqlite::ToPrimitive)]
pub enum RaftSchema {
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftSchemaEvent {
    // Delete session, if this event is received, all raft node drop the session
    DeleteSession(String),
    // New main session, if this event is received, it mean there is a new main session
    // If Any Session is received this event, it will change self session type to sub session
    NewMainSession(String, String),
    // Notify to sub session
    // If sub session is received this event, it will notify to client
    NotifyToSubSession(EventNotifyToSubSession),
    // Notify to main session
    // If main session is received this event, it will notify to server
    // client -[MCP Protocol]> raft node(with sub session) -[Raft Cluster Event]> raft node(with main session) -[MCP Protocol]> server
    NotifyToMainSession(EventNotifyToMainSession),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotifyToMainSession {
    pub session_id: String,
    // serialized ClientJsonRpcMessage
    pub raw_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotifyToSubSession {
    pub session_id: String,
    // serialized ServerJsonRpcMessage
    pub raw_json: String,
}

impl RaftSchemaEvent {
    pub fn notify_to_main_session(session_id: String, event: &ClientJsonRpcMessage) -> Self {
        Self::NotifyToMainSession(EventNotifyToMainSession {
            session_id,
            raw_json: serde_json::to_string(event).unwrap(),
        })
    }

    pub fn notify_to_sub_session(session_id: String, event: &ServerJsonRpcMessage) -> Self {
        Self::NotifyToSubSession(EventNotifyToSubSession {
            session_id,
            raw_json: serde_json::to_string(event).unwrap(),
        })
    }
}

impl EventNotifyToMainSession {
    pub fn to_client_json_rpc_message(&self) -> ClientJsonRpcMessage {
        serde_json::from_str(&self.raw_json).unwrap()
    }
}

impl EventNotifyToSubSession {
    pub fn to_server_json_rpc_message(&self) -> ServerJsonRpcMessage {
        serde_json::from_str(&self.raw_json).unwrap()
    }
}
