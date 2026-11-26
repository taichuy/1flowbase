use domain::RecoveryJobId;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    current_migration_head, run_migrations,
    system_recovery::{
        bump_all_user_session_versions_at_url, upsert_system_recovery_audit_log_at_url,
        PostRestoreCatalogReadinessCounts, SystemRecoveryAuditLogRow,
    },
    PgControlPlaneStore,
};

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

#[tokio::test]
async fn post_restore_helpers_bump_sessions_verify_readiness_and_project_idempotent_audit_logs() {
    let database = postgres_test_support::PostgresTestDatabase::create(&base_database_url())
        .await
        .unwrap();
    let url = database.database_url().to_owned();
    let store = PgControlPlaneStore::connect_for_recovery(&url)
        .await
        .unwrap();
    run_migrations(store.pool()).await.unwrap();

    let existing_users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        bump_all_user_session_versions_at_url(&url).await.unwrap(),
        existing_users as u64
    );

    let actor_user_id = Uuid::now_v7();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status, session_version) values ($1, 'recovery-root', 'recovery-root@example.test', 'hash', 'Root', 'Root', 'active', 3)",
    )
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    let expected_permissions: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions")
            .fetch_one(store.pool())
            .await
            .unwrap();

    assert_eq!(
        store.migration_head().await.unwrap(),
        current_migration_head().unwrap()
    );
    assert!(store.has_postgres_restore_privileges().await.unwrap());
    assert!(store
        .managed_schema_backup_inventory()
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .post_restore_catalog_readiness_counts("node-a")
            .await
            .unwrap(),
        PostRestoreCatalogReadinessCounts {
            permission_definitions: expected_permissions,
            unhealthy_node_artifacts: 0,
            missing_active_installations: 0,
        }
    );

    assert_eq!(
        bump_all_user_session_versions_at_url(&url).await.unwrap(),
        (existing_users + 1) as u64
    );
    let bumped_version: i64 = sqlx::query_scalar("select session_version from users where id = $1")
        .bind(actor_user_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(bumped_version, 4);

    let audit_id = Uuid::now_v7();
    let recovery_job_id = RecoveryJobId::new();
    let occurred_at = OffsetDateTime::from_unix_timestamp(1_795_000_000).unwrap();
    let row = SystemRecoveryAuditLogRow {
        audit_id,
        actor_user_id,
        recovery_job_id,
        event_code: "system.recovery.succeeded",
        payload: json!({ "outcome": "succeeded" }),
        occurred_at,
    };
    assert!(upsert_system_recovery_audit_log_at_url(&url, row.clone())
        .await
        .unwrap());
    assert!(upsert_system_recovery_audit_log_at_url(&url, row.clone())
        .await
        .unwrap());

    let mut mismatched = row;
    mismatched.payload = json!({ "outcome": "tampered" });
    assert!(!upsert_system_recovery_audit_log_at_url(&url, mismatched)
        .await
        .unwrap());

    store.close().await;
}
