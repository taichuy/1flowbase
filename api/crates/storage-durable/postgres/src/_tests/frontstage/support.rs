use super::*;

pub(super) const PAGE_TABS_MIGRATION_VERSION: i64 = 20260710130000;
pub(super) const FRONTSTAGE_WORKSPACE_INTEGRITY_MIGRATION_VERSION: i64 = 20260710193500;
pub(super) const FRONTSTAGE_PAGE_OWNER_KIND_MIGRATION_VERSION: i64 = 20260710210000;
pub(super) const FRONTSTAGE_PLACEMENT_INTEGRITY_MIGRATION_VERSION: i64 = 20260710223000;
pub(super) const FRONTSTAGE_PAGE_TAB_OWNERSHIP_MIGRATION_VERSION: i64 = 20260718210000;
pub(super) const FRONTSTAGE_BLOCK_RENDERER_VERSION_MIGRATION_VERSION: i64 = 20260718220000;

pub(super) fn page_tabs_migrator() -> Migrator {
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

pub(super) fn before_frontstage_workspace_integrity_migrator() -> Migrator {
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

pub(super) fn before_frontstage_page_owner_kind_migrator() -> Migrator {
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

pub(super) fn before_frontstage_placement_integrity_migrator() -> Migrator {
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

pub(super) fn before_frontstage_page_tab_ownership_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FRONTSTAGE_PAGE_TAB_OWNERSHIP_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

pub(super) fn before_frontstage_block_renderer_version_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FRONTSTAGE_BLOCK_RENDERER_VERSION_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

pub(super) async fn insert_pre_page_tab_ownership_documents(
    pool: &PgPool,
    divergent_blocks: bool,
) -> (Uuid, Uuid, Uuid, Uuid, Value) {
    let (workspace_id, _) = insert_frontstage_test_workspaces(pool).await;
    let page_id = Uuid::now_v7();
    let default_tab_id = Uuid::now_v7();
    let analytics_tab_id = Uuid::now_v7();
    let default_blocks = json!([{ "id": "hero", "codeRef": "hero-code" }]);
    let analytics_blocks = json!([{ "id": "chart", "codeRef": "chart-code" }]);
    let default_schema_payload = json!({
        "version": 1,
        "blocks": default_blocks
    });
    let default_root_payload = if divergent_blocks {
        json!({
            "kind": "frontstage.tab.root",
            "blocks": [{ "id": "stale", "codeRef": "stale-code" }]
        })
    } else {
        json!({
            "kind": "frontstage.tab.root",
            "blocks": default_blocks
        })
    };
    let analytics_schema_payload = json!({
        "version": 1,
        "blocks": analytics_blocks
    });
    let analytics_root_payload = json!({
        "kind": "frontstage.tab.root",
        "blocks": analytics_blocks
    });
    let mut transaction = pool.begin().await.unwrap();

    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'page', 'Reports', 'sidebar', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Overview', 'a', true, $4), ($5, $2, $3, 'Analytics', 'b', false, $6)",
    )
    .bind(default_tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{default_tab_id}.root"))
    .bind(analytics_tab_id)
    .bind(format!("frontstage.tab.{analytics_tab_id}.root"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    for (tab_id, root_uid, schema_payload, root_payload) in [
        (
            default_tab_id,
            format!("frontstage.tab.{default_tab_id}.root"),
            default_schema_payload,
            default_root_payload,
        ),
        (
            analytics_tab_id,
            format!("frontstage.tab.{analytics_tab_id}.root"),
            analytics_schema_payload,
            analytics_root_payload,
        ),
    ] {
        sqlx::query(
            "insert into frontstage_page_schemas (id, scope_id, tab_id, workspace_id, root_uid, schema_payload, root_payload) values ($1, $2, $3, $2, $4, $5, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(workspace_id)
        .bind(tab_id)
        .bind(root_uid)
        .bind(schema_payload)
        .bind(root_payload)
        .execute(&mut *transaction)
        .await
        .unwrap();
    }
    transaction.commit().await.unwrap();

    (
        workspace_id,
        page_id,
        default_tab_id,
        analytics_tab_id,
        json!({ "version": 1, "blocks": [{ "id": "chart", "codeRef": "chart-code" }] }),
    )
}

pub(super) async fn insert_frontstage_group_and_page(
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

pub(super) async fn insert_frontstage_test_workspaces(pool: &PgPool) -> (Uuid, Uuid) {
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

pub(super) async fn insert_frontstage_group(
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

pub(super) async fn insert_frontstage_page_with_owner_rows(
    pool: &PgPool,
    workspace_id: Uuid,
) -> (Uuid, Uuid, Uuid, Uuid) {
    let page_id = Uuid::now_v7();
    let default_tab_id = Uuid::now_v7();
    let secondary_tab_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    let mut transaction = pool.begin().await.unwrap();

    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, content_presentation, rank) values ($1, $2, 'page', 'Owner Page', 'tabs', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid, route_segment) values ($1, $2, $3, 'Default', 'a', true, $4, null), ($5, $2, $3, 'Secondary', 'b', false, $6, 'secondary')",
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

pub(super) async fn commit_error_contains(
    transaction: sqlx::Transaction<'_, sqlx::Postgres>,
    expected_message: &str,
) {
    let error = transaction.commit().await.unwrap_err();
    assert!(
        error.to_string().contains(expected_message),
        "unexpected commit error: {error}"
    );
}

pub(super) async fn insert_legacy_frontstage_page(
    pool: &PgPool,
) -> (Uuid, Uuid, Uuid, Value, Value) {
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

pub(super) async fn assert_page_tabs_up_migration_rolled_back(
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

pub(super) async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}
