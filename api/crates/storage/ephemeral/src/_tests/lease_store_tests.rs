use storage_ephemeral::{LeaseStore, MemoryLeaseStore};
use time::Duration;

#[tokio::test]
async fn memory_lease_store_allows_single_owner_until_expiry() {
    let store = MemoryLeaseStore::new("flowbase:lease");

    assert!(store
        .acquire("flow-run:1", "worker-a", Duration::seconds(30))
        .await
        .unwrap());
    assert!(!store
        .acquire("flow-run:1", "worker-b", Duration::seconds(30))
        .await
        .unwrap());
}

#[tokio::test]
async fn memory_lease_store_renews_owner_lease() {
    let store = MemoryLeaseStore::new("flowbase:lease");

    store
        .acquire("flow-run:2", "worker-a", Duration::seconds(1))
        .await
        .unwrap();
    assert!(store
        .renew("flow-run:2", "worker-a", Duration::seconds(30))
        .await
        .unwrap());
}
