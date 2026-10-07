use std::borrow::Cow;

use control_plane_contracts::ports::{AddModelFieldInput, ModelDefinitionRepository};
use serde_json::json;
use sqlx::{migrate::Migrator, PgPool};
use storage_durable::runtime_record_repository::{
    OrderedTreeDescendantsInput, OrderedTreeMoveInput, OrderedTreeMovePosition,
    OrderedTreeNodeInput, OrderedTreeQueryError, OrderedTreeQueryRepository,
    OrderedTreeStructureRepository, RuntimeRecordRepository,
};
use uuid::Uuid;

use super::{
    create_legacy_ordered_tree_model, create_ordered_tree_model, create_workspace,
    isolated_database, runtime_metadata,
};
use crate::{
    ordered_tree::commands::move_ordered_tree_node_in_transaction, run_migrations,
    PgControlPlaneStore,
};

const PATH_MIGRATION: i64 = 20261007120000;

fn before_paths() -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("./migrations")
                .iter()
                .filter(|migration| migration.version < PATH_MIGRATION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

async fn insert_dynamic(
    pool: &PgPool,
    table: &str,
    scope: Uuid,
    partition: Uuid,
    id: Uuid,
    parent: Option<Uuid>,
    rank: &str,
) {
    sqlx::query(&format!("insert into \"{table}\" (id, scope_id, tree_partition_id, parent_id, sibling_rank) values ($1, $2, $3, $4, $5)"))
        .bind(id).bind(scope).bind(partition).bind(parent).bind(rank).execute(pool).await.unwrap();
}

async fn path(pool: &PgPool, table: &str, id: Uuid) -> String {
    sqlx::query_scalar(&format!(
        "select tree_path::text from \"{table}\" where id = $1"
    ))
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn labels(ids: &[Uuid]) -> String {
    ids.iter()
        .map(|id| id.simple().to_string())
        .collect::<Vec<_>>()
        .join(".")
}

async fn legacy_fixture() -> (
    PgPool,
    PgControlPlaneStore,
    domain::ModelDefinitionRecord,
    Uuid,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    before_paths().run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let scope = create_workspace(&store).await;
    let model = create_legacy_ordered_tree_model(&store, scope).await;
    (pool, store, model, scope)
}

// AC-007/011: three existing tree families contain data before the official
// migration runs, and retain stable UUID identity and indexed private paths.
#[tokio::test]
async fn migration_backfills_dynamic_departments_and_frontstage_with_partition_isolation() {
    let (pool, store, model, scope) = legacy_fixture().await;
    let root = Uuid::now_v7();
    let child = Uuid::now_v7();
    insert_dynamic(
        &pool,
        &model.physical_table_name,
        scope,
        scope,
        root,
        None,
        "U",
    )
    .await;
    insert_dynamic(
        &pool,
        &model.physical_table_name,
        scope,
        scope,
        child,
        Some(root),
        "U",
    )
    .await;
    let grandchild = Uuid::now_v7();
    insert_dynamic(
        &pool,
        &model.physical_table_name,
        scope,
        scope,
        grandchild,
        Some(child),
        "U",
    )
    .await;
    let other_partition = Uuid::now_v7();
    let other_root = Uuid::now_v7();
    insert_dynamic(
        &pool,
        &model.physical_table_name,
        scope,
        other_partition,
        other_root,
        None,
        "U",
    )
    .await;
    // Old deployed bootstrap registers the built-in physical table too. It
    // must be installed once even when present in the dynamic-model catalog.
    sqlx::query("insert into model_definitions (id, scope_kind, scope_id, code, title, physical_table_name, acl_namespace, audit_namespace, availability_status, status, owner_kind, is_protected, template_provider, template_code, template_version, source_kind) values ($1, 'system', $2, 'departments', 'Departments', 'departments', 'departments', 'departments', 'available', 'published', 'core', true, 'core', 'ordered_tree', 'v1', 'main_source')")
        .bind(domain::DEPARTMENT_MODEL_ID).bind(Uuid::nil()).execute(&pool).await.unwrap();
    let dept_root = Uuid::now_v7();
    let dept_child = Uuid::now_v7();
    let dept_grandchild = Uuid::now_v7();
    for (id, parent) in [
        (dept_root, None),
        (dept_child, Some(dept_root)),
        (dept_grandchild, Some(dept_child)),
    ] {
        sqlx::query("insert into departments (id, scope_id, tree_partition_id, parent_id, sibling_rank, name) values ($1, $2, $2, $3, 'U', 'Legacy')")
            .bind(id).bind(scope).bind(parent).execute(&pool).await.unwrap();
    }
    let page = Uuid::now_v7();
    let tab = Uuid::now_v7();
    sqlx::query("insert into frontstage_pages (id, workspace_id, kind, title, placement, content_presentation, rank, slug) values ($1, $2, 'page', 'Path Fixture', 'topbar', 'tabs', 'U', 'path-fixture')")
        .bind(page).bind(scope).execute(&pool).await.unwrap();
    sqlx::query("insert into frontstage_page_tabs (id, workspace_id, page_id, rank, is_default, document_root_uid) values ($1, $2, $3, 'U', true, 'path-fixture-root')")
        .bind(tab).bind(scope).bind(page).execute(&pool).await.unwrap();
    let block_root = Uuid::now_v7();
    let block_child = Uuid::now_v7();
    let block_grandchild = Uuid::now_v7();
    for (id, parent, code) in [
        (block_root, None, "path-root"),
        (block_child, Some(block_root), "path-child"),
        (block_grandchild, Some(block_child), "path-grandchild"),
    ] {
        sqlx::query("insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, $4, 'export default null;')")
            .bind(Uuid::now_v7()).bind(scope).bind(page).bind(code).execute(&pool).await.unwrap();
        let descriptor = json!({"id":code,"codeRef":code,"rendererVersion":"1","catalog":{},"contribution":{},"props":{},"ports":{"inputs":[],"outputs":[]},"x-layout":{},"x-presentation":{},"runtime":{}});
        sqlx::query("insert into frontstage_block_nodes (id, scope_id, tree_partition_id, parent_id, sibling_rank, block_id, tab_id, presentation, code_ref, runtime_descriptor) values ($1, $2, $3, $4, 'U', $5, $6, 'page', $5, $7)")
            .bind(id).bind(scope).bind(page).bind(parent).bind(code).bind(tab).bind(descriptor).execute(&pool).await.unwrap();
    }
    run_migrations(&pool).await.unwrap();
    for (table, root, child, grandchild) in [
        (&*model.physical_table_name, root, child, grandchild),
        ("departments", dept_root, dept_child, dept_grandchild),
        (
            "frontstage_block_nodes",
            block_root,
            block_child,
            block_grandchild,
        ),
    ] {
        assert_eq!(path(&pool, table, root).await, labels(&[root]));
        assert_eq!(path(&pool, table, child).await, labels(&[root, child]));
        let indexed: bool = sqlx::query_scalar("select exists(select 1 from pg_indexes where schemaname = current_schema() and tablename = $1 and indexdef like '%USING gist%' and indexdef like '%ARRAY[tree_path]%')")
            .bind(table).fetch_one(&pool).await.unwrap();
        assert!(indexed, "{table} must have its path GiST index");
        // Old native writers still move and rename trees consistently.
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(&format!(
            "update \"{table}\" set parent_id = null, sibling_rank = 'k' where id = $1"
        ))
        .bind(child)
        .execute(&mut *tx)
        .await
        .unwrap();
        let changed: String = sqlx::query_scalar(&format!(
            "select tree_path::text from \"{table}\" where id = $1"
        ))
        .bind(child)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(changed, labels(&[child]));
        let moved_descendant: String = sqlx::query_scalar(&format!(
            "select tree_path::text from \"{table}\" where id = $1"
        ))
        .bind(grandchild)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert_eq!(moved_descendant, labels(&[child, grandchild]));
        tx.rollback().await.unwrap();
        assert_eq!(path(&pool, table, child).await, labels(&[root, child]));
        assert_eq!(
            path(&pool, table, grandchild).await,
            labels(&[root, child, grandchild])
        );
    }
    sqlx::query("update departments set name = 'Renamed' where id = $1")
        .bind(dept_root)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        path(&pool, "departments", dept_grandchild).await,
        labels(&[dept_root, dept_child, dept_grandchild])
    );
    assert_eq!(
        path(&pool, &model.physical_table_name, other_root).await,
        labels(&[other_root])
    );
    let descendants = store
        .list_ordered_tree_descendants(
            &runtime_metadata(&model),
            OrderedTreeDescendantsInput {
                scope_id: scope,
                tree_partition_id: scope,
                node_id: root,
                max_depth: 5,
                result_limit: 100,
                include_path: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(descendants.len(), 2);
    assert_eq!(descendants[0].path, Some(vec![root, child]));
    assert_eq!(descendants[1].path, Some(vec![root, child, grandchild]));
    assert!(descendants[0].record.get("tree_path").is_none());
}

// AC-007: failure is transactional; no half-column or half-backfill remains.
#[tokio::test]
async fn migration_rejects_cycles_missing_parents_and_reserved_column_conflicts() {
    for corruption in ["cycle", "missing_parent", "column", "metadata"] {
        let (pool, _store, model, scope) = legacy_fixture().await;
        let root = Uuid::now_v7();
        let child = Uuid::now_v7();
        insert_dynamic(
            &pool,
            &model.physical_table_name,
            scope,
            scope,
            root,
            None,
            "U",
        )
        .await;
        insert_dynamic(
            &pool,
            &model.physical_table_name,
            scope,
            scope,
            child,
            Some(root),
            "U",
        )
        .await;
        if corruption == "metadata" {
            sqlx::query("insert into model_fields (id, data_model_id, code, title, physical_column_name, field_kind) values ($1, $2, 'tree_path', 'User path', 'user_path', 'string')")
                .bind(Uuid::now_v7()).bind(model.id).execute(&pool).await.unwrap();
        } else if corruption == "column" {
            sqlx::query(&format!(
                "alter table \"{}\" add column tree_path text",
                model.physical_table_name
            ))
            .execute(&pool)
            .await
            .unwrap();
        } else {
            if corruption == "missing_parent" {
                sqlx::query(&format!(
                    "alter table \"{}\" drop constraint \"fk_ot_parent_{}\"",
                    model.physical_table_name,
                    model.id.simple()
                ))
                .execute(&pool)
                .await
                .unwrap();
            }
            sqlx::query(&format!(
                "update \"{}\" set parent_id = $1 where id = $2",
                model.physical_table_name
            ))
            .bind(if corruption == "cycle" {
                child
            } else {
                Uuid::now_v7()
            })
            .bind(root)
            .execute(&pool)
            .await
            .unwrap();
        }
        assert!(
            run_migrations(&pool).await.is_err(),
            "{corruption} must abort migration"
        );
        let builtin_columns: i64 = sqlx::query_scalar("select count(*) from information_schema.columns where table_schema = current_schema() and table_name in ('departments', 'frontstage_block_nodes') and column_name = 'tree_path'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(builtin_columns, 0);
        let typ: Option<String> = sqlx::query_scalar("select udt_name from information_schema.columns where table_schema = current_schema() and table_name = $1 and column_name = 'tree_path'")
            .bind(&model.physical_table_name).fetch_optional(&pool).await.unwrap();
        assert_eq!(typ.as_deref(), (corruption == "column").then_some("text"));
    }
}

#[tokio::test]
async fn path_move_updates_whole_subtree_and_rolls_back_without_reordering_identity() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let scope = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope).await;
    let metadata = runtime_metadata(&model);
    let table = &model.physical_table_name;
    let root = Uuid::now_v7();
    let branch = Uuid::now_v7();
    let leaf = Uuid::now_v7();
    let destination = Uuid::now_v7();
    for (id, parent, rank) in [
        (root, None, "U"),
        (destination, None, "k"),
        (branch, Some(root), "U"),
        (leaf, Some(branch), "U"),
    ] {
        insert_dynamic(&pool, table, scope, scope, id, parent, rank).await;
    }
    let movement = OrderedTreeMoveInput {
        actor_user_id: Uuid::nil(),
        scope_id: scope,
        tree_partition_id: scope,
        node_id: branch,
        position: OrderedTreeMovePosition {
            new_parent_id: Some(destination),
            before_id: None,
            after_id: None,
        },
    };
    let mut tx = pool.begin().await.unwrap();
    move_ordered_tree_node_in_transaction(&mut tx, &metadata, movement.clone())
        .await
        .unwrap();
    let moved: String = sqlx::query_scalar(&format!(
        "select tree_path::text from \"{table}\" where id = $1"
    ))
    .bind(leaf)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(moved, labels(&[destination, branch, leaf]));
    tx.rollback().await.unwrap();
    assert_eq!(
        path(&pool, table, leaf).await,
        labels(&[root, branch, leaf])
    );
    store
        .move_ordered_tree_node(&metadata, movement)
        .await
        .unwrap();
    assert_eq!(
        path(&pool, table, leaf).await,
        labels(&[destination, branch, leaf])
    );
    sqlx::query(&format!(
        "update \"{table}\" set sibling_rank = 'k' where id = $1"
    ))
    .bind(branch)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        path(&pool, table, leaf).await,
        labels(&[destination, branch, leaf])
    );
    for statement in [format!("update \"{table}\" set parent_id = $1 where id = $2"), format!("update \"{table}\" set tree_path = replace($1::text, '-', '')::public.ltree where id = $2")] {
        assert!(sqlx::query(&statement).bind(leaf).bind(destination).execute(&pool).await.is_err());
    }
    let foreign_scope = create_workspace(&store).await;
    let foreign_root = Uuid::now_v7();
    insert_dynamic(
        &pool,
        table,
        foreign_scope,
        foreign_scope,
        foreign_root,
        None,
        "U",
    )
    .await;
    assert!(sqlx::query(&format!(
        "update \"{table}\" set parent_id = $1 where id = $2"
    ))
    .bind(foreign_root)
    .bind(branch)
    .execute(&pool)
    .await
    .is_err());
    assert_eq!(
        path(&pool, table, leaf).await,
        labels(&[destination, branch, leaf])
    );
}

#[tokio::test]
async fn deep_tree_has_no_storage_depth_cap_and_preserves_ancestor_query_limit() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let scope = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope).await;
    let mut nodes = Vec::new();
    for _ in 0..300 {
        let id = Uuid::now_v7();
        insert_dynamic(
            &pool,
            &model.physical_table_name,
            scope,
            scope,
            id,
            nodes.last().copied(),
            "U",
        )
        .await;
        nodes.push(id);
    }
    assert_eq!(
        path(&pool, &model.physical_table_name, nodes[299]).await,
        labels(&nodes)
    );
    let error = store
        .list_ordered_tree_ancestors(
            &runtime_metadata(&model),
            OrderedTreeNodeInput {
                scope_id: scope,
                tree_partition_id: scope,
                node_id: nodes[299],
            },
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<OrderedTreeQueryError>(),
        Some(&OrderedTreeQueryError::AncestorDepthLimitExceeded { max: 256 })
    );
    store
        .move_ordered_tree_node(
            &runtime_metadata(&model),
            OrderedTreeMoveInput {
                actor_user_id: Uuid::nil(),
                scope_id: scope,
                tree_partition_id: scope,
                node_id: nodes[1],
                position: OrderedTreeMovePosition {
                    new_parent_id: None,
                    before_id: None,
                    after_id: None,
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(
        path(&pool, &model.physical_table_name, nodes[299]).await,
        labels(&nodes[1..])
    );
}

#[tokio::test]
async fn tree_path_cannot_be_declared_as_user_metadata_even_without_physical_ddl() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope).await;
    let node = Uuid::now_v7();
    insert_dynamic(
        store.pool(),
        &model.physical_table_name,
        scope,
        scope,
        node,
        None,
        "U",
    )
    .await;
    let write_error = store
        .update_record(
            &runtime_metadata(&model),
            Uuid::nil(),
            Some(scope),
            None,
            &node.to_string(),
            json!({"tree_path": "forged"}),
        )
        .await
        .unwrap_err();
    assert_eq!(
        write_error
            .downcast_ref::<storage_durable::runtime_record_repository::OrderedTreeCommandError>(),
        Some(
            &storage_durable::runtime_record_repository::OrderedTreeCommandError::FieldNotWritable(
                "tree_path".into()
            )
        )
    );
    for (code, physical) in [("tree_path", None), ("ordinary", Some("tree_path"))] {
        let result = store
            .add_model_field(&AddModelFieldInput {
                actor_user_id: Uuid::nil(),
                model_id: model.id,
                external_field_key: None,
                code: code.into(),
                title: code.into(),
                description: None,
                field_kind: domain::ModelFieldKind::String,
                is_system: false,
                is_writable: true,
                apply_physical_schema: false,
                is_required: false,
                api_required: false,
                is_unique: false,
                default_value: None,
                display_interface: None,
                display_options: json!({}),
                relation_target_model_id: None,
                physical_column_name: physical.map(str::to_owned),
                relation_options: json!({}),
            })
            .await;
        assert!(result.is_err());
    }
    assert!(!store
        .get_model_definition(Uuid::nil(), model.id)
        .await
        .unwrap()
        .unwrap()
        .fields
        .iter()
        .any(|field| field.code == "tree_path" || field.physical_column_name == "tree_path"));
}

// Native writers wait on the same structure lock before deriving parent paths.
// Opposite moves may lose by cycle/deadlock rejection but cannot both commit.
#[tokio::test]
async fn concurrent_native_opposite_moves_preserve_committed_paths() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let scope = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope).await;
    let left = Uuid::now_v7();
    let right = Uuid::now_v7();
    insert_dynamic(
        &pool,
        &model.physical_table_name,
        scope,
        scope,
        left,
        None,
        "U",
    )
    .await;
    insert_dynamic(
        &pool,
        &model.physical_table_name,
        scope,
        scope,
        right,
        None,
        "k",
    )
    .await;
    let mut lock_tx = pool.begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text || ':' || $2::text, 0))")
        .bind(model.id).bind(scope).execute(&mut *lock_tx).await.unwrap();
    let mut tasks = Vec::new();
    for (node, parent) in [(left, right), (right, left)] {
        let pool = pool.clone();
        let table = model.physical_table_name.clone();
        tasks.push(tokio::spawn(async move {
            sqlx::query(&format!(
                "update \"{table}\" set parent_id = $1 where id = $2"
            ))
            .bind(parent)
            .bind(node)
            .execute(&pool)
            .await
        }));
    }
    tokio::task::yield_now().await;
    lock_tx.commit().await.unwrap();
    let first = tasks.remove(0).await.unwrap();
    let second = tasks.remove(0).await.unwrap();
    assert_ne!(first.is_ok(), second.is_ok());
    if first.is_ok() {
        assert_eq!(
            path(&pool, &model.physical_table_name, left).await,
            labels(&[right, left])
        );
        assert_eq!(
            path(&pool, &model.physical_table_name, right).await,
            labels(&[right])
        );
    } else {
        assert_eq!(
            path(&pool, &model.physical_table_name, right).await,
            labels(&[left, right])
        );
        assert_eq!(
            path(&pool, &model.physical_table_name, left).await,
            labels(&[left])
        );
    }
}

// AC-011: bounded performance evidence from actual indexed SQL, without a
// planner-choice or timing assertion that would fail on different hardware.
#[tokio::test]
async fn path_read_and_subtree_move_emit_explain_buffers_evidence() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let scope = create_workspace(&store).await;
    let model = create_ordered_tree_model(&store, scope).await;
    let table = &model.physical_table_name;
    let root = Uuid::now_v7();
    let branch = Uuid::now_v7();
    let destination = Uuid::now_v7();
    for (id, parent, rank) in [
        (root, None, "U"),
        (destination, None, "k"),
        (branch, Some(root), "U"),
    ] {
        insert_dynamic(&pool, table, scope, scope, id, parent, rank).await;
    }
    let mut branch_leaves = Vec::new();
    for index in 0..512 {
        let id = Uuid::now_v7();
        let parent = if index < 64 {
            branch_leaves.push(id);
            branch
        } else {
            root
        };
        insert_dynamic(
            &pool,
            table,
            scope,
            scope,
            id,
            Some(parent),
            &format!("U{}", id.simple()),
        )
        .await;
    }
    sqlx::query(&format!("analyze \"{table}\""))
        .execute(&pool)
        .await
        .unwrap();
    let statement = format!("select count(*)::bigint from \"{table}\" node join \"{table}\" root on root.scope_id = $1 and root.tree_partition_id = $1 and root.id = $2 where node.scope_id = $1 and node.tree_partition_id = $1 and ARRAY[node.tree_path] OPERATOR(public.<@) root.tree_path");
    let count: i64 = sqlx::query_scalar(&statement)
        .bind(scope)
        .bind(branch)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 65);
    let read: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze, buffers, format json) {statement}"
    ))
    .bind(scope)
    .bind(branch)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let movement: serde_json::Value = sqlx::query_scalar(&format!("explain (analyze, buffers, format json) update \"{table}\" set parent_id = $1 where scope_id = $2 and tree_partition_id = $2 and id = $3"))
        .bind(destination).bind(scope).bind(branch).fetch_one(&mut *tx).await.unwrap();
    let moved: String = sqlx::query_scalar(&format!(
        "select tree_path::text from \"{table}\" where id = $1"
    ))
    .bind(branch_leaves[0])
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(moved, labels(&[destination, branch, branch_leaves[0]]));
    tx.rollback().await.unwrap();
    assert_eq!(
        path(&pool, table, branch_leaves[0]).await,
        labels(&[root, branch, branch_leaves[0]])
    );
    eprintln!(
        "ltree evidence: nodes=515 moved=65 read={} move={}",
        read, movement
    );
}
