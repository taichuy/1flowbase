use crate::openapi_docs::{build_api_docs_registry, ApiDocsRegistry, SharedApiDocument};
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
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

async fn body(document: SharedApiDocument) -> Vec<u8> {
    let response = document.into_response();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
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
            serde_json::to_vec(&registry.category_spec(&category.id).unwrap()).unwrap(),
            body(registry.cached_category_spec(&category.id).await.unwrap()).await
        );
    }
    let first = registry.cached_operation_spec("read").await.unwrap();
    let original = registry.operation_spec("read").unwrap();
    assert_eq!(
        serde_json::to_vec(&original).unwrap(),
        body(first.clone()).await
    );
    assert_eq!(
        first.managed_projection(),
        super::super::managed_projection::DocsOutputProjection::OpenApi(&original)
            .project_for_managed_hook()
            .unwrap()
    );
    assert!(Arc::ptr_eq(
        &first.0,
        &registry.cached_operation_spec("read").await.unwrap().0
    ));
}

#[tokio::test]
async fn docs_cache_snapshot_and_owned_mutation_are_isolated() {
    let first = registry("string", 1_000_000);
    let next = registry("integer", 1_000_000);
    let held = first.cached_operation_spec("read").await.unwrap();
    let mut local = first.operation_spec("read").unwrap();
    local["info"]["title"] = json!("changed by request");
    let original: Value = serde_json::from_slice(&body(held.clone()).await).unwrap();
    assert_ne!(local["info"], original["info"]);
    assert!(Arc::ptr_eq(
        &held.0,
        &first.cached_operation_spec("read").await.unwrap().0
    ));
    let newer: Value =
        serde_json::from_slice(&body(next.cached_operation_spec("read").await.unwrap()).await)
            .unwrap();
    assert_ne!(original["components"], newer["components"]);
    drop(first);
    assert_eq!(
        serde_json::from_slice::<Value>(&body(held).await).unwrap()["components"]["schemas"]
            ["Result"]["type"],
        "string"
    );
}

#[tokio::test]
async fn docs_cache_concurrent_requests_share_and_oversized_docs_still_respond() {
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
        assert!(Arc::ptr_eq(&first.0, &job.await.unwrap().0));
    }
    let small = self::registry("string", 1);
    assert_eq!(
        body(small.cached_operation_spec("read").await.unwrap()).await,
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
            super::EncodedApiDocument::from(json!({"operation":true}))
        })
        .await;
    let category = cache
        .get_or_load(super::ScopeKey::Category("same".into()), async {
            super::EncodedApiDocument::from(json!({"category":true}))
        })
        .await;
    assert_ne!(operation.body, category.body);
}

#[tokio::test]
async fn docs_encoded_response_preserves_all_json_shapes_and_shallow_hook_summary() {
    for value in [
        Value::Null,
        json!(true),
        json!(123),
        json!("带转义\n\""),
        json!([1,{"nested":false}]),
        json!({"key":["private payload"]}),
    ] {
        let document = SharedApiDocument::from(value.clone());
        assert_eq!(
            document.managed_projection(),
            super::super::managed_projection::DocsOutputProjection::OpenApi(&value)
                .project_for_managed_hook()
                .unwrap()
        );
        assert_eq!(body(document).await, serde_json::to_vec(&value).unwrap());
    }
}

#[tokio::test]
async fn docs_default_cache_and_empty_template_have_distinct_lifetimes() {
    let default = super::default_policy().unwrap();
    assert_eq!(default.max_weight_bytes(), 1024 * 1024);
    assert_eq!(default.idle_timeout(), Duration::from_secs(60));
    let registry = Arc::new(registry("string", 1_000_000).with_default_cache().unwrap());
    let weak = Arc::downgrade(&registry);
    let held = registry.cached_operation_spec("read").await.unwrap();
    let empty = ApiDocsRegistry::empty();
    assert!(empty.document_cache.is_none());
    assert!(empty.catalog().categories.is_empty());
    drop(registry);
    assert!(weak.upgrade().is_none());
    assert!(!body(held).await.is_empty());
}

#[tokio::test]
async fn docs_encoded_idle_eviction_keeps_inflight_response_readable() {
    let registry = registry("string", 1_000_000)
        .with_cache_policy(Some(
            ReadThroughCachePolicy::new(1_000_000, Duration::from_millis(25)).unwrap(),
        ))
        .unwrap();
    let held = registry.cached_operation_spec("read").await.unwrap();
    let expected = body(held.clone()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    registry
        .document_cache
        .as_ref()
        .unwrap()
        .run_pending_tasks()
        .await;
    assert_eq!(registry.document_cache.as_ref().unwrap().entry_count(), 0);
    assert_eq!(body(held).await, expected);
}
