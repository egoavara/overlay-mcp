use tokio::sync::oneshot;

pub struct StreamGuard<S>(Option<(S, oneshot::Sender<S>)>);
pub struct SessionGuard(Option<(String, oneshot::Sender<String>)>);

impl<S> StreamGuard<S> {
    pub fn new(data: S) -> (Self, oneshot::Receiver<S>) {
        let (s, r) = oneshot::channel();
        (Self(Some((data, s))), r)
    }
}

impl<S> Drop for StreamGuard<S> {
    fn drop(&mut self) {
        tracing::info!("StreamGuard dropped");
        let Some((value, dropper)) = self.0.take() else {
            panic!("StreamGuard already dropped, not expected result");
        };
        match dropper.send(value) {
            Ok(_) => {}
            Err(_s) => {
                tracing::error!("failed to send value to dropper");
            }
        };
    }
}

impl<S> std::ops::Deref for StreamGuard<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0.as_ref().unwrap().0
    }
}

impl<S> std::ops::DerefMut for StreamGuard<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.as_mut().unwrap().0
    }
}

impl SessionGuard {
    pub fn new(session_id: String) -> (Self, oneshot::Receiver<String>) {
        let (s, r) = oneshot::channel();
        (Self(Some((session_id, s))), r)
    }

    pub fn session_id(&self) -> &str {
        let Some((session_id, _)) = &self.0 else {
            panic!("StreamGuard already dropped, not expected result");
        };
        session_id.as_str()
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        tracing::info!("StreamGuard dropped");
        let Some((value, dropper)) = self.0.take() else {
            panic!("StreamGuard already dropped, not expected result");
        };
        match dropper.send(value) {
            Ok(_) => {}
            Err(_s) => {
                tracing::error!("failed to send value to dropper");
            }
        };
    }
}
