use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use storage_ephemeral::{MokaReadThroughCache, ReadThroughCachePolicy};

fn policy(budget: u64, idle_ms: u64) -> ReadThroughCachePolicy {
    ReadThroughCachePolicy::new(budget, Duration::from_millis(idle_ms)).unwrap()
}

fn cache(budget: u64, idle_ms: u64) -> MokaReadThroughCache<String, Vec<u8>> {
    MokaReadThroughCache::new(policy(budget, idle_ms), |value: &Vec<u8>| {
        value.len() as u64
    })
    .unwrap()
}

#[test]
fn read_through_policy_rejects_invalid_resources() {
    assert!(ReadThroughCachePolicy::new(0, Duration::from_secs(1)).is_err());
    assert!(ReadThroughCachePolicy::new(1, Duration::ZERO).is_err());
    assert!(ReadThroughCachePolicy::new(1, Duration::from_secs(1000 * 365 * 24 * 3600)).is_err());
    assert!(MokaReadThroughCache::<String, Vec<u8>>::new(policy(10, 100), |_| 1).is_err());
}

#[tokio::test]
async fn read_through_concurrent_misses_share_one_load() {
    let cache = cache(1024, 1000);
    let calls = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(tokio::sync::Barrier::new(20));
    let mut jobs = Vec::new();
    for _ in 0..20 {
        let (cache, calls, barrier) = (cache.clone(), calls.clone(), barrier.clone());
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            cache
                .get_or_load("same".into(), async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    vec![7; 64]
                })
                .await
        }));
    }
    let first = jobs.remove(0).await.unwrap();
    for job in jobs {
        assert!(Arc::ptr_eq(&first, &job.await.unwrap()));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn read_through_hits_extend_idle_and_no_traffic_maintenance_releases_values() {
    let cache = cache(1024, 120);
    let first = cache
        .get_or_load("same".into(), async { vec![7; 64] })
        .await;
    let weak = Arc::downgrade(&first);
    tokio::time::sleep(Duration::from_millis(80)).await;
    let second = cache
        .get_or_load("same".into(), async { panic!("unexpected reload") })
        .await;
    assert!(Arc::ptr_eq(&first, &second));
    tokio::time::sleep(Duration::from_millis(80)).await;
    let third = cache
        .get_or_load("same".into(), async { panic!("idle was not extended") })
        .await;
    assert!(Arc::ptr_eq(&first, &third));
    drop((first, second, third));
    tokio::time::sleep(Duration::from_millis(400)).await;
    // No get or manual maintenance since the final hit.
    assert_eq!(cache.entry_count(), 0);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        cache
            .get_or_load("same".into(), async { vec![9] })
            .await
            .as_ref(),
        &[9]
    );
}

#[tokio::test]
async fn read_through_oversized_values_bypass_without_losing_the_response() {
    let cache = cache(2, 1000);
    let first = cache
        .get_or_load("big".into(), async { vec![3; 100] })
        .await;
    let second = cache
        .get_or_load("big".into(), async { vec![4; 100] })
        .await;
    assert_eq!(first.as_ref(), &[3; 100]);
    assert_eq!(second.as_ref(), &[4; 100]);
    cache.run_pending_tasks().await;
    assert_eq!(cache.entry_count(), 0);
    let overflow =
        MokaReadThroughCache::new(policy(u64::MAX, 1000), |_: &Vec<u8>| u64::MAX).unwrap();
    assert_eq!(
        overflow
            .get_or_load("key", async { vec![5] })
            .await
            .as_ref(),
        &[5]
    );
    overflow.run_pending_tasks().await;
    assert_eq!(overflow.entry_count(), 0);
}

#[tokio::test]
async fn read_through_eviction_and_last_owner_drop_preserve_request_references() {
    let cache = cache(32, 10_000);
    let retained = cache
        .get_or_load("original".into(), async { vec![7; 16] })
        .await;
    for key in 0..30 {
        cache
            .get_or_load(key.to_string(), async { vec![8; 16] })
            .await;
    }
    cache.run_pending_tasks().await;
    assert!(cache.weighted_size() <= 32);
    assert_eq!(retained.as_ref(), &[7; 16]);
    let last = cache
        .get_or_load("last".into(), async { vec![9; 16] })
        .await;
    let weak = Arc::downgrade(&last);
    let other_owner = cache.clone();
    drop(cache);
    assert_eq!(last.as_ref(), &[9; 16]);
    drop((other_owner, last));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        weak.upgrade().is_none(),
        "maintenance must not retain the last owner"
    );
    assert_eq!(retained.as_ref(), &[7; 16]);
}

#[tokio::test]
async fn read_through_cancelled_loader_can_be_retried() {
    let cache = cache(1024, 1000);
    let entered = Arc::new(tokio::sync::Notify::new());
    let loading = {
        let (cache, entered) = (cache.clone(), entered.clone());
        tokio::spawn(async move {
            cache
                .get_or_load("key".into(), async {
                    entered.notify_one();
                    std::future::pending::<Vec<u8>>().await
                })
                .await
        })
    };
    entered.notified().await;
    loading.abort();
    assert!(loading.await.unwrap_err().is_cancelled());
    let loaded = tokio::time::timeout(
        Duration::from_secs(1),
        cache.get_or_load("key".into(), async { vec![1] }),
    )
    .await
    .unwrap();
    assert_eq!(loaded.as_ref(), &[1]);
}
