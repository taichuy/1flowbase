use std::borrow::Cow;

use serde_json::{json, Value};
use sqlx::{migrate::Migrator, PgPool};
use storage_postgres::{connect, run_migrations};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

#[tokio::test]
async fn page_creation_keeps_one_default_tab_and_last_tab_is_guarded() {
    use control_plane::ports::{
        CreateFrontstagePageInput, CreateFrontstagePageTabInput, FrontstagePageRepository,
    };
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let actor_user_id = Uuid::now_v7();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'x', 'Issue 1232', 'Issue 1232', 'active')",
    )
    .bind(actor_user_id)
    .bind(format!("issue1232-{actor_user_id}"))
    .bind(format!("issue1232-{actor_user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, '00000000-0000-0000-0000-000000000001', $2, $3, $3)",
    )
    .bind(workspace_id)
    .bind(format!("Issue 1232 {workspace_id}"))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    let store = storage_postgres::PgControlPlaneStore::new(pool.clone());
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let creation = store
        .create_frontstage_page(&CreateFrontstagePageInput {
            id: page_id,
            workspace_id,
            actor_user_id,
            parent_id: None,
            kind: domain::FrontstagePageKind::Page,
            title: Some("Page".into()),
            icon: None,
            tooltip: None,
            placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
            rank: "a".into(),
            default_tab: Some(CreateFrontstagePageTabInput {
                id: tab_id,
                workspace_id,
                actor_user_id,
                page_id,
                title: Some("Default".into()),
                rank: "a".into(),
                is_default: true,
                document_root_uid: format!("frontstage.tab.{tab_id}.root"),
            }),
        })
        .await
        .unwrap();
    assert_eq!(creation.default_tab.unwrap().id, tab_id);
    let tabs = store
        .list_frontstage_page_tabs(workspace_id, page_id)
        .await
        .unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs.iter().filter(|tab| tab.is_default).count(), 1);
    let error = store
        .delete_frontstage_page_tab(workspace_id, page_id, tab_id, actor_user_id)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("frontstage_page_requires_tab"));
}

const PAGE_TABS_MIGRATION_VERSION: i64 = 20260710130000;
const FRONTSTAGE_WORKSPACE_INTEGRITY_MIGRATION_VERSION: i64 = 20260710193500;
const FRONTSTAGE_PAGE_OWNER_KIND_MIGRATION_VERSION: i64 = 20260710210000;
const FRONTSTAGE_PLACEMENT_INTEGRITY_MIGRATION_VERSION: i64 = 20260710223000;

fn page_tabs_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version <= PAGE_TABS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn before_frontstage_workspace_integrity_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FRONTSTAGE_WORKSPACE_INTEGRITY_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn before_frontstage_page_owner_kind_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FRONTSTAGE_PAGE_OWNER_KIND_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn before_frontstage_placement_integrity_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FRONTSTAGE_PLACEMENT_INTEGRITY_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn insert_frontstage_group_and_page(
    pool: &PgPool,
    workspace_id: Uuid,
    group_placement: &str,
    page_placement: &str,
) -> (Uuid, Uuid) {
    let group_id = Uuid::now_v7();
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'group', 'Group', $3, 'a')",
    )
    .bind(group_id)
    .bind(workspace_id)
    .bind(group_placement)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'page', 'Page', $3, 'b')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .bind(page_placement)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    (group_id, page_id)
}

#[tokio::test]
async fn placement_integrity_migration_rejects_dirty_history_before_installing_trigger() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_frontstage_placement_integrity_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let (group_id, child_id) =
        insert_frontstage_group_and_page(&pool, workspace_id, "sidebar", "topbar").await;
    sqlx::query("update frontstage_pages set parent_id = $1 where id = $2")
        .bind(group_id)
        .bind(child_id)
        .execute(&pool)
        .await
        .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("frontstage placement integrity migration rejected dirty data"),
        "unexpected migration error: {error}"
    );
    let trigger_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from pg_trigger where tgname = 'frontstage_pages_placement_integrity_trigger' and tgrelid = 'frontstage_pages'::regclass and not tgisinternal)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!trigger_exists);
}

#[tokio::test]
async fn placement_integrity_trigger_rejects_direct_sql_and_allows_cascade_delete() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let (group_id, child_id) =
        insert_frontstage_group_and_page(&pool, workspace_id, "sidebar", "sidebar").await;
    sqlx::query("update frontstage_pages set parent_id = $1 where id = $2")
        .bind(group_id)
        .bind(child_id)
        .execute(&pool)
        .await
        .unwrap();

    let child_update_error =
        sqlx::query("update frontstage_pages set placement = 'topbar' where id = $1")
            .bind(child_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        child_update_error
            .as_database_error()
            .and_then(|error| error.constraint()),
        Some("frontstage_pages_parent_child_placement")
    );

    let parent_update_error =
        sqlx::query("update frontstage_pages set placement = 'topbar' where id = $1")
            .bind(group_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        parent_update_error
            .as_database_error()
            .and_then(|error| error.constraint()),
        Some("frontstage_pages_parent_child_placement")
    );

    sqlx::query("delete from frontstage_pages where workspace_id = $1 and id = $2")
        .bind(workspace_id)
        .bind(group_id)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_pages where workspace_id = $1 and id in ($2, $3)",
    )
    .bind(workspace_id)
    .bind(group_id)
    .bind(child_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn placement_integrity_serializes_parent_and_child_updates() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'group', 'Group', 'sidebar', 'a')",
    )
        .bind(group_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut parent_tx = pool.begin().await.unwrap();
    sqlx::query("update frontstage_pages set placement = 'topbar' where id = $1")
        .bind(group_id)
        .execute(&mut *parent_tx)
        .await
        .unwrap();

    let child_pool = pool.clone();
    let child_id = Uuid::now_v7();
    let child_insert = tokio::spawn(async move {
        sqlx::query(
            "insert into frontstage_pages (id, workspace_id, parent_id, kind, title, placement, rank) values ($1, $2, $3, 'page', 'Child', 'sidebar', 'a')",
        )
            .bind(child_id)
            .bind(workspace_id)
            .bind(group_id)
            .execute(&child_pool)
            .await
            .unwrap_err()
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!child_insert.is_finished());
    parent_tx.commit().await.unwrap();

    let error = child_insert.await.unwrap();
    assert_eq!(
        error
            .as_database_error()
            .and_then(|database_error| database_error.constraint()),
        Some("frontstage_pages_parent_child_placement")
    );
}

async fn insert_frontstage_test_workspaces(pool: &PgPool) -> (Uuid, Uuid) {
    let tenant_id = Uuid::now_v7();
    let first_workspace_id = Uuid::now_v7();
    let second_workspace_id = Uuid::now_v7();

    sqlx::query("insert into tenants (id, code, name) values ($1, $2, 'Frontstage Integrity')")
        .bind(tenant_id)
        .bind(format!("frontstage-integrity-{tenant_id}"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, $3, 'First Workspace'), ($2, $3, 'Second Workspace')",
    )
    .bind(first_workspace_id)
    .bind(second_workspace_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();

    (first_workspace_id, second_workspace_id)
}

async fn insert_frontstage_group(
    pool: &PgPool,
    id: Uuid,
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, parent_id, kind, title, rank) values ($1, $2, $3, 'group', $4, 'a')",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(parent_id)
    .bind(title)
    .execute(pool)
    .await
    .map(|_| ())
}

async fn insert_frontstage_page_with_owner_rows(
    pool: &PgPool,
    workspace_id: Uuid,
) -> (Uuid, Uuid, Uuid, Uuid) {
    let page_id = Uuid::now_v7();
    let default_tab_id = Uuid::now_v7();
    let secondary_tab_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    let mut transaction = pool.begin().await.unwrap();

    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, rank) values ($1, $2, 'page', 'Owner Page', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4), ($5, $2, $3, 'Secondary', 'b', false, $6)",
    )
    .bind(default_tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{default_tab_id}.root"))
    .bind(secondary_tab_id)
    .bind(format!("frontstage.tab.{secondary_tab_id}.root"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'code.main', 'export default 42;')",
    )
    .bind(block_code_id)
    .bind(workspace_id)
    .bind(page_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    (page_id, default_tab_id, secondary_tab_id, block_code_id)
}

async fn commit_error_contains(
    transaction: sqlx::Transaction<'_, sqlx::Postgres>,
    expected_message: &str,
) {
    let error = transaction.commit().await.unwrap_err();
    assert!(
        error.to_string().contains(expected_message),
        "unexpected commit error: {error}"
    );
}

#[tokio::test]
async fn full_migrations_reject_group_owned_tabs_and_block_codes_at_commit() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    insert_frontstage_group(&pool, group_id, workspace_id, None, "Owner Group")
        .await
        .unwrap();
    let (page_id, _, secondary_tab_id, block_code_id) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;

    let mut insert_tab = pool.begin().await.unwrap();
    let inserted_tab_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Invalid', 'z', false, $4)",
    )
    .bind(inserted_tab_id)
    .bind(workspace_id)
    .bind(group_id)
    .bind(format!("frontstage.tab.{inserted_tab_id}.root"))
    .execute(&mut *insert_tab)
    .await
    .unwrap();
    commit_error_contains(insert_tab, "frontstage_page_tab_owner_must_be_page").await;

    let mut update_tab = pool.begin().await.unwrap();
    sqlx::query("update frontstage_page_tabs set page_id = $1 where id = $2")
        .bind(group_id)
        .bind(secondary_tab_id)
        .execute(&mut *update_tab)
        .await
        .unwrap();
    commit_error_contains(update_tab, "frontstage_page_tab_owner_must_be_page").await;

    let mut insert_code = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'invalid.insert', 'export default 1;')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(group_id)
    .execute(&mut *insert_code)
    .await
    .unwrap();
    commit_error_contains(insert_code, "frontstage_block_code_owner_must_be_page").await;

    let mut update_code = pool.begin().await.unwrap();
    sqlx::query("update frontstage_block_codes set page_id = $1 where id = $2")
        .bind(group_id)
        .bind(block_code_id)
        .execute(&mut *update_code)
        .await
        .unwrap();
    commit_error_contains(update_code, "frontstage_block_code_owner_must_be_page").await;

    let preserved_owner_rows: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_page_tabs where page_id = $1), (select count(*) from frontstage_block_codes where page_id = $1)",
    )
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preserved_owner_rows, (2, 1));
}

#[tokio::test]
async fn full_migrations_reject_page_to_group_with_owner_rows_and_allow_cascade_delete() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    insert_frontstage_group(&pool, group_id, workspace_id, None, "Empty Group")
        .await
        .unwrap();
    let group_kind: String = sqlx::query_scalar("select kind from frontstage_pages where id = $1")
        .bind(group_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(group_kind, "group");

    let (guarded_page_id, _, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    let mut kind_update = pool.begin().await.unwrap();
    sqlx::query("update frontstage_pages set kind = 'group' where id = $1")
        .bind(guarded_page_id)
        .execute(&mut *kind_update)
        .await
        .unwrap();
    commit_error_contains(kind_update, "frontstage_page_owner_rows_require_page_kind").await;

    let (deleted_page_id, _, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    sqlx::query("delete from frontstage_pages where id = $1")
        .bind(deleted_page_id)
        .execute(&pool)
        .await
        .unwrap();
    let deleted_owner_rows: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_page_tabs where page_id = $1), (select count(*) from frontstage_block_codes where page_id = $1)",
    )
    .bind(deleted_page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deleted_owner_rows, (0, 0));
}

#[tokio::test]
async fn full_migrations_defer_page_owner_kind_checks_until_transaction_end() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let trigger_timing: Vec<(String, bool, bool)> = sqlx::query_as(
        r#"
        select trigger_definition.tgname,
               trigger_definition.tgdeferrable,
               trigger_definition.tginitdeferred
        from pg_trigger trigger_definition
        join pg_class table_definition
          on table_definition.oid = trigger_definition.tgrelid
        join pg_namespace table_schema
          on table_schema.oid = table_definition.relnamespace
        where table_schema.nspname = current_schema()
          and trigger_definition.tgname in (
          'frontstage_page_tabs_require_page_owner',
          'frontstage_block_codes_require_page_owner',
          'frontstage_pages_owner_rows_require_page_kind'
        )
        order by tgname
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(trigger_timing.len(), 3);
    assert!(trigger_timing
        .iter()
        .all(|(_, deferrable, initially_deferred)| *deferrable && *initially_deferred));

    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let immediate_group_id = Uuid::now_v7();
    insert_frontstage_group(
        &pool,
        immediate_group_id,
        workspace_id,
        None,
        "Immediate Group",
    )
    .await
    .unwrap();
    let immediate_tab_id = Uuid::now_v7();
    let mut immediate_check = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Immediate', 'a', false, $4)",
    )
    .bind(immediate_tab_id)
    .bind(workspace_id)
    .bind(immediate_group_id)
    .bind(format!("frontstage.tab.{immediate_tab_id}.root"))
    .execute(&mut *immediate_check)
    .await
    .unwrap();
    let immediate_error =
        sqlx::query("set constraints frontstage_page_tabs_require_page_owner immediate")
            .execute(&mut *immediate_check)
            .await
            .unwrap_err();
    assert!(
        immediate_error
            .to_string()
            .contains("frontstage_page_tab_owner_must_be_page"),
        "unexpected immediate constraint error: {immediate_error}"
    );
    immediate_check.rollback().await.unwrap();

    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, rank) values ($1, $2, 'group', 'Ordered Page', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'ordered.code', 'export default 1;')",
    )
    .bind(block_code_id)
    .bind(workspace_id)
    .bind(page_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query("update frontstage_pages set kind = 'page' where id = $1")
        .bind(page_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let final_state: (String, i64, i64) = sqlx::query_as(
        "select kind, (select count(*) from frontstage_page_tabs where page_id = $1), (select count(*) from frontstage_block_codes where page_id = $1) from frontstage_pages where id = $1",
    )
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_state, ("page".into(), 1, 1));

    let (converted_page_id, _, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    let mut conversion = pool.begin().await.unwrap();
    sqlx::query("delete from frontstage_block_codes where page_id = $1")
        .bind(converted_page_id)
        .execute(&mut *conversion)
        .await
        .unwrap();
    sqlx::query("delete from frontstage_page_tabs where page_id = $1")
        .bind(converted_page_id)
        .execute(&mut *conversion)
        .await
        .unwrap();
    sqlx::query("update frontstage_pages set kind = 'group' where id = $1")
        .bind(converted_page_id)
        .execute(&mut *conversion)
        .await
        .unwrap();
    conversion.commit().await.unwrap();

    let converted_kind: String =
        sqlx::query_scalar("select kind from frontstage_pages where id = $1")
            .bind(converted_page_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(converted_kind, "group");
}

#[tokio::test]
async fn full_migrations_validate_old_and_new_tab_owners_after_reparent() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, second_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let (source_page_id, source_default_tab_id, source_secondary_tab_id, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    let (target_page_id, target_default_tab_id, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;

    let trigger_function_definition: String = sqlx::query_scalar(
        "select pg_get_functiondef(trigger_definition.tgfoid) from pg_trigger trigger_definition join pg_class table_definition on table_definition.oid = trigger_definition.tgrelid join pg_namespace table_schema on table_schema.oid = table_definition.relnamespace where table_schema.nspname = current_schema() and table_definition.relname = 'frontstage_page_tabs' and trigger_definition.tgname = 'frontstage_page_tabs_preserve_invariant'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let normalized_trigger_function = trigger_function_definition.to_ascii_lowercase();
    assert!(normalized_trigger_function.contains("tg_op = 'update'"));
    assert!(normalized_trigger_function.contains("is distinct from"));
    assert!(normalized_trigger_function.contains("old.workspace_id"));
    assert!(normalized_trigger_function.contains("old.page_id"));
    assert!(normalized_trigger_function.contains("new.workspace_id"));
    assert!(normalized_trigger_function.contains("new.page_id"));
    assert!(normalized_trigger_function.contains(
        "else\n    perform enforce_frontstage_page_tab_invariant(new.workspace_id, new.page_id);"
    ));

    sqlx::query("delete from frontstage_page_tabs where id = $1")
        .bind(source_secondary_tab_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut invalid_reparent = pool.begin().await.unwrap();
    sqlx::query("update frontstage_page_tabs set page_id = $1, is_default = false where id = $2")
        .bind(target_page_id)
        .bind(source_default_tab_id)
        .execute(&mut *invalid_reparent)
        .await
        .unwrap();
    commit_error_contains(
        invalid_reparent,
        "frontstage page must keep at least one tab",
    )
    .await;

    let replacement_tab_id = Uuid::now_v7();
    let mut valid_reparent = pool.begin().await.unwrap();
    sqlx::query("update frontstage_page_tabs set is_default = false where id = $1")
        .bind(source_default_tab_id)
        .execute(&mut *valid_reparent)
        .await
        .unwrap();
    sqlx::query("update frontstage_page_tabs set is_default = false where id = $1")
        .bind(target_default_tab_id)
        .execute(&mut *valid_reparent)
        .await
        .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Replacement', 'c', true, $4)",
    )
    .bind(replacement_tab_id)
    .bind(workspace_id)
    .bind(source_page_id)
    .bind(format!("frontstage.tab.{replacement_tab_id}.root"))
    .execute(&mut *valid_reparent)
    .await
    .unwrap();
    sqlx::query("update frontstage_page_tabs set page_id = $1, is_default = true where id = $2")
        .bind(target_page_id)
        .bind(source_default_tab_id)
        .execute(&mut *valid_reparent)
        .await
        .unwrap();
    valid_reparent.commit().await.unwrap();

    let owner_state: Vec<(Uuid, i64, i64)> = sqlx::query_as(
        "select page_id, count(*), count(*) filter (where is_default) from frontstage_page_tabs where workspace_id = $1 and page_id in ($2, $3) group by page_id order by page_id",
    )
    .bind(workspace_id)
    .bind(source_page_id)
    .bind(target_page_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(owner_state.contains(&(source_page_id, 1, 1)));
    assert!(owner_state.contains(&(target_page_id, 3, 1)));

    let cross_workspace_error =
        sqlx::query("update frontstage_page_tabs set workspace_id = $1 where id = $2")
            .bind(second_workspace_id)
            .bind(source_default_tab_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert!(
        cross_workspace_error
            .to_string()
            .contains("frontstage_page_tabs_workspace_id_page_id_fkey"),
        "unexpected cross-workspace error: {cross_workspace_error}"
    );
}

#[tokio::test]
async fn page_owner_kind_migration_preflight_rejects_dirty_data_without_changes() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_frontstage_page_owner_kind_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    insert_frontstage_group(&pool, group_id, workspace_id, None, "Dirty Group")
        .await
        .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Dirty', 'a', false, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(group_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'dirty.code', 'export default 1;')",
    )
    .bind(block_code_id)
    .bind(workspace_id)
    .bind(group_id)
    .execute(&pool)
    .await
    .unwrap();

    let migration_error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(
        migration_error
            .to_string()
            .contains("frontstage page owner kind migration rejected dirty data: tab rows 1, block code rows 1"),
        "unexpected migration error: {migration_error}"
    );

    let dirty_rows: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_page_tabs where id = $1), (select count(*) from frontstage_block_codes where id = $2)",
    )
    .bind(tab_id)
    .bind(block_code_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dirty_rows, (1, 1));
    let migration_applied: bool = sqlx::query_scalar(
        "select exists(select 1 from _sqlx_migrations where version = $1 and success)",
    )
    .bind(FRONTSTAGE_PAGE_OWNER_KIND_MIGRATION_VERSION)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!migration_applied);
    let trigger_created: bool = sqlx::query_scalar(
        "select exists(select 1 from pg_trigger trigger_definition join pg_class table_definition on table_definition.oid = trigger_definition.tgrelid join pg_namespace table_schema on table_schema.oid = table_definition.relnamespace where table_schema.nspname = current_schema() and trigger_definition.tgname = 'frontstage_page_tabs_require_page_owner')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!trigger_created);
}

#[tokio::test]
async fn full_migrations_enforce_frontstage_page_and_block_code_workspace_ownership() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let workspace_foreign_keys: Vec<(String, String)> = sqlx::query_as(
        r#"
        select conname, pg_get_constraintdef(oid)
        from pg_constraint
        where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass)
          and conname in (
            'frontstage_pages_workspace_parent_fkey',
            'frontstage_block_codes_workspace_page_fkey'
          )
        order by conname
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(workspace_foreign_keys.len(), 2);
    let legacy_foreign_keys: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass) and conname in ('frontstage_pages_parent_id_fkey', 'frontstage_block_codes_page_id_fkey')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_foreign_keys, 0);
    assert!(workspace_foreign_keys.iter().any(|(name, definition)| {
        name == "frontstage_pages_workspace_parent_fkey"
            && definition.contains("FOREIGN KEY (workspace_id, parent_id)")
            && definition.contains("REFERENCES frontstage_pages(workspace_id, id)")
            && definition.contains("ON DELETE CASCADE")
    }));
    assert!(workspace_foreign_keys.iter().any(|(name, definition)| {
        name == "frontstage_block_codes_workspace_page_fkey"
            && definition.contains("FOREIGN KEY (workspace_id, page_id)")
            && definition.contains("REFERENCES frontstage_pages(workspace_id, id)")
            && definition.contains("ON DELETE CASCADE")
    }));
    let parent_is_nullable: bool = sqlx::query_scalar(
        "select is_nullable = 'YES' from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'parent_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(parent_is_nullable);
    let workspace_page_unique_indexes: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_index index_definition
        join pg_class table_definition on table_definition.oid = index_definition.indrelid
        join pg_class index_relation on index_relation.oid = index_definition.indexrelid
        where table_definition.oid = 'frontstage_pages'::regclass
          and index_definition.indisunique
          and index_relation.relname = 'frontstage_pages_workspace_id_id_uidx'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(workspace_page_unique_indexes, 1);
    let (first_workspace_id, second_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let first_parent_id = Uuid::now_v7();
    let first_child_id = Uuid::now_v7();
    let second_parent_id = Uuid::now_v7();
    let cross_workspace_child_id = Uuid::now_v7();
    let first_code_page_id = Uuid::now_v7();
    let first_code_page_tab_id = Uuid::now_v7();
    let second_code_page_id = Uuid::now_v7();
    let second_code_page_tab_id = Uuid::now_v7();

    insert_frontstage_group(
        &pool,
        first_parent_id,
        first_workspace_id,
        None,
        "First Parent",
    )
    .await
    .unwrap();

    let mut owner_pages = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, parent_id, kind, title, rank) values ($1, $2, $3, 'page', 'First Code Page', 'b'), ($4, $5, null, 'page', 'Second Code Page', 'a')",
    )
    .bind(first_code_page_id)
    .bind(first_workspace_id)
    .bind(first_parent_id)
    .bind(second_code_page_id)
    .bind(second_workspace_id)
    .execute(&mut *owner_pages)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4), ($5, $6, $7, 'Default', 'a', true, $8)",
    )
    .bind(first_code_page_tab_id)
    .bind(first_workspace_id)
    .bind(first_code_page_id)
    .bind(format!("frontstage.tab.{first_code_page_tab_id}.root"))
    .bind(second_code_page_tab_id)
    .bind(second_workspace_id)
    .bind(second_code_page_id)
    .bind(format!("frontstage.tab.{second_code_page_tab_id}.root"))
    .execute(&mut *owner_pages)
    .await
    .unwrap();
    owner_pages.commit().await.unwrap();
    insert_frontstage_group(
        &pool,
        first_child_id,
        first_workspace_id,
        Some(first_parent_id),
        "First Child",
    )
    .await
    .unwrap();
    insert_frontstage_group(
        &pool,
        second_parent_id,
        second_workspace_id,
        None,
        "Second Parent",
    )
    .await
    .unwrap();

    let cross_workspace_parent_error = insert_frontstage_group(
        &pool,
        cross_workspace_child_id,
        second_workspace_id,
        Some(first_parent_id),
        "Invalid Child",
    )
    .await
    .unwrap_err();
    assert_eq!(
        cross_workspace_parent_error
            .as_database_error()
            .and_then(|error| error.code().map(|code| code.into_owned())),
        Some("23503".into())
    );

    let first_block_code_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'same-workspace', 'export default 1;')",
    )
    .bind(first_block_code_id)
    .bind(first_workspace_id)
    .bind(first_code_page_id)
    .execute(&pool)
    .await
    .unwrap();
    let cross_workspace_block_error = sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'cross-workspace', 'export default 2;')",
    )
    .bind(Uuid::now_v7())
    .bind(second_workspace_id)
    .bind(first_code_page_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        cross_workspace_block_error
            .as_database_error()
            .and_then(|error| error.code().map(|code| code.into_owned())),
        Some("23503".into())
    );

    let second_block_code_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'same-workspace', 'export default 3;')",
    )
    .bind(second_block_code_id)
    .bind(second_workspace_id)
    .bind(second_code_page_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("delete from frontstage_pages where workspace_id = $1 and id = $2")
        .bind(first_workspace_id)
        .bind(first_parent_id)
        .execute(&pool)
        .await
        .unwrap();

    let first_workspace_page_count: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_pages where workspace_id = $1 and id in ($2, $3, $4)",
    )
    .bind(first_workspace_id)
    .bind(first_parent_id)
    .bind(first_child_id)
    .bind(first_code_page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first_workspace_page_count, 0);
    let first_block_code_exists: bool =
        sqlx::query_scalar("select exists(select 1 from frontstage_block_codes where id = $1)")
            .bind(first_block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!first_block_code_exists);
    let second_workspace_page_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from frontstage_pages where workspace_id = $1 and id = $2)",
    )
    .bind(second_workspace_id)
    .bind(second_parent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(second_workspace_page_exists);
    let second_block_code_exists: bool =
        sqlx::query_scalar("select exists(select 1 from frontstage_block_codes where id = $1)")
            .bind(second_block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(second_block_code_exists);
}

#[tokio::test]
async fn workspace_integrity_migration_rejects_dirty_history_without_schema_or_data_changes() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_frontstage_workspace_integrity_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (first_workspace_id, second_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let first_parent_id = Uuid::now_v7();
    let dirty_child_id = Uuid::now_v7();
    let dirty_block_code_id = Uuid::now_v7();

    insert_frontstage_group(
        &pool,
        first_parent_id,
        first_workspace_id,
        None,
        "Dirty Parent",
    )
    .await
    .unwrap();
    insert_frontstage_group(
        &pool,
        dirty_child_id,
        second_workspace_id,
        Some(first_parent_id),
        "Dirty Child",
    )
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'dirty-owner', 'export default 4;')",
    )
    .bind(dirty_block_code_id)
    .bind(second_workspace_id)
    .bind(first_parent_id)
    .execute(&pool)
    .await
    .unwrap();

    let migration_error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    let migration_message = migration_error.to_string();
    assert!(
        migration_message.contains("frontstage workspace integrity migration rejected dirty data"),
        "unexpected migration error: {migration_message}"
    );
    assert!(migration_message.contains("parent rows 1"));
    assert!(migration_message.contains("block code rows 1"));

    let dirty_rows: i64 = sqlx::query_scalar(
        "select (select count(*) from frontstage_pages where id = $1) + (select count(*) from frontstage_block_codes where id = $2)",
    )
    .bind(dirty_child_id)
    .bind(dirty_block_code_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dirty_rows, 2);
    let old_constraints: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass) and conname in ('frontstage_pages_parent_id_fkey', 'frontstage_block_codes_page_id_fkey')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old_constraints, 2);
    let new_constraints: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass) and conname in ('frontstage_pages_workspace_parent_fkey', 'frontstage_block_codes_workspace_page_fkey')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(new_constraints, 0);
    let migration_recorded: bool =
        sqlx::query_scalar("select exists(select 1 from _sqlx_migrations where version = $1)")
            .bind(FRONTSTAGE_WORKSPACE_INTEGRITY_MIGRATION_VERSION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!migration_recorded);
}

async fn insert_legacy_frontstage_page(pool: &PgPool) -> (Uuid, Uuid, Uuid, Value, Value) {
    let tenant_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let page_id = Uuid::now_v7();
    let page_schema_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    let schema_payload = json!({
        "version": 1,
        "blocks": [{"uid": "block.code", "codeRef": "code.main"}]
    });
    let root_payload = json!({
        "uid": "page.root",
        "children": ["block.code"]
    });

    sqlx::query("insert into tenants (id, code, name) values ($1, $2, 'Rollback Tenant')")
        .bind(tenant_id)
        .bind(format!("rollback-{tenant_id}"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, $2, 'Rollback Workspace')",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, slug, schema_root_uid, rank) values ($1, $2, 'page', 'Rollback Page', 'rollback-page', 'page.root', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_schemas (id, scope_id, page_id, workspace_id, root_uid, schema_payload, root_payload) values ($1, $2, $3, $2, 'page.root', $4, $5)",
    )
    .bind(page_schema_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(&schema_payload)
    .bind(&root_payload)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'code.main', 'export default 42;')",
    )
    .bind(block_code_id)
    .bind(workspace_id)
    .bind(page_id)
    .execute(pool)
    .await
    .unwrap();

    (
        workspace_id,
        page_id,
        block_code_id,
        schema_payload,
        root_payload,
    )
}

async fn assert_page_tabs_up_migration_rolled_back(
    pool: &PgPool,
    workspace_id: Uuid,
    page_id: Uuid,
    block_code_id: Uuid,
    expected_schema_root_uid: &str,
    expected_schema_row: Option<(&Value, &Value, &str)>,
) {
    let legacy_shape: (bool, bool, bool, bool, bool) = sqlx::query_as(
        r#"
        select
          to_regclass(current_schema() || '.frontstage_page_tabs') is null,
          not exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'placement'),
          exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'schema_root_uid'),
          exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_page_schemas' and column_name = 'page_id'),
          not exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_page_schemas' and column_name = 'tab_id')
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(legacy_shape, (true, true, true, true, true));

    let page_schema_root_uid: String =
        sqlx::query_scalar("select schema_root_uid from frontstage_pages where id = $1")
            .bind(page_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(page_schema_root_uid, expected_schema_root_uid);

    let schema_rows: Vec<(Value, Value, String)> = sqlx::query_as(
        "select schema_payload, root_payload, root_uid from frontstage_page_schemas where workspace_id = $1 and page_id = $2",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_all(pool)
    .await
    .unwrap();
    match expected_schema_row {
        Some((schema_payload, root_payload, root_uid)) => {
            assert_eq!(
                schema_rows,
                vec![(
                    schema_payload.clone(),
                    root_payload.clone(),
                    root_uid.into()
                )]
            );
        }
        None => assert!(schema_rows.is_empty()),
    }

    let block_code: String =
        sqlx::query_scalar("select code from frontstage_block_codes where id = $1")
            .bind(block_code_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(block_code, "export default 42;");

    let migration_recorded: bool =
        sqlx::query_scalar("select exists(select 1 from _sqlx_migrations where version = $1)")
            .bind(PAGE_TABS_MIGRATION_VERSION)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(!migration_recorded);
}

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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

#[tokio::test]
async fn migration_creates_frontstage_page_visibility_rules() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let table_exists: bool = sqlx::query_scalar(
        r#"
        select exists(
            select 1
            from information_schema.tables
            where table_schema = $1
              and table_name = 'frontstage_page_visibility_rules'
        )
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(table_exists);

    let visibility_check_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'frontstage_page_visibility_rules'
          and c.contype = 'c'
          and pg_get_constraintdef(c.oid) ilike '%visible%'
          and pg_get_constraintdef(c.oid) ilike '%hidden%'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(visibility_check_count, 1);

    let foreign_key_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'frontstage_page_visibility_rules'
          and c.contype = 'f'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(foreign_key_count, 3);

    let unique_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexname
        from pg_indexes
        where schemaname = $1
          and tablename = 'frontstage_page_visibility_rules'
          and indexname in (
              'frontstage_page_visibility_rules_root_uidx',
              'frontstage_page_visibility_rules_page_uidx'
          )
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(unique_indexes.len(), 2);
}
