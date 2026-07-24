use control_plane::ports::{
    CreateFileStorageInput, FileManagementRepository, UpdateFileStorageBindingInput,
};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "File Management")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
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

async fn seed_model_definition(
    store: &PgControlPlaneStore,
    actor_user_id: Uuid,
    scope_kind: &str,
    scope_id: Uuid,
    code_prefix: &str,
) -> Uuid {
    let model_definition_id = Uuid::now_v7();
    let code = format!("{code_prefix}_{}", model_definition_id.simple());
    let physical_table_name = format!("rtm_{scope_kind}_{}", model_definition_id.simple());

    sqlx::query(
        r#"
        insert into model_definitions (
            id,
            scope_kind,
            scope_id,
            code,
            title,
            physical_table_name,
            acl_namespace,
            audit_namespace,
            created_by,
            updated_by
        ) values (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $9
        )
        "#,
    )
    .bind(model_definition_id)
    .bind(scope_kind)
    .bind(scope_id)
    .bind(&code)
    .bind(code_prefix)
    .bind(&physical_table_name)
    .bind(format!("acl.{code}"))
    .bind(format!("audit.{code}"))
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    model_definition_id
}

async fn seed_file_table(
    store: &PgControlPlaneStore,
    actor_user_id: Uuid,
    scope_kind: &str,
    scope_id: Uuid,
    model_definition_id: Uuid,
    bound_storage_id: Uuid,
    code_prefix: &str,
) -> Uuid {
    let file_table_id = Uuid::now_v7();
    let code = format!("{code_prefix}_{}", file_table_id.simple());

    sqlx::query(
        r#"
        insert into file_tables (
            id,
            code,
            title,
            scope_kind,
            scope_id,
            model_definition_id,
            bound_storage_id,
            is_builtin,
            is_default,
            status,
            created_by,
            updated_by
        ) values (
            $1, $2, $3, $4, $5, $6, $7, false, false, 'active', $8, $8
        )
        "#,
    )
    .bind(file_table_id)
    .bind(&code)
    .bind(code_prefix)
    .bind(scope_kind)
    .bind(scope_id)
    .bind(model_definition_id)
    .bind(bound_storage_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    file_table_id
}

#[tokio::test]
async fn file_management_repository_creates_default_storage_and_updates_bindings() {
    let (store, workspace, actor) = seed_store().await;

    let default_storage = <PgControlPlaneStore as FileManagementRepository>::create_file_storage(
        &store,
        &CreateFileStorageInput {
            storage_id: Uuid::now_v7(),
            actor_user_id: actor.id,
            code: "local-default".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: serde_json::json!({ "root_path": "api/storage" }),
            rule_json: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    assert_eq!(default_storage.driver_type, "local");
    assert!(default_storage.is_default);

    let listed_storages =
        <PgControlPlaneStore as FileManagementRepository>::list_file_storages(&store)
            .await
            .unwrap();
    assert_eq!(listed_storages.len(), 1);

    let loaded_default =
        <PgControlPlaneStore as FileManagementRepository>::get_default_file_storage(&store)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(loaded_default.id, default_storage.id);

    let loaded_storage = <PgControlPlaneStore as FileManagementRepository>::get_file_storage(
        &store,
        default_storage.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(loaded_storage.code, "local-default");

    let empty_tables = <PgControlPlaneStore as FileManagementRepository>::list_visible_file_tables(
        &store,
        workspace.id,
    )
    .await
    .unwrap();
    assert!(empty_tables.is_empty());

    let backup_storage = <PgControlPlaneStore as FileManagementRepository>::create_file_storage(
        &store,
        &CreateFileStorageInput {
            storage_id: Uuid::now_v7(),
            actor_user_id: actor.id,
            code: "local-backup".into(),
            title: "Backup".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: false,
            config_json: serde_json::json!({ "root_path": "api/storage-backup" }),
            rule_json: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    let system_model_definition_id = seed_model_definition(
        &store,
        actor.id,
        "system",
        domain::SYSTEM_SCOPE_ID,
        "system_files",
    )
    .await;
    seed_file_table(
        &store,
        actor.id,
        "system",
        domain::SYSTEM_SCOPE_ID,
        system_model_definition_id,
        default_storage.id,
        "system_files",
    )
    .await;

    let workspace_model_definition_id = seed_model_definition(
        &store,
        actor.id,
        "workspace",
        workspace.id,
        "workspace_files",
    )
    .await;
    let workspace_file_table_id = seed_file_table(
        &store,
        actor.id,
        "workspace",
        workspace.id,
        workspace_model_definition_id,
        default_storage.id,
        "workspace_files",
    )
    .await;

    let visible_tables =
        <PgControlPlaneStore as FileManagementRepository>::list_visible_file_tables(
            &store,
            workspace.id,
        )
        .await
        .unwrap();
    assert_eq!(visible_tables.len(), 2);
    assert!(visible_tables
        .iter()
        .any(|record| record.scope_id == domain::SYSTEM_SCOPE_ID));
    assert!(visible_tables
        .iter()
        .any(|record| record.id == workspace_file_table_id));

    let updated = <PgControlPlaneStore as FileManagementRepository>::update_file_table_binding(
        &store,
        &UpdateFileStorageBindingInput {
            actor_user_id: actor.id,
            file_table_id: workspace_file_table_id,
            bound_storage_id: backup_storage.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.bound_storage_id, backup_storage.id);
}
