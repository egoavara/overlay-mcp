mod any_error;
pub mod json_request;
pub mod urls_display;
pub use any_error::*;
pub use json_request::*;
use rmcp::model::{JsonRpcMessage, NumberOrString};

pub fn extract_json_rpc_id<Req, Res, Noti>(
    message: &JsonRpcMessage<Req, Res, Noti>,
) -> Option<NumberOrString> {
    match message {
        JsonRpcMessage::Request(req) => Some(req.id.clone()),
        JsonRpcMessage::Response(resp) => Some(resp.id.clone()),
        JsonRpcMessage::Notification(_) => None,
        JsonRpcMessage::Error(err) => Some(err.id.clone()),
    }
}
