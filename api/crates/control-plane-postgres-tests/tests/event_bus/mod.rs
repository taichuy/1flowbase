//! Service + real PostgreSQL handoff and selective backup/restore acceptance.
use control_plane::billing::handoff_legacy_credit_events;
use control_plane_contracts::{
    ports::{BillingRepository, EphemeralValueRevealMode, EventBus},
    system_backup::selective::{SelectiveBackupRepository, SelectiveBackupSelection},
};
use serde_json::json;
use storage_durable_postgres::{PgControlPlaneStore, PgEventBus, PgSelectiveBackupRepository};
use tokio::io::AsyncReadExt;

async fn pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    pool
}

async fn rejected_credit(pool: &sqlx::PgPool) -> uuid::Uuid {
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    // A fresh fixture row has no unrelated bootstrap notification dependencies;
    // changing the name passed to bootstrap would still seed those dependencies.
    let workspace = uuid::Uuid::now_v7();
    sqlx::query(
        "insert into workspaces(id,tenant_id,name) values ($1,$2,'Durable credit handoff fixture')",
    )
    .bind(workspace)
    .bind(tenant.id)
    .execute(pool)
    .await
    .unwrap();
    store
        .record_credit_command_rejected(
            workspace,
            "fixture.plugin",
            "grant",
            "permission_denied",
            "fixture:rejected",
        )
        .await
        .unwrap();
    // Current optional-zero rejection retains a completed fact, with no future
    // transport obligation. This recovery fixture explicitly converts only its
    // own source row into the pre-cutover unpublished state.
    let (event_id, _published_at): (uuid::Uuid, time::OffsetDateTime) = sqlx::query_as(
        "select event_id,published_at from credit_event_outbox where workspace_id=$1 and event_type='CreditCommandRejected' and payload->>'idempotency_key'='fixture:rejected'",
    ).bind(workspace).fetch_one(pool).await.unwrap();
    let changed = sqlx::query("update credit_event_outbox set published_at=null where event_id=$1 and workspace_id=$2 and published_at is not null")
        .bind(event_id).bind(workspace).execute(pool).await.unwrap();
    assert_eq!(changed.rows_affected(), 1);
    workspace
}

async fn archive(pool: &sqlx::PgPool, features: &[&str]) -> Vec<u8> {
    let repository = PgSelectiveBackupRepository::new(pool.clone());
    let catalog = repository.catalog().await.unwrap();
    let runtime = catalog
        .iter()
        .find(|category| category.feature_id == "system.system-runtime")
        .unwrap();
    for name in [
        "host_event_bus_counter",
        "host_event_bus_pending",
        "credit_event_handoffs",
    ] {
        assert!(runtime.data_tables.iter().any(|table| table == name));
    }
    let source = repository
        .source(
            features
                .iter()
                .map(|feature| SelectiveBackupSelection {
                    feature_id: (*feature).into(),
                    structure: true,
                    data: true,
                })
                .collect(),
        )
        .await
        .unwrap();
    let (writer, mut reader) = tokio::io::duplex(64 * 1024);
    let write = tokio::spawn(async move { source.write_to(Box::pin(writer)).await });
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    write.await.unwrap().unwrap();
    bytes
}

#[tokio::test]
async fn explicit_legacy_handoff_preserves_rejection_and_survives_backup_restore() {
    let original = pool().await;
    rejected_credit(&original).await;
    let bus = PgEventBus::new(original.clone());
    assert!(handoff_legacy_credit_events(None, 50).await.is_err());
    let unpublished: i64 =
        sqlx::query_scalar("select count(*) from credit_event_outbox where published_at is null")
            .fetch_one(&original)
            .await
            .unwrap();
    assert_eq!(unpublished, 1);
    assert_eq!(
        handoff_legacy_credit_events(Some(&bus), 50).await.unwrap(),
        1
    );
    let generic = json!({"nul\0key":["before\0after","literal\\u0000",null],
        "nul\\u0000key":"distinct", "array":[i64::MIN,u64::MAX,1.25,1e100]});
    bus.publish("generic\0events", generic.clone())
        .await
        .unwrap();
    let entries = bus.list_ephemeral_entries().await.unwrap();
    let credit = entries
        .iter()
        .find(|entry| entry.group_code.as_deref() == Some("credit.events"))
        .unwrap();
    let expected = bus
        .reveal_ephemeral_entry(&credit.entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap()
        .value
        .unwrap();
    assert_eq!(expected["event_type"], "CreditCommandRejected");
    assert_eq!(expected["account_id"], json!(null));
    assert_eq!(
        expected["payload"],
        json!({"actor_plugin_id":"fixture.plugin","command":"grant","reason":"permission_denied","idempotency_key":"fixture:rejected"})
    );
    let snapshots: Vec<(i64, String)> =
        sqlx::query_as("select sequence,payload from host_event_bus_pending order by sequence")
            .fetch_all(&original)
            .await
            .unwrap();
    let bytes = archive(&original, &["system.members", "system.system-runtime"]).await;
    let target = pool().await;
    let repository = PgSelectiveBackupRepository::new(target.clone());
    let preview = repository
        .restore(
            Box::pin(std::io::Cursor::new(bytes.clone())),
            "fixture-key",
            "fixture-key",
            false,
        )
        .await
        .unwrap();
    assert!(preview.failures.is_empty());
    let restored_snapshots: Vec<(i64, String)> =
        sqlx::query_as("select sequence,payload from host_event_bus_pending order by sequence")
            .fetch_all(&target)
            .await
            .unwrap();
    assert_eq!(
        restored_snapshots, snapshots,
        "serialized snapshots must restore verbatim"
    );
    let restored = PgEventBus::new(target.clone());
    assert_eq!(
        restored.poll("credit.events").await.unwrap(),
        Some(expected)
    );
    assert_eq!(
        restored.poll("generic\0events").await.unwrap(),
        Some(generic)
    );
    assert_eq!(
        handoff_legacy_credit_events(Some(&restored), 50)
            .await
            .unwrap(),
        0
    );
    assert!(restored.list_ephemeral_entries().await.unwrap().is_empty());
    restored.publish("new", json!("later")).await.unwrap();
    assert_eq!(
        restored.list_ephemeral_entries().await.unwrap()[0].entry_ref,
        "3"
    );

    // A restore must not replace a different target event with the same sequence.
    let conflict_pool = pool().await;
    let conflict = PgEventBus::new(conflict_pool.clone());
    conflict
        .publish("occupied", json!({"must":"survive"}))
        .await
        .unwrap();
    let repository = PgSelectiveBackupRepository::new(conflict_pool);
    assert!(
        repository
            .restore(
                Box::pin(std::io::Cursor::new(bytes)),
                "fixture-key",
                "fixture-key",
                false
            )
            .await
            .is_err()
    );
    assert_eq!(
        conflict.poll("occupied").await.unwrap(),
        Some(json!({"must":"survive"}))
    );
}

#[tokio::test]
async fn backup_selection_keeps_business_sources_separate_from_durable_runtime_events() {
    let original = pool().await;
    rejected_credit(&original).await;
    let bus = PgEventBus::new(original.clone());
    assert_eq!(
        handoff_legacy_credit_events(Some(&bus), 50).await.unwrap(),
        1
    );
    bus.publish(
        "unrelated-runtime-topic",
        json!({"must":"not enter members-only backup"}),
    )
    .await
    .unwrap();
    let repository = PgSelectiveBackupRepository::new(original.clone());
    let members = archive(&original, &["system.members"]).await;
    let preview = repository
        .preflight(
            Box::pin(std::io::Cursor::new(members)),
            "fixture-key",
            "fixture-key",
        )
        .await
        .unwrap();
    assert!(preview.failures.is_empty());
    assert!(
        preview
            .selected_tables
            .iter()
            .any(|name| name == "credit_event_outbox")
    );
    for table in [
        "host_event_bus_counter",
        "host_event_bus_pending",
        "credit_event_handoffs",
    ] {
        assert!(!preview.selected_tables.iter().any(|name| name == table));
    }
    let runtime = archive(&original, &["system.system-runtime"]).await;
    let preview = repository
        .preflight(
            Box::pin(std::io::Cursor::new(runtime)),
            "fixture-key",
            "fixture-key",
        )
        .await
        .unwrap();
    assert!(preview.failures.is_empty());
    assert!(
        !preview
            .selected_tables
            .iter()
            .any(|name| name == "credit_event_outbox")
    );
    for table in [
        "host_event_bus_counter",
        "host_event_bus_pending",
        "credit_event_handoffs",
    ] {
        assert!(preview.selected_tables.iter().any(|name| name == table));
    }
}

#[tokio::test]
async fn source_independent_snapshot_and_receipt_restore_after_workspace_cascade_and_poll() {
    let original = pool().await;
    let workspace = rejected_credit(&original).await;
    let bus = PgEventBus::new(original.clone());
    assert_eq!(
        handoff_legacy_credit_events(Some(&bus), 50).await.unwrap(),
        1
    );
    let entry = bus.list_ephemeral_entries().await.unwrap().remove(0);
    let expected = bus
        .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap()
        .value
        .unwrap();
    let event_id = uuid::Uuid::parse_str(expected["event_id"].as_str().unwrap()).unwrap();
    sqlx::query("delete from workspaces where id=$1")
        .bind(workspace)
        .execute(&original)
        .await
        .unwrap();
    let sources: i64 = sqlx::query_scalar("select count(*) from credit_event_outbox")
        .fetch_one(&original)
        .await
        .unwrap();
    assert_eq!(sources, 0);
    let snapshots = archive(&original, &["system.system-runtime"]).await;
    let target = pool().await;
    let repository = PgSelectiveBackupRepository::new(target.clone());
    let preview = repository
        .restore(
            Box::pin(std::io::Cursor::new(snapshots)),
            "fixture-key",
            "fixture-key",
            false,
        )
        .await
        .unwrap();
    assert!(preview.failures.is_empty());
    assert!(
        !preview
            .selected_tables
            .iter()
            .any(|table| table == "credit_event_outbox")
    );
    let restored = PgEventBus::new(target.clone());
    assert_eq!(
        restored
            .import_legacy_credit_events(&[event_id])
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        restored
            .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .unwrap()
            .value,
        Some(expected.clone())
    );
    assert_eq!(
        restored.poll("credit.events").await.unwrap(),
        Some(expected)
    );
    // The tombstone alone, with neither source nor pending body, is a legitimate backup.
    let receipt_only = archive(&target, &["system.system-runtime"]).await;
    let third = pool().await;
    PgSelectiveBackupRepository::new(third.clone())
        .restore(
            Box::pin(std::io::Cursor::new(receipt_only)),
            "fixture-key",
            "fixture-key",
            false,
        )
        .await
        .unwrap();
    let receipts_only = PgEventBus::new(third.clone());
    assert_eq!(
        receipts_only
            .import_legacy_credit_events(&[event_id, event_id])
            .await
            .unwrap(),
        0
    );
    assert!(receipts_only.poll("credit.events").await.unwrap().is_none());
    receipts_only
        .publish("later", json!("after restore"))
        .await
        .unwrap();
    assert_eq!(
        receipts_only.list_ephemeral_entries().await.unwrap()[0].entry_ref,
        "2"
    );
}

mod credit_notification_delivery;
