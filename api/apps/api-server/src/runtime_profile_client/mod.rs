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
const PLUGIN_RUNNER_TARGET_ID: &str = "plugin-runner";
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
pub trait PluginRunnerSystemPort: Send + Sync {
    async fn fetch_runtime_profile(&self) -> Result<RuntimeProfile>;
}

#[derive(Clone)]
pub struct HttpPluginRunnerSystemClient {
    base_url: String,
    client: reqwest::Client,
}

impl HttpPluginRunnerSystemClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl PluginRunnerSystemPort for HttpPluginRunnerSystemClient {
    async fn fetch_runtime_profile(&self) -> Result<RuntimeProfile> {
        self.client
            .get(format!(
                "{}/system/runtime-profile",
                self.base_url.trim_end_matches('/')
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProfilesSnapshot {
    pub api_profile: RuntimeProfile,
    pub runner_profile: Option<RuntimeProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CachedRuntimeTargetObservation {
    Reachable {
        observed_at: OffsetDateTime,
        profile: Box<RuntimeProfile>,
    },
    Unreachable {
        observed_at: OffsetDateTime,
        target_id: String,
    },
}

impl CachedRuntimeTargetObservation {
    fn reachable(profile: RuntimeProfile, observed_at: OffsetDateTime) -> Self {
        Self::Reachable {
            observed_at,
            profile: Box::new(profile),
        }
    }

    fn unreachable(target_id: impl Into<String>, observed_at: OffsetDateTime) -> Self {
        Self::Unreachable {
            observed_at,
            target_id: target_id.into(),
        }
    }

    fn observed_at(&self) -> OffsetDateTime {
        match self {
            Self::Reachable { observed_at, .. } | Self::Unreachable { observed_at, .. } => {
                *observed_at
            }
        }
    }

    fn target_id(&self) -> &str {
        match self {
            Self::Reachable { profile, .. } => &profile.service,
            Self::Unreachable { target_id, .. } => target_id,
        }
    }

    fn is_fresh(&self, now: OffsetDateTime) -> bool {
        let age = now - self.observed_at();
        age >= Duration::ZERO && age <= SNAPSHOT_FRESHNESS
    }

    fn into_profile(self) -> Option<RuntimeProfile> {
        match self {
            Self::Reachable { profile, .. } => Some(*profile),
            Self::Unreachable { .. } => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeProfileSnapshotCache {
    cache_store: Arc<dyn CacheStore>,
    refresh_lock: Arc<dyn DistributedLock>,
    api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
    api_node_id: String,
    runtime_instance_id: String,
}

impl RuntimeProfileSnapshotCache {
    pub(crate) fn new(
        cache_store: Arc<dyn CacheStore>,
        refresh_lock: Arc<dyn DistributedLock>,
        api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
        plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
        api_node_id: impl Into<String>,
        process_started_at: OffsetDateTime,
    ) -> Self {
        Self {
            cache_store,
            refresh_lock,
            api_runtime_profile,
            plugin_runner_system,
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
            self.read_target_observation(PLUGIN_RUNNER_TARGET_ID),
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
        let Some(api_profile) = api_observation.into_profile() else {
            return Err(anyhow!("api-server runtime target cannot be unreachable"));
        };

        Ok(Some(RuntimeProfilesSnapshot {
            api_profile,
            runner_profile: runner_observation.into_profile(),
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
        let runner_profile = self.plugin_runner_system.fetch_runtime_profile().await.ok();
        let observed_at = OffsetDateTime::now_utc();
        let api_observation =
            CachedRuntimeTargetObservation::reachable(api_profile.clone(), observed_at);
        let runner_observation = match runner_profile.as_ref() {
            Some(profile) => {
                CachedRuntimeTargetObservation::reachable(profile.clone(), observed_at)
            }
            None => {
                CachedRuntimeTargetObservation::unreachable(PLUGIN_RUNNER_TARGET_ID, observed_at)
            }
        };
        let api_value = serde_json::to_value(api_observation)?;
        let runner_value = serde_json::to_value(runner_observation)?;
        let api_key = self.target_key(API_SERVER_TARGET_ID);
        let runner_key = self.target_key(PLUGIN_RUNNER_TARGET_ID);

        tokio::try_join!(
            self.cache_store
                .set_json(&api_key, api_value, Some(SNAPSHOT_RETENTION),),
            self.cache_store
                .set_json(&runner_key, runner_value, Some(SNAPSHOT_RETENTION),),
        )?;

        Ok(RuntimeProfilesSnapshot {
            api_profile,
            runner_profile,
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
