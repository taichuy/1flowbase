use std::{
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    ephemeral_metadata_size_bytes, EphemeralEntrySnapshot, EphemeralEntryValueSnapshot,
    EphemeralInspectionCapabilities, EphemeralValueRevealMode, RateLimitDecision, RateLimitStore,
};
use moka::{future::Cache, Expiry};
use time::OffsetDateTime;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct RateLimitWindow {
    count: u64,
    reset_at: OffsetDateTime,
}

struct RateLimitWindowExpiry;

impl Expiry<String, RateLimitWindow> for RateLimitWindowExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &RateLimitWindow,
        _created_at: Instant,
    ) -> Option<StdDuration> {
        std_duration_until(value.reset_at)
    }

    fn expire_after_update(
        &self,
        _key: &String,
        value: &RateLimitWindow,
        _updated_at: Instant,
        _duration_until_expiry: Option<StdDuration>,
    ) -> Option<StdDuration> {
        std_duration_until(value.reset_at)
    }
}

#[derive(Clone)]
pub struct MokaRateLimitStore {
    namespace: String,
    cache: Cache<String, RateLimitWindow>,
    update_guard: Arc<Mutex<()>>,
}

impl MokaRateLimitStore {
    pub fn new(namespace: impl Into<String>, max_capacity: u64) -> Self {
        Self {
            namespace: namespace.into(),
            cache: Cache::builder()
                .max_capacity(max_capacity)
                .expire_after(RateLimitWindowExpiry)
                .build(),
            update_guard: Arc::new(Mutex::new(())),
        }
    }

    fn namespaced_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace, key)
    }

    fn key_without_namespace(&self, key: &str) -> Option<String> {
        key.strip_prefix(&format!("{}:", self.namespace))
            .map(ToString::to_string)
    }

    fn value_size_bytes(window: &RateLimitWindow) -> u64 {
        serde_json::to_vec(&serde_json::json!({
            "count": window.count,
            "reset_at_unix": window.reset_at.unix_timestamp(),
        }))
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(0)
    }

    fn entry_snapshot(key: String, window: &RateLimitWindow) -> EphemeralEntrySnapshot {
        let now = OffsetDateTime::now_utc();
        let metadata = serde_json::json!({
            "count": window.count,
            "reset_at_unix": window.reset_at.unix_timestamp(),
        });
        EphemeralEntrySnapshot {
            contract_code: "rate-limit-store".to_string(),
            group_code: key.split_once(':').map(|(group, _)| group.to_string()),
            entry_ref: key.clone(),
            inspection_path: key
                .split(':')
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect(),
            key,
            entry_kind: "rate_limit_window".to_string(),
            status: if window.reset_at > now {
                "active".to_string()
            } else {
                "expired".to_string()
            },
            owner: None,
            value_size_bytes: Self::value_size_bytes(window),
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: Some((window.reset_at - now).whole_seconds().max(0)),
            created_at_unix: None,
            expires_at_unix: Some(window.reset_at.unix_timestamp()),
            sensitive: false,
            metadata,
        }
    }
}

#[async_trait]
impl RateLimitStore for MokaRateLimitStore {
    async fn consume(
        &self,
        key: &str,
        limit: u64,
        window: time::Duration,
    ) -> anyhow::Result<RateLimitDecision> {
        let _guard = self.update_guard.lock().await;
        let namespaced_key = self.namespaced_key(key);
        let now = OffsetDateTime::now_utc();
        let reset_at = if window <= time::Duration::ZERO {
            now
        } else {
            now + window
        };
        let mut current = self
            .cache
            .get(&namespaced_key)
            .await
            .filter(|entry| entry.reset_at > now)
            .unwrap_or(RateLimitWindow { count: 0, reset_at });

        let allowed = limit > 0 && current.count < limit;
        if allowed {
            current.count += 1;
        }

        let remaining = limit.saturating_sub(current.count).min(limit);
        let reset_after_ms = (current.reset_at - now).whole_milliseconds().max(0) as u64;
        self.cache.insert(namespaced_key, current).await;

        Ok(RateLimitDecision {
            allowed,
            remaining,
            reset_after_ms,
        })
    }

    async fn reset(&self, key: &str) -> anyhow::Result<()> {
        self.cache.invalidate(&self.namespaced_key(key)).await;
        Ok(())
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::supported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        self.cache.run_pending_tasks().await;
        let mut entries = self
            .cache
            .iter()
            .filter_map(|(key, window)| {
                self.key_without_namespace(&key)
                    .map(|key| Self::entry_snapshot(key, &window))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(entries)
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        let Some(window) = self.cache.get(&self.namespaced_key(entry_ref)).await else {
            return Ok(None);
        };
        let metadata = Self::entry_snapshot(entry_ref.to_string(), &window);
        let value = serde_json::json!({
                "count": window.count,
                "reset_at_unix": window.reset_at.unix_timestamp(),
        });
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            metadata,
            value,
            reveal_mode,
        )))
    }
}

fn std_duration_until(deadline: OffsetDateTime) -> Option<StdDuration> {
    let ttl = deadline - OffsetDateTime::now_utc();
    if ttl <= time::Duration::ZERO {
        Some(StdDuration::ZERO)
    } else {
        ttl.try_into().ok()
    }
}
