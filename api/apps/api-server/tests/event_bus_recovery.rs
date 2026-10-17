// Exercise the actual composition without linking the server's complete fixture tree.
#[path = "../src/host_infrastructure/recovery_event_bus.rs"]
mod recovery_event_bus;

use control_plane_contracts::ports::{
    EphemeralValueRevealMode, EventBus, ephemeral_metadata_size_bytes,
};
use recovery_event_bus::RecoveryEventBus;
use serde_json::json;
use storage_durable_postgres::PgLegacyEventRecovery;

#[tokio::test]
async fn new_live_publication_does_not_require_a_durable_connection() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let bus = RecoveryEventBus::new(pool);
    bus.publish("live", json!({"full":"body\u{0}"}))
        .await
        .unwrap();
}

#[tokio::test]
async fn legacy_recovery_is_visible_lossless_and_precedes_live_without_new_durable_writes() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    let old = json!({"nul\u{0}key":[u64::MAX,"before\u{0}after",null]});
    PgLegacyEventRecovery::new(pool.clone())
        .publish("same", old.clone())
        .await
        .unwrap();
    let bus = RecoveryEventBus::new(pool.clone());
    let live = json!({"new":"完整正文"});
    bus.publish("same", live.clone()).await.unwrap();
    let counts: (i64,i64) = sqlx::query_as("select (select count(*) from legacy_event_recovery_pending),(select last_sequence from legacy_event_recovery_state)")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1));
    let entries = bus.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 2);
    let legacy = entries
        .iter()
        .find(|e| e.entry_ref.starts_with("legacy:"))
        .unwrap();
    let new = entries
        .iter()
        .find(|e| !e.entry_ref.starts_with("legacy:"))
        .unwrap();
    assert_ne!(legacy.key, new.key);
    assert_ne!(legacy.inspection_path, new.inspection_path);
    assert_eq!(legacy.owner.as_deref(), Some("legacy-event-recovery"));
    assert_eq!(
        legacy.metadata_size_bytes,
        ephemeral_metadata_size_bytes(&legacy.metadata)
    );
    let revealed = bus
        .reveal_ephemeral_entry(&legacy.entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata, *legacy);
    assert_eq!(revealed.value, Some(old.clone()));
    assert_eq!(bus.poll("same").await.unwrap(), Some(old));
    assert_eq!(bus.poll("same").await.unwrap(), Some(live));
    assert!(bus.poll("same").await.unwrap().is_none());
    assert!(bus.list_ephemeral_entries().await.unwrap().is_empty());
    let sequence: i64 = sqlx::query_scalar("select last_sequence from legacy_event_recovery_state")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sequence, 1);
    // Switching provider must not introduce the legacy MemoryEventBus 1 MiB cap.
    let large = json!({"nul\u{0}key":[u64::MAX,"原文\u{0}".repeat(200_000),null]});
    assert!(serde_json::to_vec(&large).unwrap().len() > 1024 * 1024);
    assert_eq!(
        bus.ephemeral_inspection_capabilities()
            .max_payload_size_bytes,
        u64::MAX
    );
    bus.publish("large", large.clone()).await.unwrap();
    assert_eq!(bus.poll("large").await.unwrap(), Some(large));
    let durable:(i64,i64)=sqlx::query_as("select (select count(*) from legacy_event_recovery_pending),(select last_sequence from legacy_event_recovery_state)")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(durable, (0, 1));
}
