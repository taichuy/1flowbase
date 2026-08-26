use control_plane_contracts::ports::{EphemeralValueRevealMode, RateLimitStore};
use storage_ephemeral::MokaRateLimitStore;
use time::Duration;

#[tokio::test]
async fn moka_rate_limit_store_counts_inside_window() {
    let store = MokaRateLimitStore::new("flowbase:rate", 128);

    let first = store
        .consume("actor:1", 2, Duration::seconds(60))
        .await
        .unwrap();
    let second = store
        .consume("actor:1", 2, Duration::seconds(60))
        .await
        .unwrap();
    let third = store
        .consume("actor:1", 2, Duration::seconds(60))
        .await
        .unwrap();

    assert!(first.allowed);
    assert_eq!(first.remaining, 1);
    assert!(first.reset_after_ms > 0);
    assert!(second.allowed);
    assert_eq!(second.remaining, 0);
    assert!(!third.allowed);
    assert_eq!(third.remaining, 0);
    assert!(third.reset_after_ms > 0);
}

#[tokio::test]
async fn moka_rate_limit_store_resets_key() {
    let store = MokaRateLimitStore::new("flowbase:rate", 128);

    assert!(
        store
            .consume("actor:1", 1, Duration::seconds(60))
            .await
            .unwrap()
            .allowed
    );
    assert!(
        !store
            .consume("actor:1", 1, Duration::seconds(60))
            .await
            .unwrap()
            .allowed
    );

    store.reset("actor:1").await.unwrap();

    let after_reset = store
        .consume("actor:1", 1, Duration::seconds(60))
        .await
        .unwrap();
    assert!(after_reset.allowed);
    assert_eq!(after_reset.remaining, 0);
}

#[tokio::test]
async fn moka_rate_limit_store_starts_new_window_after_expiry() {
    let store = MokaRateLimitStore::new("flowbase:rate", 128);

    assert!(
        store
            .consume("actor:1", 1, Duration::milliseconds(30))
            .await
            .unwrap()
            .allowed
    );
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let next_window = store
        .consume("actor:1", 1, Duration::milliseconds(30))
        .await
        .unwrap();
    assert!(next_window.allowed);
    assert_eq!(next_window.remaining, 0);
}

#[tokio::test]
async fn moka_rate_limit_store_exposes_ephemeral_inspection_snapshots() {
    let store = MokaRateLimitStore::new("flowbase:rate", 128);

    store
        .consume("actor:1", 3, Duration::seconds(60))
        .await
        .unwrap();

    let capabilities = store.ephemeral_inspection_capabilities();
    assert!(capabilities.list_entries);
    assert!(capabilities.reveal_value);
    let entries = store.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contract_code, "rate-limit-store");
    assert_eq!(entries[0].key, "actor:1");
    assert!(!entries[0].sensitive);
    assert_eq!(entries[0].metadata["count"], 1);

    let revealed = store
        .reveal_ephemeral_entry("actor:1", EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata.key, "actor:1");
    assert_eq!(revealed.value.unwrap()["count"], 1);
}
