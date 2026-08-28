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
        occurred_at: OffsetDateTime::from_unix_timestamp_nanos(1_700_000_000_123_456_789).unwrap(),
    };
    let first = store.record_lifecycle_fact(&input).await.unwrap();
    let replay = store.record_lifecycle_fact(&input).await.unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.occurred_at.nanosecond(), 123_456_000);

    let worker = Uuid::now_v7();
    let lease = Duration::seconds(30);
    let claimed = store
        .claim_lifecycle_facts(worker, 10, lease)
        .await
        .unwrap();
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
    let claimed_again = store
        .claim_lifecycle_facts(worker, 10, lease)
        .await
        .unwrap();
    assert_eq!(claimed_again[0].attempt_count, 2);
    let delivered = store
        .mark_lifecycle_fact_delivered(input.event_id, worker)
        .await
        .unwrap();
    assert_eq!(delivered.status, LifecycleOutboxStatus::Delivered);
    assert!(store
        .claim_lifecycle_facts(worker, 10, lease)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn lcf_qaf_stale_claim_is_recovered_by_a_new_worker() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "model_definition.committed".to_string(),
        contract_version: "v1".to_string(),
        canonical_payload: b"recovery".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let crashed_worker = Uuid::now_v7();
    store
        .claim_lifecycle_facts(crashed_worker, 1, Duration::seconds(30))
        .await
        .unwrap();
    sqlx::query("update lifecycle_outbox set claimed_at = now() - interval '31 seconds' where event_id = $1")
        .bind(input.event_id)
        .execute(store.pool())
        .await
        .unwrap();

    let recovery_worker = Uuid::now_v7();
    let recovered = store
        .claim_lifecycle_facts(recovery_worker, 1, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].claimed_by, Some(recovery_worker));
    assert_eq!(recovered[0].attempt_count, 2);
    assert!(store
        .mark_lifecycle_fact_delivered(input.event_id, crashed_worker)
        .await
        .is_err());
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
