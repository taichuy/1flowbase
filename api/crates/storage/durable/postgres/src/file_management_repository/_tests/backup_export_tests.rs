use control_plane::{
    file_management::{CreateWorkspaceFileTableCommand, FileTableProvisioningService},
    ports::{CreateFileStorageInput, FileManagementRepository},
};
use control_plane_contracts::ports::{
    BackupObjectDatabaseReference, BackupObjectInventoryRepository,
};
use uuid::Uuid;

use crate::{run_migrations, PgControlPlaneStore};

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn fixture_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Backup Object Inventory")
        .await
        .unwrap();
    control_plane::bootstrap::upsert_permission_catalog(&store)
        .await
        .unwrap();
    control_plane::bootstrap::upsert_builtin_roles(&store, workspace.id)
        .await
        .unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();
    (store, workspace, actor)
}

#[tokio::test]
async fn inventory_reads_registered_file_tables_and_live_runtime_debug_references() {
    let (store, workspace, actor) = fixture_store().await;
    let storage = <PgControlPlaneStore as FileManagementRepository>::create_file_storage(
        &store,
        &CreateFileStorageInput {
            storage_id: Uuid::now_v7(),
            actor_user_id: actor.id,
            code: "backup_inventory_local".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: serde_json::json!({ "root_path": "/tmp/backup-object-inventory" }),
            rule_json: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    let file_table = FileTableProvisioningService::new(store.clone())
        .create_workspace_file_table(CreateWorkspaceFileTableCommand {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            code: format!("backup_files_{}", Uuid::now_v7().simple()),
            title: "Backup Files".into(),
            default_storage_id: storage.id,
        })
        .await
        .unwrap();
    let physical_table_name: String =
        sqlx::query_scalar("select physical_table_name from model_definitions where id = $1")
            .bind(file_table.model_definition_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let file_record_id = Uuid::now_v7();
    sqlx::query(&format!(
        r#"
        insert into "{physical_table_name}" (
            id, scope_id, created_by, updated_by,
            filename, size, mimetype, path, meta, storage_id
        ) values ($1, $2, $3, $3, 'empty.bin', 0, 'application/octet-stream',
                  'workspace/files/empty.bin', '{{}}'::jsonb, $4)
        "#
    ))
    .bind(file_record_id)
    .bind(workspace.id)
    .bind(actor.id)
    .bind(storage.id.to_string())
    .execute(store.pool())
    .await
    .unwrap();

    let application_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into applications (
            id, workspace_id, application_type, name, description, created_by, updated_by
        ) values ($1, $2, 'agent_flow', 'Backup fixture', '', $3, $3)
        "#,
    )
    .bind(application_id)
    .bind(workspace.id)
    .bind(actor.id)
    .execute(store.pool())
    .await
    .unwrap();
    for (path, retention_state) in [
        ("runtime/active.json", "active"),
        ("runtime/pending.json", "pending_delete"),
        ("runtime/deleted.json", "deleted"),
    ] {
        sqlx::query(
            r#"
            insert into runtime_debug_artifacts (
                id, workspace_id, application_id, artifact_kind, content_type,
                original_size_bytes, preview_size_bytes, storage_id, storage_ref,
                retention_state
            ) values ($1, $2, $3, 'node_output', 'application/json', 2, 1, $4, $5, $6)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(workspace.id)
        .bind(application_id)
        .bind(storage.id)
        .bind(path)
        .bind(retention_state)
        .execute(store.pool())
        .await
        .unwrap();
    }

    let records = store.list_backup_object_inventory().await.unwrap();
    let paths = records
        .iter()
        .map(|record| record.object_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "runtime/active.json",
            "runtime/pending.json",
            "workspace/files/empty.bin",
        ]
    );
    assert!(!paths.contains(&"runtime/deleted.json"));
    assert!(records.iter().any(|record| {
        record.size_bytes == 0
            && matches!(
                &record.reference,
                BackupObjectDatabaseReference::FileRecord {
                    file_table_id,
                    record_id
                } if *file_table_id == file_table.id && *record_id == file_record_id
            )
    }));
}
