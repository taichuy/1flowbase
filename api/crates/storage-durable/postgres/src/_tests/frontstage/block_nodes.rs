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
use runtime_core::runtime_record_repository::{OrderedTreeCommandError, OrderedTreeQueryError};

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
        "insert into frontstage_pages (id, workspace_id, kind, title, placement, content_presentation, rank, slug) values ($1, $2, 'page', 'Legacy', 'topbar', 'single', 'U', 'legacy-blocks')",
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
    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            other_page_id,
            other_tab_id,
            "root",
            "other-root-code",
            FrontstageBlockPosition::default(),
            Uuid::now_v7(),
        ))
        .await
        .unwrap();

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
    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "grandchild",
            "grandchild-code",
            FrontstageBlockPosition {
                parent_block_id: Some("child".to_owned()),
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
            expected_affected_count: 3,
            audit_log: delete_audit,
        })
        .await
        .unwrap();
    assert_eq!(deleted.deleted_count, 3);
    let remaining: (i64, i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($3)), (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $4 and block_id = 'root'), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $4 and code_ref = 'other-root-code')",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(vec!["root-code", "child-code", "grandchild-code"])
    .bind(other_page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, (0, 0, 1, 1));
}

// AC-008/009: expected-count, audit and code-cleanup failures roll back the locked subtree write
// instead of leaving structure, code or audit in a partially-applied state.
#[tokio::test]
async fn block_subtree_delete_failures_roll_back_structure_code_and_audit() {
    let (pool, store, workspace_id, actor_user_id) = block_fixture().await;
    let (page_id, tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "delete-rollback").await;
    let root_audit_id = Uuid::now_v7();
    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "root",
            "root-code",
            FrontstageBlockPosition::default(),
            root_audit_id,
        ))
        .await
        .unwrap();
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

    let mismatch_audit_id = Uuid::now_v7();
    let mut mismatch_audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_block",
        Some(page_id),
        "frontstage.block_subtree_deleted",
        json!({ "block_id": "root" }),
    );
    mismatch_audit.id = mismatch_audit_id;
    let error = store
        .delete_frontstage_block_subtree(&DeleteFrontstageBlockSubtreeInput {
            workspace_id,
            page_id,
            block_id: "root".to_owned(),
            expected_affected_count: 1,
            audit_log: mismatch_audit,
        })
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<OrderedTreeCommandError>(),
        Some(&OrderedTreeCommandError::ExpectedAffectedCountMismatch {
            expected: 1,
            actual: 2,
        })
    );
    let after_mismatch: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($3)), (select count(*) from audit_logs where id = $4)",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(vec!["root-code", "child-code"])
    .bind(mismatch_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_mismatch, (2, 2, 0));

    let mut conflicting_audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_block",
        Some(page_id),
        "frontstage.block_subtree_deleted",
        json!({ "block_id": "root" }),
    );
    conflicting_audit.id = root_audit_id;
    assert!(store
        .delete_frontstage_block_subtree(&DeleteFrontstageBlockSubtreeInput {
            workspace_id,
            page_id,
            block_id: "root".to_owned(),
            expected_affected_count: 2,
            audit_log: conflicting_audit,
        })
        .await
        .is_err());
    let after_audit_failure: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($3)), (select count(*) from audit_logs where id = $4)",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(vec!["root-code", "child-code"])
    .bind(root_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_audit_failure, (2, 2, 1));

    // Controlled corruption: remove the normal FK guard so the adapter's cleanup-count invariant
    // is exercised independently of the schema constraint.
    sqlx::query(
        "alter table frontstage_block_nodes drop constraint frontstage_block_nodes_code_fkey",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "delete from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = 'child-code'",
    )
    .bind(workspace_id)
    .bind(page_id)
    .execute(&pool)
    .await
    .unwrap();
    let cleanup_audit_id = Uuid::now_v7();
    let mut cleanup_audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_block",
        Some(page_id),
        "frontstage.block_subtree_deleted",
        json!({ "block_id": "root" }),
    );
    cleanup_audit.id = cleanup_audit_id;
    let error = store
        .delete_frontstage_block_subtree(&DeleteFrontstageBlockSubtreeInput {
            workspace_id,
            page_id,
            block_id: "root".to_owned(),
            expected_affected_count: 2,
            audit_log: cleanup_audit,
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("code cleanup does not match deleted nodes"));
    let after_cleanup_failure: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = 'root-code'), (select count(*) from audit_logs where id = $3)",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(cleanup_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_cleanup_failure, (2, 1, 0));
}

// AC-008/011: a structural write committed while delete waits for the ordered-tree advisory lock
// is included in the locked snapshot, so stale expected counts fail before any cleanup or audit.
#[tokio::test]
async fn block_subtree_delete_revalidates_after_waiting_for_structure_lock() {
    let (pool, store, workspace_id, actor_user_id) = block_fixture().await;
    let (page_id, tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "delete-race").await;
    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "root",
            "root-code",
            FrontstageBlockPosition::default(),
            Uuid::now_v7(),
        ))
        .await
        .unwrap();
    let root_internal_id: Uuid = sqlx::query_scalar(
        "select id from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and block_id = 'root'",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut lock_tx = pool.begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text || ':' || $3::text, 0))")
        .bind(Uuid::from_u128(
            0xe6aa0cc5_dfc0_8d8d_b6c8_b9bd0113a61a,
        ))
        .bind(workspace_id)
        .bind(page_id)
        .execute(&mut *lock_tx)
        .await
        .unwrap();
    let delete_audit_id = Uuid::now_v7();
    let waiting_delete = {
        let store = store.clone();
        tokio::spawn(async move {
            let mut delete_audit = audit_log(
                Some(workspace_id),
                Some(actor_user_id),
                "frontstage_block",
                Some(page_id),
                "frontstage.block_subtree_deleted",
                json!({ "block_id": "root" }),
            );
            delete_audit.id = delete_audit_id;
            store
                .delete_frontstage_block_subtree(&DeleteFrontstageBlockSubtreeInput {
                    workspace_id,
                    page_id,
                    block_id: "root".to_owned(),
                    expected_affected_count: 1,
                    audit_log: delete_audit,
                })
                .await
        })
    };
    tokio::task::yield_now().await;
    assert!(!waiting_delete.is_finished());

    let raced_node_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'raced-code', 'export default null;')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(page_id)
    .execute(&mut *lock_tx)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into frontstage_block_nodes (
            id, scope_id, tree_partition_id, parent_id, sibling_rank, block_id, tab_id,
            presentation, code_ref, runtime_descriptor
        ) values ($1, $2, $3, $4, 'U', 'raced-child', $5, 'page', 'raced-code', '{}')
        "#,
    )
    .bind(raced_node_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(root_internal_id)
    .bind(tab_id)
    .execute(&mut *lock_tx)
    .await
    .unwrap();
    lock_tx.commit().await.unwrap();

    let error = waiting_delete.await.unwrap().unwrap_err();
    assert_eq!(
        error.downcast_ref::<OrderedTreeCommandError>(),
        Some(&OrderedTreeCommandError::ExpectedAffectedCountMismatch {
            expected: 1,
            actual: 2,
        })
    );
    let after_race: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_block_nodes where scope_id = $1 and tree_partition_id = $2 and id = any($3)), (select count(*) from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = any($4)), (select count(*) from audit_logs where id = $5)",
    )
    .bind(workspace_id)
    .bind(page_id)
    .bind(vec![root_internal_id, raced_node_id])
    .bind(vec!["root-code", "raced-code"])
    .bind(delete_audit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_race, (2, 2, 0));
}

// AC-003/008: frontstage structural reads preserve generic ordered-tree semantics while exposing
// stable public identities and the intentionally narrow tree-summary contract.
#[tokio::test]
async fn block_node_structural_reads_delegate_and_map_public_tree_context() {
    let (_pool, store, workspace_id, actor_user_id) = block_fixture().await;
    let (page_id, tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "tree-reads").await;
    let (other_page_id, other_tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "tree-isolation").await;

    let mut root_a = create_input(
        workspace_id,
        actor_user_id,
        page_id,
        tab_id,
        "root-a",
        "root-a-code",
        FrontstageBlockPosition::default(),
        Uuid::now_v7(),
    );
    root_a.title = Some("Root A".to_owned());
    store.create_frontstage_block_node(&root_a).await.unwrap();

    let mut root_b = create_input(
        workspace_id,
        actor_user_id,
        page_id,
        tab_id,
        "root-b",
        "root-b-code",
        FrontstageBlockPosition {
            after_block_id: Some("root-a".to_owned()),
            ..Default::default()
        },
        Uuid::now_v7(),
    );
    root_b.title = Some("Root B".to_owned());
    store.create_frontstage_block_node(&root_b).await.unwrap();

    let mut match_a = create_input(
        workspace_id,
        actor_user_id,
        page_id,
        tab_id,
        "match-a",
        "match-a-code",
        FrontstageBlockPosition {
            parent_block_id: Some("root-a".to_owned()),
            ..Default::default()
        },
        Uuid::now_v7(),
    );
    match_a.title = Some("Match Alpha".to_owned());
    store.create_frontstage_block_node(&match_a).await.unwrap();

    let mut match_b = create_input(
        workspace_id,
        actor_user_id,
        page_id,
        tab_id,
        "match-b",
        "match-b-code",
        FrontstageBlockPosition {
            parent_block_id: Some("root-a".to_owned()),
            after_block_id: Some("match-a".to_owned()),
            ..Default::default()
        },
        Uuid::now_v7(),
    );
    match_b.title = Some("Match Beta".to_owned());
    store.create_frontstage_block_node(&match_b).await.unwrap();

    let mut leaf = create_input(
        workspace_id,
        actor_user_id,
        page_id,
        tab_id,
        "leaf",
        "leaf-code",
        FrontstageBlockPosition {
            parent_block_id: Some("match-a".to_owned()),
            ..Default::default()
        },
        Uuid::now_v7(),
    );
    leaf.title = Some("Leaf".to_owned());
    store.create_frontstage_block_node(&leaf).await.unwrap();

    let mut isolated = create_input(
        workspace_id,
        actor_user_id,
        other_page_id,
        other_tab_id,
        "match-isolated",
        "match-isolated-code",
        FrontstageBlockPosition::default(),
        Uuid::now_v7(),
    );
    isolated.title = Some("Match Isolated".to_owned());
    store.create_frontstage_block_node(&isolated).await.unwrap();

    let roots = store
        .list_frontstage_block_roots(workspace_id, page_id, 10)
        .await
        .unwrap();
    assert_eq!(
        roots
            .iter()
            .map(|node| node.block_id.as_str())
            .collect::<Vec<_>>(),
        ["root-a", "root-b"]
    );
    assert!(roots.iter().all(|node| {
        node.workspace_id == workspace_id
            && node.page_id == page_id
            && node.parent_block_id.is_none()
            && node.schema_version == 1
    }));

    let children = store
        .list_frontstage_block_children(workspace_id, page_id, "root-a", 10)
        .await
        .unwrap();
    assert_eq!(
        children
            .iter()
            .map(|node| node.block_id.as_str())
            .collect::<Vec<_>>(),
        ["match-a", "match-b"]
    );
    assert!(children
        .iter()
        .all(|node| node.parent_block_id.as_deref() == Some("root-a")));

    let ancestors = store
        .list_frontstage_block_ancestors(workspace_id, page_id, "leaf")
        .await
        .unwrap();
    assert_eq!(
        ancestors
            .iter()
            .map(|node| node.block_id.as_str())
            .collect::<Vec<_>>(),
        ["root-a", "match-a"]
    );

    let descendants = store
        .list_frontstage_block_descendants(workspace_id, page_id, "root-a", 8, 10)
        .await
        .unwrap();
    let descendants = descendants
        .into_iter()
        .map(|projection| (projection.node.block_id.clone(), projection))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(descendants.len(), 3);
    assert_eq!(descendants["match-a"].depth, 1);
    assert!(descendants["match-a"].has_children);
    assert_eq!(descendants["match-a"].path, ["root-a", "match-a"]);
    assert_eq!(descendants["match-b"].depth, 1);
    assert!(!descendants["match-b"].has_children);
    assert_eq!(descendants["leaf"].depth, 2);
    assert_eq!(descendants["leaf"].path, ["root-a", "match-a", "leaf"]);

    let matches = store
        .search_frontstage_blocks(workspace_id, page_id, "match", 10)
        .await
        .unwrap();
    assert_eq!(
        matches
            .iter()
            .map(|result| result.node.block_id.as_str())
            .collect::<Vec<_>>(),
        ["match-a", "match-b"]
    );
    assert!(matches.iter().all(|result| {
        result
            .ancestors
            .iter()
            .map(|node| node.block_id.as_str())
            .eq(["root-a"])
    }));

    let impact = store
        .get_frontstage_block_subtree_impact(workspace_id, page_id, "root-a")
        .await
        .unwrap();
    assert_eq!(impact.affected_count, 4);

    let parent_error = store
        .list_frontstage_block_children(workspace_id, other_page_id, "root-a", 10)
        .await
        .unwrap_err();
    assert_eq!(
        parent_error.downcast_ref::<OrderedTreeQueryError>(),
        Some(&OrderedTreeQueryError::ParentNotFound)
    );
    let node_error = store
        .list_frontstage_block_ancestors(workspace_id, other_page_id, "root-a")
        .await
        .unwrap_err();
    assert_eq!(
        node_error.downcast_ref::<OrderedTreeQueryError>(),
        Some(&OrderedTreeQueryError::NodeNotFound)
    );
    let limit_error = store
        .list_frontstage_block_roots(workspace_id, page_id, 0)
        .await
        .unwrap_err();
    assert_eq!(
        limit_error.downcast_ref::<OrderedTreeQueryError>(),
        Some(&OrderedTreeQueryError::InvalidResultLimit { max: 1_000 })
    );
}
