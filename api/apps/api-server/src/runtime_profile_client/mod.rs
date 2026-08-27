use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{CacheStore, DistributedLock};
use runtime_profile::{RuntimeProfile, RuntimeProfileCollector};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const API_SERVER_TARGET_ID: &str = "api-server";
const RUNTIME_HOST_TARGET_ID: &str = "runtime-extension-host";
const SNAPSHOT_FRESHNESS: Duration = Duration::seconds(1);
const SNAPSHOT_RETENTION: Duration = Duration::seconds(10);
const REFRESH_LOCK_TTL: Duration = Duration::seconds(10);
const REFRESH_WAIT_TIMEOUT: StdDuration = StdDuration::from_secs(10);
const REFRESH_WAIT_INTERVAL: StdDuration = StdDuration::from_millis(25);

#[async_trait]
pub trait ApiRuntimeProfilePort: Send + Sync {
    async fn collect_runtime_profile(&self) -> Result<RuntimeProfile>;
}

#[derive(Debug, Clone)]
pub struct HostApiRuntimeProfileCollector {
    collector: Arc<RuntimeProfileCollector>,
}

impl HostApiRuntimeProfileCollector {
    pub fn new(process_started_at: OffsetDateTime) -> Result<Self> {
        Ok(Self {
            collector: Arc::new(RuntimeProfileCollector::new(
                "api-server",
                env!("CARGO_PKG_VERSION"),
                process_started_at,
                "ok",
            )?),
        })
    }
}

#[async_trait]
impl ApiRuntimeProfilePort for HostApiRuntimeProfileCollector {
    async fn collect_runtime_profile(&self) -> Result<RuntimeProfile> {
        let collector = self.collector.clone();
        tokio::task::spawn_blocking(move || collector.collect()).await?
    }
}

#[async_trait]
pub trait RuntimeHostSystemPort: Send + Sync {
    async fn fetch_runtime_profile(&self) -> Result<RuntimeProfile>;
}

#[async_trait]
impl RuntimeHostSystemPort for runtime_extension_host::RuntimeExtensionHost {
    async fn fetch_runtime_profile(&self) -> Result<RuntimeProfile> {
        let host = self.clone();
        tokio::task::spawn_blocking(move || host.collect_runtime_profile().map_err(Into::into))
            .await?
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProfilesSnapshot {
    pub api_profile: RuntimeProfile,
    pub host_profile: RuntimeProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CachedRuntimeTargetObservation {
    Reachable {
        observed_at: OffsetDateTime,
        profile: Box<RuntimeProfile>,
    },
}

impl CachedRuntimeTargetObservation {
    fn reachable(profile: RuntimeProfile, observed_at: OffsetDateTime) -> Self {
        Self::Reachable {
            observed_at,
            profile: Box::new(profile),
        }
    }

    fn observed_at(&self) -> OffsetDateTime {
        match self {
            Self::Reachable { observed_at, .. } => *observed_at,
        }
    }

    fn target_id(&self) -> &str {
        match self {
            Self::Reachable { profile, .. } => &profile.service,
        }
    }

    fn is_fresh(&self, now: OffsetDateTime) -> bool {
        let age = now - self.observed_at();
        age >= Duration::ZERO && age <= SNAPSHOT_FRESHNESS
    }

    fn into_profile(self) -> RuntimeProfile {
        match self {
            Self::Reachable { profile, .. } => *profile,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeProfileSnapshotCache {
    cache_store: Arc<dyn CacheStore>,
    refresh_lock: Arc<dyn DistributedLock>,
    api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    runtime_host_system: Arc<dyn RuntimeHostSystemPort>,
    api_node_id: String,
    runtime_instance_id: String,
}

impl RuntimeProfileSnapshotCache {
    pub(crate) fn new(
        cache_store: Arc<dyn CacheStore>,
        refresh_lock: Arc<dyn DistributedLock>,
        api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
        runtime_host_system: Arc<dyn RuntimeHostSystemPort>,
        api_node_id: impl Into<String>,
        process_started_at: OffsetDateTime,
    ) -> Self {
        Self {
            cache_store,
            refresh_lock,
            api_runtime_profile,
            runtime_host_system,
            api_node_id: api_node_id.into(),
            runtime_instance_id: process_started_at.unix_timestamp_nanos().to_string(),
        }
    }

    pub(crate) async fn get_or_refresh(&self) -> Result<RuntimeProfilesSnapshot> {
        if let Some(snapshot) = self.read_fresh_snapshot().await? {
            return Ok(snapshot);
        }

        let owner = Uuid::now_v7().to_string();
        let wait_started_at = Instant::now();
        loop {
            if let Some(snapshot) = self.read_fresh_snapshot().await? {
                return Ok(snapshot);
            }
            if self
                .refresh_lock
                .acquire(&self.refresh_lock_key(), &owner, REFRESH_LOCK_TTL)
                .await?
            {
                return self.refresh_with_owned_lock(&owner).await;
            }
            if wait_started_at.elapsed() >= REFRESH_WAIT_TIMEOUT {
                return Err(anyhow!("runtime profile snapshot refresh timed out"));
            }
            tokio::time::sleep(REFRESH_WAIT_INTERVAL).await;
        }
    }

    async fn refresh_with_owned_lock(&self, owner: &str) -> Result<RuntimeProfilesSnapshot> {
        let refresh_result = async {
            if let Some(snapshot) = self.read_fresh_snapshot().await? {
                return Ok(snapshot);
            }
            self.refresh_snapshot().await
        }
        .await;
        let release_result = self
            .refresh_lock
            .release(&self.refresh_lock_key(), owner)
            .await;

        match refresh_result {
            Ok(snapshot) => {
                let released = release_result.context("release runtime profile refresh lock")?;
                if !released {
                    return Err(anyhow!("runtime profile refresh lock ownership was lost"));
                }
                Ok(snapshot)
            }
            Err(error) => {
                if let Err(release_error) = release_result {
                    tracing::warn!(
                        error = %release_error,
                        "failed to release runtime profile refresh lock after refresh error"
                    );
                }
                Err(error)
            }
        }
    }

    async fn read_fresh_snapshot(&self) -> Result<Option<RuntimeProfilesSnapshot>> {
        let (api_observation, runner_observation) = tokio::try_join!(
            self.read_target_observation(API_SERVER_TARGET_ID),
            self.read_target_observation(RUNTIME_HOST_TARGET_ID),
        )?;
        let (Some(api_observation), Some(runner_observation)) =
            (api_observation, runner_observation)
        else {
            return Ok(None);
        };
        let now = OffsetDateTime::now_utc();
        if !api_observation.is_fresh(now) || !runner_observation.is_fresh(now) {
            return Ok(None);
        }
        Ok(Some(RuntimeProfilesSnapshot {
            api_profile: api_observation.into_profile(),
            host_profile: runner_observation.into_profile(),
        }))
    }

    async fn read_target_observation(
        &self,
        target_id: &str,
    ) -> Result<Option<CachedRuntimeTargetObservation>> {
        let target_key = self.target_key(target_id);
        let Some(value) = self.cache_store.get_json(&target_key).await? else {
            return Ok(None);
        };
        let observation = match serde_json::from_value::<CachedRuntimeTargetObservation>(value) {
            Ok(observation) => observation,
            Err(error) => {
                tracing::warn!(
                    target_id,
                    error = %error,
                    "evicting invalid cached runtime target observation"
                );
                self.cache_store.delete(&target_key).await?;
                return Ok(None);
            }
        };
        if observation.target_id() != target_id {
            tracing::warn!(
                target_id,
                cached_target_id = observation.target_id(),
                "evicting mismatched cached runtime target observation"
            );
            self.cache_store.delete(&target_key).await?;
            return Ok(None);
        }
        Ok(Some(observation))
    }

    async fn refresh_snapshot(&self) -> Result<RuntimeProfilesSnapshot> {
        let api_profile = self.api_runtime_profile.collect_runtime_profile().await?;
        let host_profile = self.runtime_host_system.fetch_runtime_profile().await?;
        let observed_at = OffsetDateTime::now_utc();
        let api_observation =
            CachedRuntimeTargetObservation::reachable(api_profile.clone(), observed_at);
        let host_observation =
            CachedRuntimeTargetObservation::reachable(host_profile.clone(), observed_at);
        let api_value = serde_json::to_value(api_observation)?;
        let host_value = serde_json::to_value(host_observation)?;
        let api_key = self.target_key(API_SERVER_TARGET_ID);
        let host_key = self.target_key(RUNTIME_HOST_TARGET_ID);

        tokio::try_join!(
            self.cache_store
                .set_json(&api_key, api_value, Some(SNAPSHOT_RETENTION),),
            self.cache_store
                .set_json(&host_key, host_value, Some(SNAPSHOT_RETENTION),),
        )?;

        Ok(RuntimeProfilesSnapshot {
            api_profile,
            host_profile,
        })
    }

    fn target_key(&self, target_id: &str) -> String {
        format!(
            "system-runtime:v1:snapshot:{}:{}:{target_id}",
            self.api_node_id, self.runtime_instance_id
        )
    }

    fn refresh_lock_key(&self) -> String {
        format!(
            "system-runtime:v1:refresh:{}:{}",
            self.api_node_id, self.runtime_instance_id
        )
    }
}

#[cfg(test)]
mod _tests;
