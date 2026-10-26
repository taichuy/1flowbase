use std::{future::Future, hash::Hash, sync::Arc, time::Duration};

use anyhow::{ensure, Context, Result};
use moka::future::Cache;
use tokio::{runtime::Handle, task::JoinHandle, time::Instant};

/// Resource policy for disposable copies, never a limit on source data or requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadThroughCachePolicy {
    max_weight_bytes: u64,
    idle_timeout: Duration,
}

impl ReadThroughCachePolicy {
    pub fn new(max_weight_bytes: u64, idle_timeout: Duration) -> Result<Self> {
        ensure!(max_weight_bytes > 0, "cache weight budget must be positive");
        ensure!(
            !idle_timeout.is_zero(),
            "cache idle timeout must be positive"
        );
        // Moka rejects expiration durations at this boundary (1000 years).
        ensure!(
            idle_timeout < Duration::from_secs(1000 * 365 * 24 * 3600),
            "cache idle timeout exceeds the supported expiration range"
        );
        Ok(Self {
            max_weight_bytes,
            idle_timeout,
        })
    }

    pub fn max_weight_bytes(self) -> u64 {
        self.max_weight_bytes
    }

    pub fn idle_timeout(self) -> Duration {
        self.idle_timeout
    }
}

struct Entry<V> {
    value: Arc<V>,
    weight: u32,
}

impl<V> Clone for Entry<V> {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
            weight: self.weight,
        }
    }
}

struct Owner<K, V> {
    cache: Cache<K, Entry<V>>,
    maintenance: JoinHandle<()>,
    policy: ReadThroughCachePolicy,
    weigh: fn(&V) -> u64,
}

impl<K, V> Drop for Owner<K, V> {
    fn drop(&mut self) {
        self.maintenance.abort();
    }
}

/// Shared read-through values with Moka admission/eviction and idle maintenance.
/// Weight is caller-declared payload size, not allocator usage or an RSS bound.
pub struct MokaReadThroughCache<K, V> {
    owner: Arc<Owner<K, V>>,
}

impl<K, V> Clone for MokaReadThroughCache<K, V> {
    fn clone(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
        }
    }
}

impl<K, V> std::fmt::Debug for MokaReadThroughCache<K, V> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MokaReadThroughCache")
            .field("policy", &self.owner.policy)
            .finish_non_exhaustive()
    }
}

impl<K, V> MokaReadThroughCache<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    pub fn new(policy: ReadThroughCachePolicy, weigh: fn(&V) -> u64) -> Result<Self> {
        let runtime =
            Handle::try_current().context("read-through cache requires a Tokio runtime")?;
        let cache = Cache::builder()
            .max_capacity(policy.max_weight_bytes)
            .time_to_idle(policy.idle_timeout)
            .weigher(|_: &K, entry: &Entry<V>| entry.weight)
            .build();
        let maintained = cache.clone();
        let interval = (policy.idle_timeout / 2).max(Duration::from_nanos(1));
        // Capture only the cache, never its owner: the last owner must stop this task.
        let maintenance = runtime.spawn(async move {
            let mut timer = tokio::time::interval_at(Instant::now() + interval, interval);
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                timer.tick().await;
                maintained.run_pending_tasks().await;
            }
        });
        Ok(Self {
            owner: Arc::new(Owner {
                cache,
                maintenance,
                policy,
                weigh,
            }),
        })
    }

    /// Concurrent loads share a value; an uncacheable value is still returned whole.
    pub async fn get_or_load(&self, key: K, load: impl Future<Output = V> + Send) -> Arc<V> {
        let policy = self.owner.policy;
        let weigh = self.owner.weigh;
        match self
            .owner
            .cache
            .try_get_with(key, async move {
                let value = Arc::new(load.await);
                let weight = weigh(&value).max(1);
                if weight > policy.max_weight_bytes || weight > u64::from(u32::MAX) {
                    return Err(value);
                }
                Ok(Entry {
                    value,
                    weight: weight as u32,
                })
            })
            .await
        {
            Ok(entry) => entry.value,
            Err(value) => Arc::clone(value.as_ref()),
        }
    }

    pub async fn run_pending_tasks(&self) {
        self.owner.cache.run_pending_tasks().await;
    }

    pub fn entry_count(&self) -> u64 {
        self.owner.cache.entry_count()
    }

    pub fn weighted_size(&self) -> u64 {
        self.owner.cache.weighted_size()
    }
}
