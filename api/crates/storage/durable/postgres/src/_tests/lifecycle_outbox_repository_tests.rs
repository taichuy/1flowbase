use control_plane_contracts::ports::{
    LifecycleOutboxRepository, LifecycleOutboxStatus, RecordLifecycleFactInput,
};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn store() -> PgControlPlaneStore {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

#[tokio::test]
async fn lcf_005_outbox_claim_retry_delivery_and_idempotency_are_durable() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: br#"{"model_definition_id":"model-1"}"#.to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
    };
    let first = store.record_lifecycle_fact(&input).await.unwrap();
    let replay = store.record_lifecycle_fact(&input).await.unwrap();
    assert_eq!(first, replay);

    let worker = Uuid::now_v7();
    let claimed = store.claim_lifecycle_facts(worker, 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].status, LifecycleOutboxStatus::Claimed);
    assert_eq!(claimed[0].attempt_count, 1);

    store
        .retry_lifecycle_fact(
            input.event_id,
            worker,
            OffsetDateTime::now_utc() - Duration::seconds(1),
            "subscriber unavailable",
        )
        .await
        .unwrap();
    let claimed_again = store.claim_lifecycle_facts(worker, 10).await.unwrap();
    assert_eq!(claimed_again[0].attempt_count, 2);
    let delivered = store
        .mark_lifecycle_fact_delivered(input.event_id, worker)
        .await
        .unwrap();
    assert_eq!(delivered.status, LifecycleOutboxStatus::Delivered);
    assert!(store
        .claim_lifecycle_facts(worker, 10)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn lcf_005_same_event_id_with_different_fact_is_rejected() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: b"first".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let error = store
        .record_lifecycle_fact(&RecordLifecycleFactInput {
            canonical_payload: b"different".to_vec(),
            ..input
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("conflicts with a different fact"));
}
