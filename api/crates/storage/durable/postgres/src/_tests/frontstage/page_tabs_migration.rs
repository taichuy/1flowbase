use super::*;

#[test]
fn page_tabs_up_migration_preflight_precedes_schema_changes_and_uses_transaction() {
    let migrator = page_tabs_migrator();
    let migration = migrator
        .migrations
        .iter()
        .find(|migration| migration.version == PAGE_TABS_MIGRATION_VERSION)
        .unwrap();
    assert!(
        !migration.no_tx,
        "SQLx must execute this migration transactionally"
    );

    let sql = include_str!("../../../migrations/20260710130000_add_frontstage_page_tabs.up.sql");
    let preflight = sql
        .find("frontstage page tabs preflight rejected legacy data")
        .expect("preflight exception must exist");
    let first_schema_change = sql
        .find("alter table frontstage_pages")
        .expect("page-tabs migration must change the legacy schema");
    let tab_table_creation = sql
        .find("create table frontstage_page_tabs")
        .expect("page-tabs migration must create the tab table");
    assert!(preflight < first_schema_change);
    assert!(preflight < tab_table_creation);
}

#[tokio::test]
async fn legacy_frontstage_page_schema_primary_key_rejects_duplicate_rows() {
    let pool = isolated_database().await.connect().await.unwrap();
    let migrator = page_tabs_migrator();
    migrator.run(&pool).await.unwrap();
    migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let (workspace_id, page_id, _, _, _) = insert_legacy_frontstage_page(&pool).await;

    let duplicate_error = sqlx::query(
        "insert into frontstage_page_schemas (id, scope_id, page_id, workspace_id, root_uid, schema_payload, root_payload) values ($1, $2, $3, $2, 'duplicate.root', '{}'::jsonb, '{}'::jsonb)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(page_id)
    .execute(&pool)
    .await
    .unwrap_err();
    let database_error = duplicate_error.as_database_error().unwrap();
    assert_eq!(database_error.code().as_deref(), Some("23505"));
    assert_eq!(
        database_error.constraint(),
        Some("frontstage_page_schemas_pkey")
    );
}

#[tokio::test]
async fn page_tabs_up_migration_rejects_page_without_schema_and_rolls_back() {
    let pool = isolated_database().await.connect().await.unwrap();
    let migrator = page_tabs_migrator();
    migrator.run(&pool).await.unwrap();
    migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let (workspace_id, page_id, block_code_id, _, _) = insert_legacy_frontstage_page(&pool).await;
    sqlx::query("delete from frontstage_page_schemas where page_id = $1")
        .bind(page_id)
        .execute(&pool)
        .await
        .unwrap();

    let migration_error = migrator.run(&pool).await.unwrap_err();
    let migration_message = migration_error.to_string();
    assert!(
        migration_message.contains("frontstage page tabs preflight rejected legacy data"),
        "unexpected migration error: {migration_message}"
    );
    assert!(migration_message.contains("missing schema rows 1"));
    assert!(migration_message.contains("duplicate schema rows 0"));
    assert!(migration_message.contains("root mismatches 0"));

    assert_page_tabs_up_migration_rolled_back(
        &pool,
        workspace_id,
        page_id,
        block_code_id,
        "page.root",
        None,
    )
    .await;
}

#[tokio::test]
async fn page_tabs_up_migration_rejects_schema_root_mismatch_and_rolls_back() {
    let pool = isolated_database().await.connect().await.unwrap();
    let migrator = page_tabs_migrator();
    migrator.run(&pool).await.unwrap();
    migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let (workspace_id, page_id, block_code_id, schema_payload, root_payload) =
        insert_legacy_frontstage_page(&pool).await;
    sqlx::query("update frontstage_page_schemas set root_uid = 'other.root' where page_id = $1")
        .bind(page_id)
        .execute(&pool)
        .await
        .unwrap();

    let migration_error = migrator.run(&pool).await.unwrap_err();
    let migration_message = migration_error.to_string();
    assert!(
        migration_message.contains("frontstage page tabs preflight rejected legacy data"),
        "unexpected migration error: {migration_message}"
    );
    assert!(migration_message.contains("missing schema rows 0"));
    assert!(migration_message.contains("duplicate schema rows 0"));
    assert!(migration_message.contains("root mismatches 1"));

    assert_page_tabs_up_migration_rolled_back(
        &pool,
        workspace_id,
        page_id,
        block_code_id,
        "page.root",
        Some((&schema_payload, &root_payload, "other.root")),
    )
    .await;
}

#[tokio::test]
async fn page_tabs_migration_round_trip_preserves_legacy_document_and_block_code() {
    let pool = isolated_database().await.connect().await.unwrap();
    let migrator = page_tabs_migrator();
    migrator.run(&pool).await.unwrap();
    migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let (workspace_id, page_id, block_code_id, schema_payload, root_payload) =
        insert_legacy_frontstage_page(&pool).await;

    migrator.run(&pool).await.unwrap();

    let migrated_document: (Value, Value, String) = sqlx::query_as(
        "select schemas.schema_payload, schemas.root_payload, tabs.document_root_uid from frontstage_page_schemas schemas join frontstage_page_tabs tabs on tabs.workspace_id = schemas.workspace_id and tabs.id = schemas.tab_id where tabs.workspace_id = $1 and tabs.page_id = $2",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(migrated_document.0, schema_payload);
    assert_eq!(migrated_document.1, root_payload);
    assert_eq!(migrated_document.2, "page.root");
    let migrated_code: String =
        sqlx::query_scalar("select code from frontstage_block_codes where id = $1")
            .bind(block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(migrated_code, "export default 42;");

    migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap();

    let restored_page_root: String =
        sqlx::query_scalar("select schema_root_uid from frontstage_pages where id = $1")
            .bind(page_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(restored_page_root, "page.root");
    let restored_document: (Value, Value, String) = sqlx::query_as(
        "select schema_payload, root_payload, root_uid from frontstage_page_schemas where workspace_id = $1 and page_id = $2",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(restored_document.0, schema_payload);
    assert_eq!(restored_document.1, root_payload);
    assert_eq!(restored_document.2, "page.root");
    let restored_code: String =
        sqlx::query_scalar("select code from frontstage_block_codes where id = $1")
            .bind(block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(restored_code, "export default 42;");
    let restored_shape: (bool, bool, bool, bool, bool) = sqlx::query_as(
        r#"
        select
          to_regclass(current_schema() || '.frontstage_page_tabs') is null,
          exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'schema_root_uid'),
          not exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'placement'),
          exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_page_schemas' and column_name = 'page_id'),
          not exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_page_schemas' and column_name = 'tab_id')
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(restored_shape, (true, true, true, true, true));
}

#[tokio::test]
async fn page_tabs_down_migration_rejects_multi_tab_page_without_changing_new_schema() {
    let pool = isolated_database().await.connect().await.unwrap();
    let migrator = page_tabs_migrator();
    migrator.run(&pool).await.unwrap();
    migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let (workspace_id, page_id, block_code_id, schema_payload, root_payload) =
        insert_legacy_frontstage_page(&pool).await;
    migrator.run(&pool).await.unwrap();

    let second_tab_id = Uuid::now_v7();
    let second_schema_id = Uuid::now_v7();
    let second_schema_payload = json!({"version": 1, "blocks": [{"uid": "second.block"}]});
    let second_root_payload = json!({"uid": "second.root", "children": ["second.block"]});
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Second', 'b', false, 'second.root')",
    )
    .bind(second_tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_schemas (id, scope_id, tab_id, workspace_id, root_uid, schema_payload, root_payload) values ($1, $2, $3, $2, 'second.root', $4, $5)",
    )
    .bind(second_schema_id)
    .bind(workspace_id)
    .bind(second_tab_id)
    .bind(&second_schema_payload)
    .bind(&second_root_payload)
    .execute(&pool)
    .await
    .unwrap();

    let error = migrator
        .undo(&pool, PAGE_TABS_MIGRATION_VERSION - 1)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("frontstage rollback cannot downgrade: each page must have exactly one tab"),
        "unexpected rollback error: {error}"
    );

    let placement: String =
        sqlx::query_scalar("select placement from frontstage_pages where id = $1")
            .bind(page_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(placement, "sidebar");
    let legacy_schema_root_column_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'schema_root_uid')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!legacy_schema_root_column_exists);
    let page_tabs_table_exists: bool = sqlx::query_scalar(
        "select to_regclass(current_schema() || '.frontstage_page_tabs') is not null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(page_tabs_table_exists);
    let tab_count: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_page_tabs where workspace_id = $1 and page_id = $2",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tab_count, 2);
    let documents: Vec<(Value, Value)> = sqlx::query_as(
        "select schema_payload, root_payload from frontstage_page_schemas where workspace_id = $1 order by root_uid",
    )
    .bind(workspace_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(documents.contains(&(schema_payload, root_payload)));
    assert!(documents.contains(&(second_schema_payload, second_root_payload)));
    let preserved_tabs: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "select id, document_root_uid, is_default from frontstage_page_tabs where workspace_id = $1 and page_id = $2 order by rank",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(preserved_tabs.len(), 2);
    assert!(preserved_tabs.contains(&(second_tab_id, "second.root".into(), false)));
    assert_eq!(preserved_tabs.iter().filter(|tab| tab.2).count(), 1);
    let preserved_code: String =
        sqlx::query_scalar("select code from frontstage_block_codes where id = $1")
            .bind(block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(preserved_code, "export default 42;");
    let migration_still_applied: bool = sqlx::query_scalar(
        "select exists(select 1 from _sqlx_migrations where version = $1 and success)",
    )
    .bind(PAGE_TABS_MIGRATION_VERSION)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(migration_still_applied);
}
