use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};

use crate::WakeupSignalBus;

#[derive(Clone)]
pub struct MemoryWakeupSignalBus {
    sender: mpsc::UnboundedSender<String>,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<String>>>,
}

impl MemoryWakeupSignalBus {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }
}

impl Default for MemoryWakeupSignalBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WakeupSignalBus for MemoryWakeupSignalBus {
    async fn publish(&self, key: &str) -> anyhow::Result<()> {
        self.sender
            .send(key.to_string())
            .map_err(|_| anyhow!("memory wakeup signal bus receiver closed"))
    }

    async fn poll(&self) -> anyhow::Result<Option<String>> {
        let mut receiver = self.receiver.lock().await;
        match receiver.try_recv() {
            Ok(key) => Ok(Some(key)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }
}
