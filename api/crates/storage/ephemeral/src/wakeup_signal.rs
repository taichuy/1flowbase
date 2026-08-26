use async_trait::async_trait;

#[async_trait]
pub trait WakeupSignalBus: Send + Sync {
    async fn publish(&self, key: &str) -> anyhow::Result<()>;
    async fn poll(&self) -> anyhow::Result<Option<String>>;
}
