use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    ephemeral_metadata_size_bytes, EphemeralEntrySnapshot, EphemeralEntryValueSnapshot,
    EphemeralValueRevealMode,
};
use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::LeaseStore;

#[derive(Clone)]
pub struct MemoryLeaseStore {
    namespace: String,
    inner: Arc<RwLock<HashMap<String, LeaseEntry>>>,
}

#[derive(Clone)]
struct LeaseEntry {
    owner: String,
    expires_at: OffsetDateTime,
}

impl MemoryLeaseStore {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn namespaced_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace, key)
    }

    fn key_without_namespace(&self, key: &str) -> Option<String> {
        key.strip_prefix(&format!("{}:", self.namespace))
            .map(ToString::to_string)
    }

    fn expires_at(ttl: time::Duration) -> OffsetDateTime {
        OffsetDateTime::now_utc() + ttl
    }

    fn entry_snapshot(key: String, entry: &LeaseEntry) -> EphemeralEntrySnapshot {
        let now = OffsetDateTime::now_utc();
        let metadata = serde_json::json!({
            "owner": entry.owner,
            "expires_at_unix": entry.expires_at.unix_timestamp(),
        });
        EphemeralEntrySnapshot {
            contract_code: "distributed-lock".to_string(),
            group_code: key.split_once(':').map(|(group, _)| group.to_string()),
            entry_ref: key.clone(),
            inspection_path: key
                .split(':')
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect(),
            key,
            entry_kind: "lock".to_string(),
            status: if entry.expires_at > now {
                "active".to_string()
            } else {
                "expired".to_string()
            },
            owner: Some(entry.owner.clone()),
            value_size_bytes: serde_json::to_vec(&serde_json::json!({
                "owner": entry.owner,
                "expires_at_unix": entry.expires_at.unix_timestamp(),
            }))
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0),
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: Some((entry.expires_at - now).whole_seconds().max(0)),
            created_at_unix: None,
            expires_at_unix: Some(entry.expires_at.unix_timestamp()),
            sensitive: false,
            metadata,
        }
    }

    pub(crate) async fn list_ephemeral_entries_for_inspection(
        &self,
    ) -> Vec<EphemeralEntrySnapshot> {
        let now = OffsetDateTime::now_utc();
        let mut entries = self
            .inner
            .read()
            .await
            .iter()
            .filter(|(_, entry)| entry.expires_at > now)
            .filter_map(|(key, entry)| {
                self.key_without_namespace(key)
                    .map(|key| Self::entry_snapshot(key, entry))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        entries
    }

    pub(crate) async fn reveal_ephemeral_entry_for_inspection(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> Option<EphemeralEntryValueSnapshot> {
        let namespaced_key = self.namespaced_key(entry_ref);
        let map = self.inner.read().await;
        let entry = map.get(&namespaced_key)?;
        if entry.expires_at <= OffsetDateTime::now_utc() {
            return None;
        }
        let metadata = Self::entry_snapshot(entry_ref.to_string(), entry);
        let value = serde_json::json!({
                "owner": entry.owner,
                "expires_at_unix": entry.expires_at.unix_timestamp(),
        });
        Some(EphemeralEntryValueSnapshot::from_value(
            metadata,
            value,
            reveal_mode,
        ))
    }
}

#[async_trait]
impl LeaseStore for MemoryLeaseStore {
    async fn acquire(&self, key: &str, owner: &str, ttl: time::Duration) -> anyhow::Result<bool> {
        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;
        let now = OffsetDateTime::now_utc();

        match map.get(&namespaced_key) {
            Some(entry) if entry.expires_at > now && entry.owner != owner => return Ok(false),
            _ => {}
        }

        map.insert(
            namespaced_key,
            LeaseEntry {
                owner: owner.to_string(),
                expires_at: Self::expires_at(ttl),
            },
        );
        Ok(true)
    }

    async fn renew(&self, key: &str, owner: &str, ttl: time::Duration) -> anyhow::Result<bool> {
        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;
        let now = OffsetDateTime::now_utc();
        let Some(entry) = map.get_mut(&namespaced_key) else {
            return Ok(false);
        };

        if entry.expires_at <= now {
            map.remove(&namespaced_key);
            return Ok(false);
        }
        if entry.owner != owner {
            return Ok(false);
        }

        entry.expires_at = Self::expires_at(ttl);
        Ok(true)
    }

    async fn release(&self, key: &str, owner: &str) -> anyhow::Result<bool> {
        let namespaced_key = self.namespaced_key(key);
        let mut map = self.inner.write().await;
        let now = OffsetDateTime::now_utc();
        let Some(entry) = map.get(&namespaced_key) else {
            return Ok(false);
        };

        if entry.expires_at <= now {
            map.remove(&namespaced_key);
            return Ok(false);
        }
        if entry.owner != owner {
            return Ok(false);
        }

        map.remove(&namespaced_key);
        Ok(true)
    }
}
