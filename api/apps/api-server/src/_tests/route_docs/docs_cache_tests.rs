use crate::openapi_docs::{build_api_docs_registry, ApiDocsRegistry, SharedApiDocument};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use storage_ephemeral::ReadThroughCachePolicy;

fn registry(kind: &str, budget: u64) -> ApiDocsRegistry {
    build_api_docs_registry(json!({
        "openapi":"3.1.0", "info":{"title":"Cache", "version":"1"},
        "paths":{"/read":{"get":{"operationId":"read", "tags":["Shared"],
            "responses":{"200":{"description":"ok", "content":{"application/json":{"schema":{"$ref":"#/components/schemas/Result"}}}}}}}},
        "components":{"schemas":{"Result":{"type":kind}}}
    })).unwrap().with_cache_policy(Some(ReadThroughCachePolicy::new(budget, Duration::from_secs(1)).unwrap())).unwrap()
}

#[tokio::test]
async fn docs_cache_preserves_full_bytes_and_does_not_fill_from_metadata_or_missing_ids() {
    let registry = registry("string", 1_000_000);
    registry.operation_spec_view("read").unwrap();
    assert!(registry.cached_operation_spec("missing").await.is_none());
    let cache = registry.document_cache.as_ref().unwrap();
    cache.run_pending_tasks().await;
    assert_eq!(cache.entry_count(), 0);
    for category in &registry.catalog().categories {
        assert_eq!(
            serde_json::to_vec(&registry.category_spec(&category.id)).unwrap(),
            serde_json::to_vec(&registry.cached_category_spec(&category.id).await.as_deref())
                .unwrap()
        );
    }
    let first = registry.cached_operation_spec("read").await.unwrap();
    assert_eq!(
        serde_json::to_vec(&registry.operation_spec("read").unwrap()).unwrap(),
        serde_json::to_vec(first.as_ref()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&SharedApiDocument::from(first.clone())).unwrap(),
        serde_json::to_vec(first.as_ref()).unwrap()
    );
    assert!(Arc::ptr_eq(
        &first,
        &registry.cached_operation_spec("read").await.unwrap()
    ));
}

#[tokio::test]
async fn docs_cache_snapshot_and_owned_mutation_are_isolated() {
    let first = registry("string", 1_000_000);
    let next = registry("integer", 1_000_000);
    let mut held = first.cached_operation_spec("read").await.unwrap();
    let original = held.clone();
    Arc::make_mut(&mut held)["info"]["title"] = json!("changed by request");
    assert_ne!(held["info"], original["info"]);
    assert!(Arc::ptr_eq(
        &original,
        &first.cached_operation_spec("read").await.unwrap()
    ));
    let newer = next.cached_operation_spec("read").await.unwrap();
    assert_ne!(original["components"], newer["components"]);
    drop(first);
    assert_eq!(
        original["components"]["schemas"]["Result"]["type"],
        "string"
    );
}

#[tokio::test]
async fn docs_cache_concurrent_requests_share_and_oversized_docs_still_serialize() {
    let registry = Arc::new(registry("string", 1_000_000));
    let mut jobs = Vec::new();
    for _ in 0..20 {
        let registry = registry.clone();
        jobs.push(tokio::spawn(async move {
            registry.cached_operation_spec("read").await.unwrap()
        }));
    }
    let first = jobs.remove(0).await.unwrap();
    for job in jobs {
        assert!(Arc::ptr_eq(&first, &job.await.unwrap()));
    }
    let small = self::registry("string", 1);
    let complete = small.cached_operation_spec("read").await.unwrap();
    assert_eq!(
        serde_json::to_vec(complete.as_ref()).unwrap(),
        serde_json::to_vec(&small.operation_spec("read").unwrap()).unwrap()
    );
    small
        .document_cache
        .as_ref()
        .unwrap()
        .run_pending_tasks()
        .await;
    assert_eq!(small.document_cache.as_ref().unwrap().entry_count(), 0);
}

#[tokio::test]
async fn docs_cache_category_and_operation_keys_cannot_collide() {
    let cache = super::DocumentCache::new(
        ReadThroughCachePolicy::new(1000, Duration::from_secs(1)).unwrap(),
        super::document_weight,
    )
    .unwrap();
    let operation = cache
        .get_or_load(super::ScopeKey::Operation("same".into()), async {
            json!({"operation":true})
        })
        .await;
    let category = cache
        .get_or_load(super::ScopeKey::Category("same".into()), async {
            json!({"category":true})
        })
        .await;
    assert_ne!(operation.as_ref(), category.as_ref());
}
