use control_plane::ports::CacheStore;
use serde_json::json;
use storage_ephemeral::{EphemeralKvStore, MokaCacheStore};
use time::Duration;

#[tokio::test]
async fn moka_cache_store_reads_writes_and_expires_json() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    CacheStore::set_json(
        &store,
        "catalog:1",
        json!({ "items": 1 }),
        Some(Duration::milliseconds(30)),
    )
    .await
    .unwrap();
    assert_eq!(
        CacheStore::get_json(&store, "catalog:1").await.unwrap(),
        Some(json!({ "items": 1 }))
    );

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert_eq!(
        CacheStore::get_json(&store, "catalog:1").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn moka_cache_store_does_not_make_non_positive_ttl_immortal() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    CacheStore::set_json(
        &store,
        "expired",
        json!({ "value": true }),
        Some(Duration::seconds(-1)),
    )
    .await
    .unwrap();

    assert_eq!(CacheStore::get_json(&store, "expired").await.unwrap(), None);

    assert!(EphemeralKvStore::set_if_absent_json(
        &store,
        "lease",
        json!({ "owner": "a" }),
        Some(Duration::ZERO),
    )
    .await
    .unwrap());
    assert_eq!(CacheStore::get_json(&store, "lease").await.unwrap(), None);
}

#[tokio::test]
async fn moka_cache_store_touch_with_non_positive_ttl_clears_entry() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    CacheStore::set_json(&store, "manifest:1", json!({ "parsed": true }), None)
        .await
        .unwrap();

    assert!(!CacheStore::touch(&store, "manifest:1", Duration::ZERO)
        .await
        .unwrap());
    assert_eq!(
        CacheStore::get_json(&store, "manifest:1").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn moka_cache_store_extends_ttl_with_touch() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    CacheStore::set_json(
        &store,
        "manifest:1",
        json!({ "parsed": true }),
        Some(Duration::milliseconds(40)),
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    assert!(
        CacheStore::touch(&store, "manifest:1", Duration::milliseconds(120))
            .await
            .unwrap()
    );
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    assert_eq!(
        CacheStore::get_json(&store, "manifest:1").await.unwrap(),
        Some(json!({ "parsed": true }))
    );
}

#[tokio::test]
async fn moka_cache_store_supports_ephemeral_set_if_absent() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    assert!(
        EphemeralKvStore::set_if_absent_json(&store, "lease", json!({ "owner": "a" }), None)
            .await
            .unwrap()
    );
    assert!(
        !EphemeralKvStore::set_if_absent_json(&store, "lease", json!({ "owner": "b" }), None)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn moka_cache_store_increments_counter_atomically_and_expires_with_ttl() {
    // AC-004: counter primitive is a store-level atomic increment with TTL.
    let store = MokaCacheStore::new("flowbase:test", 128);
    let key = "llm-router:workspace:w1:provider:p1:model:m1:targets:t1";

    let first = CacheStore::increment_counter(&store, key, 1, Some(Duration::milliseconds(80)))
        .await
        .unwrap();
    let second = CacheStore::increment_counter(&store, key, 1, Some(Duration::milliseconds(80)))
        .await
        .unwrap();

    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_eq!(
        CacheStore::get_json(&store, key).await.unwrap(),
        Some(json!(2))
    );

    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    assert_eq!(CacheStore::get_json(&store, key).await.unwrap(), None);
    assert_eq!(
        CacheStore::increment_counter(&store, key, 1, None)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn moka_cache_store_rejects_oversized_ephemeral_values() {
    let store = MokaCacheStore::new("flowbase:test", 128);
    let oversized = json!({ "blob": "x".repeat(2 * 1024 * 1024) });

    let set_result = CacheStore::set_json(&store, "large:value", oversized.clone(), None).await;
    assert!(set_result.is_err());
    assert_eq!(
        CacheStore::get_json(&store, "large:value").await.unwrap(),
        None
    );

    let set_if_absent_result =
        EphemeralKvStore::set_if_absent_json(&store, "large:lease", oversized, None).await;
    assert!(set_if_absent_result.is_err());
    assert_eq!(
        CacheStore::get_json(&store, "large:lease").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn moka_cache_store_exposes_cache_inspection_snapshots() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    CacheStore::set_json(
        &store,
        "application-logs:run:1",
        json!({ "status": "succeeded" }),
        Some(Duration::seconds(60)),
    )
    .await
    .unwrap();
    CacheStore::set_json(
        &store,
        "runtime-records:contact:1",
        json!({ "name": "Ada" }),
        None,
    )
    .await
    .unwrap();

    assert!(CacheStore::inspection_capabilities(&store).reveal_value);
    let domains = CacheStore::list_cache_domains(&store).await.unwrap();
    assert_eq!(domains.len(), 2);
    assert!(domains.iter().any(|domain| {
        domain.domain_code == "application-logs"
            && domain.entry_count == 1
            && domain.total_value_size_bytes > 0
    }));

    let entries = CacheStore::list_cache_entries(&store, "application-logs")
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "application-logs:run:1");
    assert_eq!(entries[0].domain_code, "application-logs");
    let ttl_seconds = entries[0].ttl_seconds.unwrap();
    assert!(ttl_seconds > 0);
    assert!(ttl_seconds <= 60);
    assert!(entries[0].created_at_unix.is_some());
    assert!(entries[0].expires_at_unix.is_some());

    let value =
        CacheStore::reveal_cache_entry(&store, "application-logs", "application-logs:run:1")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(value.value, json!({ "status": "succeeded" }));
}

#[tokio::test]
async fn moka_cache_store_clears_entry_and_domain_through_inspection() {
    let store = MokaCacheStore::new("flowbase:test", 128);

    CacheStore::set_json(&store, "application-logs:run:1", json!({ "a": 1 }), None)
        .await
        .unwrap();
    CacheStore::set_json(&store, "application-logs:run:2", json!({ "a": 2 }), None)
        .await
        .unwrap();
    CacheStore::set_json(&store, "runtime-records:row:1", json!({ "b": 1 }), None)
        .await
        .unwrap();

    assert!(
        CacheStore::clear_cache_entry(&store, "application-logs", "application-logs:run:1")
            .await
            .unwrap()
    );
    assert_eq!(
        CacheStore::reveal_cache_entry(&store, "application-logs", "application-logs:run:1")
            .await
            .unwrap(),
        None
    );

    assert_eq!(
        CacheStore::clear_cache_domain(&store, "application-logs")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        CacheStore::list_cache_entries(&store, "application-logs")
            .await
            .unwrap(),
        Vec::new()
    );
    assert_eq!(
        CacheStore::list_cache_entries(&store, "runtime-records")
            .await
            .unwrap()
            .len(),
        1
    );
}
