use rmcp::{
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    RoleClient, RoleServer,
};
use tokio::sync::broadcast;

pub struct MCPTunnelForServer {
    pub recv: broadcast::Receiver<RxJsonRpcMessage<RoleServer>>,
    pub send: broadcast::Sender<TxJsonRpcMessage<RoleServer>>,
}
pub struct MCPTunnelForClient {
    pub recv: broadcast::Receiver<RxJsonRpcMessage<RoleClient>>,
    pub send: broadcast::Sender<TxJsonRpcMessage<RoleClient>>,
}

pub fn new_tunnel() -> (MCPTunnelForServer, MCPTunnelForClient) {
    let (svr_send, svr_recv) = broadcast::channel::<RxJsonRpcMessage<RoleServer>>(16);
    let (clt_send, clt_recv) = broadcast::channel::<RxJsonRpcMessage<RoleClient>>(16);

    (
        MCPTunnelForServer {
            recv: svr_recv,
            send: clt_send,
        },
        MCPTunnelForClient {
            recv: clt_recv,
            send: svr_send,
        },
    )
}
