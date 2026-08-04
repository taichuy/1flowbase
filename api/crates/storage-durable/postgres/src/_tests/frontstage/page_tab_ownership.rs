use super::*;

#[tokio::test]
async fn page_tab_ownership_migration_backfills_presentation_routes_and_document_payload() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_page_tab_ownership_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, page_id, default_tab_id, analytics_tab_id, _analytics_document) =
        insert_pre_page_tab_ownership_documents(&pool, false).await;

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let presentation: String = sqlx::query_scalar(
        "select content_presentation from frontstage_pages where workspace_id = $1 and id = $2",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(presentation, "tabs");
    let routes: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        "select id, route_segment from frontstage_page_tabs where workspace_id = $1 and page_id = $2 order by rank",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(routes.contains(&(default_tab_id, None)));
    assert!(routes.contains(&(
        analytics_tab_id,
        Some(format!("tab-{}", analytics_tab_id.simple()))
    )));
    let document_payload: Value = sqlx::query_scalar(
        "select document_payload from frontstage_page_schemas where workspace_id = $1 and tab_id = $2",
    )
    .bind(workspace_id)
    .bind(analytics_tab_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        document_payload,
        json!({
            "version": 1,
            "blocks": [{
                "id": "chart",
                "codeRef": "chart-code",
                "renderer_version": "v1"
            }]
        })
    );
}

#[tokio::test]
async fn page_tab_ownership_migration_rejects_divergent_legacy_blocks_without_partial_schema_change(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_page_tab_ownership_migrator()
        .run(&pool)
        .await
        .unwrap();
    insert_pre_page_tab_ownership_documents(&pool, true).await;

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(
        error.to_string().contains(
            "frontstage tab document migration rejected divergent schema and root blocks"
        ),
        "unexpected migration error: {error}"
    );
    let content_presentation_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'content_presentation')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!content_presentation_exists);
}

#[tokio::test]
async fn frontstage_block_renderer_version_migration_backfills_document_and_compatibility_projections(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_block_renderer_version_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let root_uid = format!("frontstage.tab.{tab_id}.root");
    let legacy_blocks = json!([
        {
            "id": "hero",
            "codeRef": "hero-code",
            "props": { "title": "Hero" }
        },
        {
            "id": "future",
            "codeRef": "future-code",
            "renderer_version": "v2"
        }
    ]);

    let mut transaction = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, content_presentation, rank) values ($1, $2, 'page', 'Versioned Page', 'sidebar', 'single', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid, route_segment) values ($1, $2, $3, 'Default', 'a', true, $4, null)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(&root_uid)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_schemas (id, scope_id, tab_id, workspace_id, root_uid, schema_payload, root_payload, document_payload) values ($1, $2, $3, $2, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(tab_id)
    .bind(&root_uid)
    .bind(json!({
        "version": 1,
        "schema_meta": { "owner": "compat" },
        "blocks": legacy_blocks
    }))
    .bind(json!({
        "uid": root_uid,
        "kind": "frontstage.tab.root",
        "root_meta": { "owner": "compat" },
        "blocks": legacy_blocks
    }))
    .bind(json!({
        "version": 1,
        "document_meta": { "owner": "document" },
        "blocks": legacy_blocks
    }))
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let (schema_payload, root_payload, document_payload): (Value, Value, Value) = sqlx::query_as(
        "select schema_payload, root_payload, document_payload from frontstage_page_schemas where workspace_id = $1 and tab_id = $2",
    )
    .bind(workspace_id)
    .bind(tab_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expected_blocks = json!([
        {
            "id": "hero",
            "codeRef": "hero-code",
            "props": { "title": "Hero" },
            "renderer_version": "v1"
        },
        {
            "id": "future",
            "codeRef": "future-code",
            "renderer_version": "v2"
        }
    ]);

    assert_eq!(document_payload["blocks"], expected_blocks);
    assert_eq!(schema_payload["blocks"], expected_blocks);
    assert_eq!(root_payload["blocks"], expected_blocks);
    assert_eq!(
        document_payload["document_meta"],
        json!({ "owner": "document" })
    );
    assert_eq!(schema_payload["schema_meta"], json!({ "owner": "compat" }));
    assert_eq!(root_payload["root_meta"], json!({ "owner": "compat" }));
}
