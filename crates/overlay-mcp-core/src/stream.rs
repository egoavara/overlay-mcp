use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use tokio::sync::broadcast;

use crate::Error;

pub struct Upstream(pub broadcast::Sender<ClientJsonRpcMessage>);

pub struct Downstream(pub broadcast::Receiver<ServerJsonRpcMessage>);

pub struct BypassDownstream(pub broadcast::Sender<ServerJsonRpcMessage>);

impl Upstream {
    pub async fn send(&self, msg: ClientJsonRpcMessage) -> Result<(), Error> {
        self.0.send(msg).map_err(|_| Error::TokioBroadcastError)?;
        Ok(())
    }
}

impl Downstream {
    pub async fn recv(&mut self) -> Result<ServerJsonRpcMessage, Error> {
        let msg = self
            .0
            .recv()
            .await
            .map_err(|_| Error::TokioBroadcastError)?;
        Ok(msg)
    }
}

impl BypassDownstream {
    pub async fn send(&self, msg: ServerJsonRpcMessage) -> Result<(), Error> {
        self.0.send(msg).map_err(|_| Error::TokioBroadcastError)?;
        Ok(())
    }
}
