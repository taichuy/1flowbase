use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use control_plane_contracts::{
    application_public_api::{ApplicationApiMappingConfig, ApplicationPublicationVersionRecord},
    ports::PublishedPublicationCache,
};
use storage_ephemeral::MokaPublishedPublicationCache;
use time::OffsetDateTime;
use uuid::Uuid;

fn publication(id: Uuid) -> ApplicationPublicationVersionRecord {
    ApplicationPublicationVersionRecord {
        id,
        application_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        flow_id: Uuid::new_v4(),
        flow_version_id: Uuid::new_v4(),
        mapping_snapshot: ApplicationApiMappingConfig::default_native(),
        extension_slug: None,
        compiled_plan_id: id,
        version_sequence: 1,
        active: true,
        api_enabled: true,
        flow_schema_version: "1".into(),
        document_hash: "hash".into(),
        document_snapshot: serde_json::json!({"nodes": [{"id": "start"}]}),
        runtime_profile_snapshot: serde_json::json!({}),
        output_selector: serde_json::json!({}),
        dependency_snapshot: Vec::new(),
        created_by: Uuid::new_v4(),
        created_at: OffsetDateTime::now_utc(),
    }
}

#[tokio::test]
async fn concurrent_requests_share_a_version_and_new_version_loads_separately() {
    let cache = Arc::new(MokaPublishedPublicationCache::new());
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
                            Ok(Some(publication(id)))
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

    // PostgreSQL reuses the publication row ID on republish, but creates a new plan ID.
    let next = Uuid::new_v4();
    let next_snapshot = cache
        .get_or_load(
            next,
            Box::new(move || {
                Box::pin(async move {
                    let mut republished = publication(id);
                    republished.compiled_plan_id = next;
                    Ok(Some(republished))
                })
            }),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_snapshot.id, id);
    assert_eq!(next_snapshot.compiled_plan_id, next);
    assert!(!Arc::ptr_eq(&first, &next_snapshot));
}

#[tokio::test]
async fn absent_publication_is_not_cached() {
    let cache = MokaPublishedPublicationCache::new();
    let id = Uuid::new_v4();
    assert!(cache
        .get_or_load(id, Box::new(|| Box::pin(async { Ok(None) })))
        .await
        .unwrap()
        .is_none());
    assert!(cache
        .get_or_load(
            id,
            Box::new(move || Box::pin(async move { Ok(Some(publication(id))) }))
        )
        .await
        .unwrap()
        .is_some());
}
