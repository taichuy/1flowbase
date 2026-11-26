use serde_json::json;
use storage_ephemeral::{EphemeralKvStore, MemoryKvStore};
use time::Duration;

#[tokio::test]
async fn memory_kv_reclaims_unvisited_expired_tokens_and_retains_touched_keys() {
    let store = MemoryKvStore::new("file-access-test");
    for index in 0..20 {
        store
            .set_json(
                &format!("expired:{index}"),
                json!(index),
                Some(Duration::milliseconds(10)),
            )
            .await
            .unwrap();
    }
    store
        .set_json("renewed", json!(true), Some(Duration::milliseconds(10)))
        .await
        .unwrap();
    assert!(store.touch("renewed", Duration::seconds(60)).await.unwrap());
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    store
        .set_json("new", json!(true), Some(Duration::seconds(60)))
        .await
        .unwrap();
    assert_eq!(store.entry_count_for_test().await, 2);
    assert_eq!(store.get_json("renewed").await.unwrap(), Some(json!(true)));
}

#[tokio::test]
async fn memory_kv_store_expires_entries_on_read() {
    let store = MemoryKvStore::new("flowbase:test");
    store
        .set_json(
            "session:1",
            json!({ "ok": true }),
            Some(Duration::seconds(1)),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    assert_eq!(store.get_json("session:1").await.unwrap(), None);
}

#[tokio::test]
async fn memory_kv_store_set_if_absent_only_writes_once() {
    let store = MemoryKvStore::new("flowbase:test");

    assert!(store
        .set_if_absent_json("key", json!({ "value": 1 }), None)
        .await
        .unwrap());
    assert!(!store
        .set_if_absent_json("key", json!({ "value": 2 }), None)
        .await
        .unwrap());
}

#[tokio::test]
async fn memory_kv_store_increments_counter_from_zero_and_respects_ttl() {
    // AC-004: missing counter keys start at 0 and increment atomically inside the store.
    let store = MemoryKvStore::new("flowbase:test");

    assert_eq!(
        store
            .increment_counter("counter:short", 1, Some(Duration::milliseconds(60)))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .increment_counter("counter:short", 2, None)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        store.get_json("counter:short").await.unwrap(),
        Some(json!(3))
    );

    tokio::time::sleep(std::time::Duration::from_millis(90)).await;
    assert_eq!(store.get_json("counter:short").await.unwrap(), None);
}

#[tokio::test]
async fn memory_kv_store_prefixes_namespace_and_extends_ttl() {
    let store = MemoryKvStore::new("flowbase:test");
    store
        .set_json(
            "session:2",
            json!({ "ok": true }),
            Some(Duration::milliseconds(50)),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    assert!(store
        .touch("session:2", Duration::milliseconds(120))
        .await
        .unwrap());
    assert_eq!(
        store.raw_key_for_test("session:2"),
        "flowbase:test:session:2".to_string()
    );

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert_eq!(
        store.get_json("session:2").await.unwrap(),
        Some(json!({ "ok": true }))
    );
}

#[test]
fn public_exports_match_capability_names() {
    use storage_ephemeral::{EphemeralKvStore, MemoryKvStore};

    let _ = std::any::type_name::<MemoryKvStore>();
    let _ = std::any::type_name::<&dyn EphemeralKvStore>();
}

#[test]
fn moka_session_store_default_is_public() {
    let _ = storage_ephemeral::MokaSessionStore::default();
}
