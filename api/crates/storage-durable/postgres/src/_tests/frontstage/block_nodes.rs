use std::collections::BTreeMap;

use control_plane::{
    audit::audit_log,
    ports::{
        CreateFrontstageBlockNodeInput, CreateFrontstagePageInput, CreateFrontstagePageTabInput,
        DeleteFrontstageBlockSubtreeInput, FrontstageBlockPosition, FrontstageBlockTreeRepository,
        FrontstagePageRepository,
    },
};
use domain::FrontstageBlockPresentation;

use super::*;
use crate::PgControlPlaneStore;

async fn create_page_and_tab(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    slug: &str,
) -> (Uuid, Uuid) {
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    store
        .create_frontstage_page(&CreateFrontstagePageInput {
            id: page_id,
            workspace_id,
            actor_user_id,
            parent_id: None,
            kind: domain::FrontstagePageKind::Page,
            title: Some(slug.to_owned()),
            icon: None,
            tooltip: None,
            placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
            content_presentation: domain::frontstage::FrontstagePageContentPresentation::Tabs,
            slug: Some(slug.to_owned()),
            rank: format!("{slug}U"),
            default_tab: Some(CreateFrontstagePageTabInput {
                id: tab_id,
                workspace_id,
                actor_user_id,
                page_id,
                title: None,
                rank: "U".to_owned(),
                is_default: true,
                route_segment: None,
                document_root_uid: format!("frontstage.tab.{tab_id}.root"),
            }),
        })
        .await
        .unwrap();
    (page_id, tab_id)
}

fn create_input(
    workspace_id: Uuid,
    actor_user_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: &str,
    code_ref: &str,
    position: FrontstageBlockPosition,
    audit_id: Uuid,
) -> CreateFrontstageBlockNodeInput {
    let mut audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_block",
        Some(page_id),
        "frontstage.block_node_created",
        json!({ "block_id": block_id }),
    );
    audit.id = audit_id;
    CreateFrontstageBlockNodeInput {
        workspace_id,
        actor_user_id,
        page_id,
        tab_id,
        block_id: block_id.to_owned(),
        position,
        presentation: FrontstageBlockPresentation::Page,
        title: None,
        code_ref: code_ref.to_owned(),
        schema_version: 1,
        input_mapping: BTreeMap::new(),
        output_mapping: BTreeMap::new(),
        runtime_descriptor: json!({ "id": block_id, "codeRef": code_ref }),
        code: format!("export default function {block_id}() {{ return null; }}"),
        audit_log: audit,
    }
}

async fn block_fixture() -> (PgPool, PgControlPlaneStore, Uuid, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let actor_user_id = Uuid::now_v7();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'x', 'Block Nodes', 'Block Nodes', 'active')",
    )
    .bind(actor_user_id)
    .bind(format!("block-nodes-{actor_user_id}"))
    .bind(format!("block-nodes-{actor_user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, '00000000-0000-0000-0000-000000000001', 'Block Nodes', $2, $2)",
    )
    .bind(workspace_id)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    (pool, store, workspace_id, actor_user_id)
}

// AC-001/008: the cutover preserves the public id and complete runtime descriptor while deriving
// only the frozen page presentation, nullable title, empty typed mappings and stable array order.
#[tokio::test]
async fn block_node_migration_backfills_legacy_roots_without_descriptor_loss() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_block_nodes_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let descriptor = json!({ "id": "hero", "codeRef": "hero-code" });
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, content_presentation, rank, slug) values ($1, $2, 'page', 'Legacy', 'sidebar', 'single', 'U', 'legacy-blocks')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, rank, is_default, document_root_uid) values ($1, $2, $3, 'U', true, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_schemas (id, scope_id, workspace_id, tab_id, root_uid, schema_payload, root_payload, document_payload) values ($1, $2, $2, $3, $4, $5, $5, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(tab_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .bind(json!({ "version": 1, "blocks": [descriptor.clone()] }))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'hero-code', 'export default 1;')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(page_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let row: (String, Uuid, Uuid, String, String, Option<String>, Value, Value, Value) =
        sqlx::query_as(
            "select block_id, tree_partition_id, tab_id, sibling_rank, presentation, title, input_mapping, output_mapping, runtime_descriptor from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2",
        )
        .bind(workspace_id)
        .bind(page_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "hero");
    assert_eq!(row.1, page_id);
    assert_eq!(row.2, tab_id);
    assert!(row.3.ends_with('U'));
    assert_eq!(row.4, "page");
    assert_eq!(row.5, None);
    assert_eq!(row.6, json!({}));
    assert_eq!(row.7, json!({}));
    assert_eq!(row.8, descriptor);
}

// AC-009: child-container state has no lossless block-node mapping and must stop the migration.
#[tokio::test]
async fn block_node_migration_rejects_nonempty_legacy_child_containers() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_block_nodes_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, content_presentation, rank, slug) values ($1, $2, 'page', 'Legacy', 'sidebar', 'single', 'U', 'legacy-child')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, rank, is_default, document_root_uid) values ($1, $2, $3, 'U', true, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&mut *tx)
    .await
    .unwrap();
    let dirty = json!({
        "version": 1,
        "blocks": [],
        "child_containers": [{ "id": "drawer", "block_ids": ["hero"] }]
    });
    sqlx::query(
        "insert into frontstage_page_schemas (id, scope_id, workspace_id, tab_id, root_uid, schema_payload, root_payload, document_payload) values ($1, $2, $2, $3, $4, $5, $5, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(tab_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .bind(dirty)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("frontstage block node migration rejected legacy child-container data"));
    let table_exists: bool = sqlx::query_scalar(
        "select to_regclass(current_schema() || '.frontstage_block_nodes') is not null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!table_exists);
}

// AC-008/009: structure, code and audit share one transaction; public identities remain page and
// tab scoped; subtree deletion removes its exclusively-owned source rows.
#[tokio::test]
async fn block_node_repository_is_atomic_scoped_and_cleans_subtree_codes() {
    let (pool, store, workspace_id, actor_user_id) = block_fixture().await;
    let (page_id, tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "blocks-a").await;
    let (other_page_id, other_tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "blocks-b").await;
    let audit_id = Uuid::now_v7();
    let root = store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "root",
            "root-code",
            FrontstageBlockPosition::default(),
            audit_id,
        ))
        .await
        .unwrap();
    assert_eq!(root.block_id, "root");

    let failed = store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "rolled-back",
            "rolled-back-code",
            FrontstageBlockPosition::default(),
            audit_id,
        ))
        .await;
    assert!(failed.is_err());
    let rolled_back: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where block_id = 'rolled-back'), (select count(*) from frontstage_block_codes where code_ref = 'rolled-back-code')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rolled_back, (0, 0));

    let cross_page = store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            other_page_id,
            other_tab_id,
            "cross-page",
            "cross-page-code",
            FrontstageBlockPosition {
                parent_block_id: Some("root".to_owned()),
                ..Default::default()
            },
            Uuid::now_v7(),
        ))
        .await;
    assert!(cross_page.is_err());

    let second_tab_id = Uuid::now_v7();
    store
        .create_frontstage_page_tab(&CreateFrontstagePageTabInput {
            id: second_tab_id,
            workspace_id,
            actor_user_id,
            page_id,
            title: Some("Second".to_owned()),
            rank: "k".to_owned(),
            is_default: false,
            route_segment: Some("second".to_owned()),
            document_root_uid: format!("frontstage.tab.{second_tab_id}.root"),
        })
        .await
        .unwrap();
    let cross_tab = store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            second_tab_id,
            "cross-tab",
            "cross-tab-code",
            FrontstageBlockPosition {
                parent_block_id: Some("root".to_owned()),
                ..Default::default()
            },
            Uuid::now_v7(),
        ))
        .await;
    assert!(cross_tab.is_err());

    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "child",
            "child-code",
            FrontstageBlockPosition {
                parent_block_id: Some("root".to_owned()),
                ..Default::default()
            },
            Uuid::now_v7(),
        ))
        .await
        .unwrap();
    let mut delete_audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_block",
        Some(page_id),
        "frontstage.block_subtree_deleted",
        json!({ "block_id": "root" }),
    );
    delete_audit.id = Uuid::now_v7();
    let deleted = store
        .delete_frontstage_block_subtree(&DeleteFrontstageBlockSubtreeInput {
            workspace_id,
            page_id,
            block_id: "root".to_owned(),
            expected_affected_count: 2,
            audit_log: delete_audit,
        })
        .await
        .unwrap();
    assert_eq!(deleted.deleted_count, 2);
    let remaining: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($3))",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(vec!["root-code", "child-code"])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, (0, 0));
}
