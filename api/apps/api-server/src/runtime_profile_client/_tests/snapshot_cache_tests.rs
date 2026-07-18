use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use async_trait::async_trait;
use control_plane::ports::CacheStore;
use runtime_profile::RuntimeProfile;
use storage_ephemeral::{MemoryDistributedLock, MokaCacheStore};
use time::OffsetDateTime;

use super::super::{ApiRuntimeProfilePort, PluginRunnerSystemPort, RuntimeProfileSnapshotCache};
use crate::_tests::support::{sample_api_profile, sample_runner_profile};

#[derive(Clone)]
struct CountingApiRuntimeProfileCollector {
    calls: Arc<AtomicUsize>,
    delay: Duration,
    profile: RuntimeProfile,
}

#[async_trait]
impl ApiRuntimeProfilePort for CountingApiRuntimeProfileCollector {
    async fn collect_runtime_profile(&self) -> anyhow::Result<RuntimeProfile> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        Ok(self.profile.clone())
    }
}

#[derive(Clone)]
struct CountingPluginRunnerSystemClient {
    calls: Arc<AtomicUsize>,
    delay: Duration,
    profile: Option<RuntimeProfile>,
}

#[async_trait]
impl PluginRunnerSystemPort for CountingPluginRunnerSystemClient {
    async fn fetch_runtime_profile(&self) -> anyhow::Result<RuntimeProfile> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        self.profile
            .clone()
            .ok_or_else(|| anyhow::anyhow!("plugin runner unavailable"))
    }
}

struct SnapshotCacheFixture {
    cache_store: Arc<MokaCacheStore>,
    api_calls: Arc<AtomicUsize>,
    runner_calls: Arc<AtomicUsize>,
    snapshots: RuntimeProfileSnapshotCache,
}

fn snapshot_cache_fixture(delay: Duration) -> SnapshotCacheFixture {
    snapshot_cache_fixture_with_runner(delay, Some(sample_runner_profile("host-same")))
}

fn snapshot_cache_fixture_with_runner(
    delay: Duration,
    runner_profile: Option<RuntimeProfile>,
) -> SnapshotCacheFixture {
    let cache_store = Arc::new(MokaCacheStore::new("runtime-profile-test", 32));
    let api_calls = Arc::new(AtomicUsize::new(0));
    let runner_calls = Arc::new(AtomicUsize::new(0));
    let process_started_at = OffsetDateTime::now_utc();
    let snapshots = RuntimeProfileSnapshotCache::new(
        cache_store.clone(),
        Arc::new(MemoryDistributedLock::new("runtime-profile-test-lock")),
        Arc::new(CountingApiRuntimeProfileCollector {
            calls: api_calls.clone(),
            delay,
            profile: sample_api_profile("host-same"),
        }),
        Arc::new(CountingPluginRunnerSystemClient {
            calls: runner_calls.clone(),
            delay,
            profile: runner_profile,
        }),
        "api-node-test",
        process_started_at,
    );

    SnapshotCacheFixture {
        cache_store,
        api_calls,
        runner_calls,
        snapshots,
    }
}

#[tokio::test]
async fn ac_001_reuses_fresh_runtime_target_snapshots_and_exposes_them_for_inspection() {
    let fixture = snapshot_cache_fixture(Duration::ZERO);

    let first = fixture.snapshots.get_or_refresh().await.unwrap();
    let second = fixture.snapshots.get_or_refresh().await.unwrap();

    assert_eq!(first.api_profile.service, "api-server");
    assert_eq!(first.runner_profile.unwrap().service, "plugin-runner");
    assert_eq!(second.api_profile.service, "api-server");
    assert_eq!(fixture.api_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.runner_calls.load(Ordering::SeqCst), 1);

    let entries = CacheStore::list_ephemeral_entries(fixture.cache_store.as_ref())
        .await
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| {
        entry.contract_code == "cache-store"
            && entry.inspection_path.first().map(String::as_str) == Some("system-runtime")
            && entry.ttl_seconds.is_some_and(|ttl| (1..=10).contains(&ttl))
    }));
    assert!(entries.iter().any(|entry| entry
        .inspection_path
        .iter()
        .any(|part| part == "api-server")));
    assert!(entries.iter().any(|entry| entry
        .inspection_path
        .iter()
        .any(|part| part == "plugin-runner")));
}

#[tokio::test]
async fn ac_002_refreshes_runtime_target_snapshots_after_one_second() {
    let fixture = snapshot_cache_fixture(Duration::ZERO);

    fixture.snapshots.get_or_refresh().await.unwrap();
    tokio::time::sleep(Duration::from_millis(1_050)).await;
    let retained_entries = CacheStore::list_ephemeral_entries(fixture.cache_store.as_ref())
        .await
        .unwrap();
    assert_eq!(retained_entries.len(), 2);
    fixture.snapshots.get_or_refresh().await.unwrap();

    assert_eq!(fixture.api_calls.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.runner_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn ac_003_coalesces_concurrent_runtime_snapshot_refreshes() {
    let fixture = snapshot_cache_fixture(Duration::from_millis(50));

    let (first, second, third, fourth) = tokio::join!(
        fixture.snapshots.get_or_refresh(),
        fixture.snapshots.get_or_refresh(),
        fixture.snapshots.get_or_refresh(),
        fixture.snapshots.get_or_refresh(),
    );

    first.unwrap();
    second.unwrap();
    third.unwrap();
    fourth.unwrap();
    assert_eq!(fixture.api_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.runner_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ac_004_caches_plugin_runner_unreachable_observations() {
    let fixture = snapshot_cache_fixture_with_runner(Duration::ZERO, None);

    let first = fixture.snapshots.get_or_refresh().await.unwrap();
    let second = fixture.snapshots.get_or_refresh().await.unwrap();

    assert!(first.runner_profile.is_none());
    assert!(second.runner_profile.is_none());
    assert_eq!(fixture.api_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.runner_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ac_005_evicts_invalid_cached_observations_before_recollecting() {
    let fixture = snapshot_cache_fixture(Duration::ZERO);
    fixture
        .cache_store
        .set_json(
            &fixture.snapshots.target_key("api-server"),
            serde_json::json!({ "invalid": true }),
            Some(time::Duration::seconds(10)),
        )
        .await
        .unwrap();

    let snapshot = fixture.snapshots.get_or_refresh().await.unwrap();

    assert_eq!(snapshot.api_profile.service, "api-server");
    assert_eq!(fixture.api_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.runner_calls.load(Ordering::SeqCst), 1);
}
