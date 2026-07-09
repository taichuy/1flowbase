use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::EphemeralKvStore;

#[derive(Clone)]
pub struct MemoryKvStore {
    namespace: String,
    inner: Arc<RwLock<HashMap<String, MemoryEntry>>>,
}

#[derive(Clone)]
struct MemoryEntry {
    value: serde_json::Value,
    expires_at: Option<OffsetDateTime>,
}

impl MemoryKvStore {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn raw_key_for_test(&self, key: &str) -> String {
        self.namespaced_key(key)
    }

    fn namespaced_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace, key)
    }

    fn expires_at(ttl: Option<time::Duration>) -> Option<OffsetDateTime> {
        ttl.map(|ttl| OffsetDateTime::now_utc() + ttl)
    }

    fn ensure_counter_ttl(ttl: Option<time::Duration>) -> anyhow::Result<()> {
        anyhow::ensure!(
            ttl.is_none_or(|ttl| ttl > time::Duration::ZERO),
            "counter_ttl_must_be_positive"
        );
        Ok(())
    }
}

#[async_trait]
impl EphemeralKvStore for MemoryKvStore {
    async fn set_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<()> {
        self.inner.write().await.insert(
            self.namespaced_key(key),
            MemoryEntry {
                value,
                expires_at: Self::expires_at(ttl),
            },
        );
        Ok(())
    }

    async fn get_json(&self, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;
        let Some(entry) = map.get(&namespaced_key).cloned() else {
            return Ok(None);
        };

        if entry
            .expires_at
            .is_some_and(|deadline| deadline <= OffsetDateTime::now_utc())
        {
            map.remove(&namespaced_key);
            return Ok(None);
        }

        Ok(Some(entry.value))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.inner.write().await.remove(&self.namespaced_key(key));
        Ok(())
    }

    async fn touch(&self, key: &str, ttl: time::Duration) -> anyhow::Result<bool> {
        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;
        let Some(entry) = map.get_mut(&namespaced_key) else {
            return Ok(false);
        };

        if entry
            .expires_at
            .is_some_and(|deadline| deadline <= OffsetDateTime::now_utc())
        {
            map.remove(&namespaced_key);
            return Ok(false);
        }

        entry.expires_at = Some(OffsetDateTime::now_utc() + ttl);
        Ok(true)
    }

    async fn set_if_absent_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<bool> {
        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;

        if let Some(entry) = map.get(&namespaced_key) {
            if entry
                .expires_at
                .is_some_and(|deadline| deadline <= OffsetDateTime::now_utc())
            {
                map.remove(&namespaced_key);
            } else {
                return Ok(false);
            }
        }

        map.insert(
            namespaced_key,
            MemoryEntry {
                value,
                expires_at: Self::expires_at(ttl),
            },
        );
        Ok(true)
    }

    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<i64> {
        Self::ensure_counter_ttl(ttl)?;

        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;
        let now = OffsetDateTime::now_utc();
        let existing = map.get(&namespaced_key).cloned();
        let active_entry = existing.filter(|entry| {
            !entry
                .expires_at
                .is_some_and(|deadline| deadline <= OffsetDateTime::now_utc())
        });
        if active_entry.is_none() {
            map.remove(&namespaced_key);
        }

        let current = active_entry
            .as_ref()
            .map(|entry| {
                entry
                    .value
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("counter_value_must_be_integer"))
            })
            .transpose()?
            .unwrap_or(0);
        let next = current
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("counter_value_overflow"))?;
        let expires_at = ttl
            .map(|ttl| now + ttl)
            .or_else(|| active_entry.as_ref().and_then(|entry| entry.expires_at));
        map.insert(
            namespaced_key,
            MemoryEntry {
                value: serde_json::Value::Number(next.into()),
                expires_at,
            },
        );
        Ok(next)
    }
}
