use async_trait::async_trait;

#[async_trait]
pub trait EphemeralKvStore: Send + Sync {
    async fn set_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<()>;
    async fn get_json(&self, key: &str) -> anyhow::Result<Option<serde_json::Value>>;
    async fn delete(&self, key: &str) -> anyhow::Result<()>;
    async fn touch(&self, key: &str, ttl: time::Duration) -> anyhow::Result<bool>;
    async fn set_if_absent_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<bool>;
    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<i64>;
}
