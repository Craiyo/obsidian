use tokio::sync::broadcast;

#[derive(Clone)]
pub struct WsHub {
    sender: broadcast::Sender<String>,
}

impl WsHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    pub fn send(&self, msg: String) {
        let _ = self.sender.send(msg);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}
