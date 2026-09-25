use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use control_plane_contracts::ports::PublishedPlanCache;
use storage_ephemeral::MokaPublishedPlanCache;
use time::OffsetDateTime;
use uuid::Uuid;

fn plan(id: Uuid) -> domain::CompiledPlanRecord {
    domain::CompiledPlanRecord {
        id,
        flow_id: Uuid::new_v4(),
        draft_id: Uuid::new_v4(),
        schema_version: "1".into(),
        document_hash: "hash".into(),
        document_updated_at: OffsetDateTime::now_utc(),
        plan: serde_json::json!({"nodes": [{"id": "start"}]}),
        created_by: Uuid::new_v4(),
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[tokio::test]
async fn concurrent_requests_share_one_immutable_plan_load() {
    let cache = Arc::new(MokaPublishedPlanCache::new());
    let id = Uuid::new_v4();
    let loads = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let cache = cache.clone();
        let loads = loads.clone();
        tasks.push(tokio::spawn(async move {
            cache
                .get_or_load(
                    id,
                    Box::new(move || {
                        Box::pin(async move {
                            loads.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            Ok(Some(plan(id)))
                        })
                    }),
                )
                .await
                .unwrap()
                .unwrap()
        }));
    }
    let first = tasks.remove(0).await.unwrap();
    for task in tasks {
        assert!(Arc::ptr_eq(&first, &task.await.unwrap()));
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn missing_and_failed_reads_are_not_cached() {
    let cache = MokaPublishedPlanCache::new();
    let id = Uuid::new_v4();
    assert!(cache
        .get_or_load(id, Box::new(|| Box::pin(async { Ok(None) })))
        .await
        .unwrap()
        .is_none());
    assert!(cache
        .get_or_load(
            id,
            Box::new(|| Box::pin(async { Err(anyhow::anyhow!("db unavailable")) }))
        )
        .await
        .is_err());
    let loaded = cache
        .get_or_load(
            id,
            Box::new(move || Box::pin(async move { Ok(Some(plan(id))) })),
        )
        .await
        .unwrap();
    assert!(loaded.is_some());
}
