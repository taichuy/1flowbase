use std::sync::Arc;

use control_plane::ports::{CreateModelDefinitionInput, ModelDefinitionRepository};
use domain::DataModelScopeKind;
use runtime_core::{
    model_metadata::ModelMetadata,
    runtime_record_repository::{
        OrderedTreeCommandError, OrderedTreeCreateInput, OrderedTreeCreatePosition,
        OrderedTreeLeafDeleteInput, OrderedTreeMoveInput, OrderedTreeMovePosition,
        OrderedTreeStructureRepository, OrderedTreeSubtreeDeleteInput, RuntimeRecordRepository,
    },
};
use serde_json::json;
use tokio::sync::Barrier;
use uuid::Uuid;

use super::runtime_metadata as metadata;
use crate::{
    ordered_tree::commands::{
        create_ordered_tree_node_in_transaction, delete_ordered_tree_leaf_in_transaction,
        delete_ordered_tree_subtree_in_transaction, move_ordered_tree_node_in_transaction,
    },
    run_migrations, PgControlPlaneStore,
};

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn create_workspace(store: &PgControlPlaneStore) -> Uuid {
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query("insert into workspaces (id, tenant_id, name) values ($1, $2, $3)")
        .bind(workspace_id)
        .bind(tenant_id)
        .bind(format!("Ordered Tree Commands {}", workspace_id.simple()))
        .execute(store.pool())
        .await
        .unwrap();
    workspace_id
}

async fn create_model(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    suffix: &str,
) -> domain::ModelDefinitionRecord {
    ModelDefinitionRepository::create_model_definition(
        store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: "ordered_tree".to_owned(),
            template_version: "v1".to_owned(),
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: format!("tree_{suffix}_{}", workspace_id.simple()),
            title: "Ordered Tree Commands".to_owned(),
            description: None,
        },
    )
    .await
    .unwrap()
}

fn position(
    parent_id: Option<Uuid>,
    before_id: Option<Uuid>,
    after_id: Option<Uuid>,
) -> OrderedTreeCreatePosition {
    OrderedTreeCreatePosition {
        parent_id,
        before_id,
        after_id,
    }
}

fn move_position(
    new_parent_id: Option<Uuid>,
    before_id: Option<Uuid>,
    after_id: Option<Uuid>,
) -> OrderedTreeMovePosition {
    OrderedTreeMovePosition {
        new_parent_id,
        before_id,
        after_id,
    }
}

async fn create_node(
    store: &PgControlPlaneStore,
    metadata: &ModelMetadata,
    scope_id: Uuid,
    position: OrderedTreeCreatePosition,
) -> Uuid {
    create_node_in_partition(store, metadata, scope_id, scope_id, position).await
}

async fn create_node_in_partition(
    store: &PgControlPlaneStore,
    metadata: &ModelMetadata,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    position: OrderedTreeCreatePosition,
) -> Uuid {
    store
        .create_ordered_tree_node(
            metadata,
            OrderedTreeCreateInput {
                actor_user_id: Uuid::nil(),
                scope_id,
                tree_partition_id,
                payload: json!({}),
                position,
            },
        )
        .await
        .unwrap()
        .node_id
}

fn typed_error(error: anyhow::Error) -> OrderedTreeCommandError {
    error
        .downcast::<OrderedTreeCommandError>()
        .expect("ordered-tree command should preserve its typed business error")
}

// Packet A1b fixture: crate-internal commands compose into a caller-owned transaction while the
// existing acceptance matrix continues to exercise the atomic public begin/commit boundary.
#[tokio::test]
async fn caller_owned_transaction_controls_ordered_tree_command_commit_and_rollback() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let tree_partition_id = scope_id;
    let model = create_model(&store, scope_id, "caller_tx").await;
    let metadata = metadata(&model);

    let mut tx = store.pool().begin().await.unwrap();
    let rolled_back_create = create_ordered_tree_node_in_transaction(
        &mut tx,
        &metadata,
        OrderedTreeCreateInput {
            actor_user_id: Uuid::nil(),
            scope_id,
            tree_partition_id,
            payload: json!({}),
            position: position(None, None, None),
        },
    )
    .await
    .unwrap()
    .node_id;
    tx.rollback().await.unwrap();
    let create_rollback_count: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = $3",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(rolled_back_create)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(create_rollback_count, 0);

    let mut tx = store.pool().begin().await.unwrap();
    let committed_root = create_ordered_tree_node_in_transaction(
        &mut tx,
        &metadata,
        OrderedTreeCreateInput {
            actor_user_id: Uuid::nil(),
            scope_id,
            tree_partition_id,
            payload: json!({}),
            position: position(None, None, None),
        },
    )
    .await
    .unwrap()
    .node_id;
    tx.commit().await.unwrap();
    let create_commit_count: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = $3",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(committed_root)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(create_commit_count, 1);

    let committed_child = create_node(
        &store,
        &metadata,
        scope_id,
        position(Some(committed_root), None, None),
    )
    .await;
    let alternate_parent =
        create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let leaf = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let before_move: (Option<Uuid>, String) = sqlx::query_as(&format!(
        "select parent_id, sibling_rank from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = $3",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(committed_child)
    .fetch_one(store.pool())
    .await
    .unwrap();

    let mut tx = store.pool().begin().await.unwrap();
    move_ordered_tree_node_in_transaction(
        &mut tx,
        &metadata,
        OrderedTreeMoveInput {
            actor_user_id: Uuid::nil(),
            scope_id,
            tree_partition_id,
            node_id: committed_child,
            position: move_position(Some(alternate_parent), None, None),
        },
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let after_move_rollback: (Option<Uuid>, String) = sqlx::query_as(&format!(
        "select parent_id, sibling_rank from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = $3",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(committed_child)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(after_move_rollback, before_move);

    let mut tx = store.pool().begin().await.unwrap();
    assert!(delete_ordered_tree_leaf_in_transaction(
        &mut tx,
        &metadata,
        OrderedTreeLeafDeleteInput {
            scope_id,
            tree_partition_id,
            node_id: leaf,
        },
    )
    .await
    .unwrap());
    tx.rollback().await.unwrap();
    let leaf_after_rollback: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = $3",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(leaf)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(leaf_after_rollback, 1);

    let mut tx = store.pool().begin().await.unwrap();
    let error = delete_ordered_tree_subtree_in_transaction(
        &mut tx,
        &metadata,
        OrderedTreeSubtreeDeleteInput {
            scope_id,
            tree_partition_id,
            node_id: committed_root,
            expected_affected_count: 1,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        typed_error(error),
        OrderedTreeCommandError::ExpectedAffectedCountMismatch {
            expected: 1,
            actual: 2,
        }
    );
    tx.rollback().await.unwrap();
    let subtree_after_mismatch: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = any($3)",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(vec![committed_root, committed_child])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(subtree_after_mismatch, 2);

    let mut tx = store.pool().begin().await.unwrap();
    let deleted = delete_ordered_tree_subtree_in_transaction(
        &mut tx,
        &metadata,
        OrderedTreeSubtreeDeleteInput {
            scope_id,
            tree_partition_id,
            node_id: committed_root,
            expected_affected_count: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(deleted.deleted_count, 2);
    tx.commit().await.unwrap();
    let subtree_after_commit: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = $2 and id = any($3)",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(vec![committed_root, committed_child])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(subtree_after_commit, 0);
}

// AC-007/AC-008/AC-011: real transactions enforce positioning, cycles, write isolation,
// ordinary-rank write rejection, leaf restriction, and subtree count compare-and-delete.
#[tokio::test]
async fn ordered_tree_structure_command_acceptance_matrix() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let other_scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "matrix").await;
    let metadata = metadata(&model);
    let other_partition_id = Uuid::now_v7();
    let partition_parent = create_node_in_partition(
        &store,
        &metadata,
        scope_id,
        other_partition_id,
        position(None, None, None),
    )
    .await;

    let first = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let third = create_node(
        &store,
        &metadata,
        scope_id,
        position(None, None, Some(first)),
    )
    .await;
    let second = create_node(
        &store,
        &metadata,
        scope_id,
        position(None, Some(third), None),
    )
    .await;
    // AC-002: repository validation and the composite FK both reject parents
    // from another structural partition inside the same authorized scope.
    let error = store
        .create_ordered_tree_node(
            &metadata,
            OrderedTreeCreateInput {
                actor_user_id: Uuid::nil(),
                scope_id,
                tree_partition_id: scope_id,
                payload: json!({}),
                position: position(Some(partition_parent), None, None),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(typed_error(error), OrderedTreeCommandError::ParentNotFound);
    assert!(!store
        .delete_ordered_tree_leaf(
            &metadata,
            OrderedTreeLeafDeleteInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: partition_parent,
            },
        )
        .await
        .unwrap());
    let error = store
        .delete_ordered_tree_subtree(
            &metadata,
            OrderedTreeSubtreeDeleteInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: partition_parent,
                expected_affected_count: 1,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(typed_error(error), OrderedTreeCommandError::NodeNotFound);
    let database_error = sqlx::query(&format!(
        "insert into \"{}\" (id, scope_id, tree_partition_id, parent_id, sibling_rank) values ($1, $2, $2, $3, 'z')",
        model.physical_table_name
    ))
    .bind(Uuid::now_v7())
    .bind(scope_id)
    .bind(partition_parent)
    .execute(store.pool())
    .await
    .unwrap_err();
    let database_error_code = database_error
        .as_database_error()
        .and_then(|error| error.code())
        .map(|code| code.into_owned());
    assert_eq!(database_error_code.as_deref(), Some("23503"));
    let error = store
        .move_ordered_tree_node(
            &metadata,
            OrderedTreeMoveInput {
                actor_user_id: Uuid::nil(),
                scope_id,
                tree_partition_id: scope_id,
                node_id: second,
                position: move_position(Some(partition_parent), None, None),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(typed_error(error), OrderedTreeCommandError::ParentNotFound);
    let ordered_ids: Vec<Uuid> = sqlx::query_scalar(&format!(
        "select id from \"{}\" where scope_id = $1 and tree_partition_id = scope_id and parent_id is null order by sibling_rank collate \"C\", id",
        model.physical_table_name
    ))
    .bind(scope_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(ordered_ids, vec![first, second, third]);

    let child = create_node(
        &store,
        &metadata,
        scope_id,
        position(Some(first), None, None),
    )
    .await;
    let error = store
        .create_ordered_tree_node(
            &metadata,
            OrderedTreeCreateInput {
                actor_user_id: Uuid::nil(),
                scope_id,
                tree_partition_id: scope_id,
                payload: json!({}),
                position: position(None, Some(first), Some(second)),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(
        typed_error(error),
        OrderedTreeCommandError::ConflictingAnchors
    );
    let grandchild = create_node(
        &store,
        &metadata,
        scope_id,
        position(Some(child), None, None),
    )
    .await;
    for invalid_parent in [Some(first), Some(grandchild)] {
        let error = store
            .move_ordered_tree_node(
                &metadata,
                OrderedTreeMoveInput {
                    actor_user_id: Uuid::nil(),
                    scope_id,
                    tree_partition_id: scope_id,
                    node_id: first,
                    position: move_position(invalid_parent, None, None),
                },
            )
            .await
            .unwrap_err();
        assert_eq!(typed_error(error), OrderedTreeCommandError::Cycle);
    }

    let cross_scope_parent = sqlx::query(&format!(
        "insert into \"{}\" (id, scope_id, tree_partition_id, sibling_rank) values ($1, $2, $2, 'U')",
        model.physical_table_name
    ))
    .bind(Uuid::now_v7())
    .bind(other_scope_id)
    .execute(store.pool())
    .await
    .unwrap();
    assert_eq!(cross_scope_parent.rows_affected(), 1);
    let cross_scope_parent_id: Uuid = sqlx::query_scalar(&format!(
        "select id from \"{}\" where scope_id = $1",
        model.physical_table_name
    ))
    .bind(other_scope_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let error = store
        .move_ordered_tree_node(
            &metadata,
            OrderedTreeMoveInput {
                actor_user_id: Uuid::nil(),
                scope_id,
                tree_partition_id: scope_id,
                node_id: second,
                position: move_position(Some(cross_scope_parent_id), None, None),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(typed_error(error), OrderedTreeCommandError::ParentNotFound);

    let before_move: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(&format!(
        "select id, parent_id, sibling_rank from \"{}\" where scope_id = $1 and tree_partition_id = scope_id order by id",
        model.physical_table_name
    ))
    .bind(scope_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    store
        .move_ordered_tree_node(
            &metadata,
            OrderedTreeMoveInput {
                actor_user_id: Uuid::nil(),
                scope_id,
                tree_partition_id: scope_id,
                node_id: third,
                position: move_position(Some(first), None, None),
            },
        )
        .await
        .unwrap();
    let after_move: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(&format!(
        "select id, parent_id, sibling_rank from \"{}\" where scope_id = $1 and tree_partition_id = scope_id order by id",
        model.physical_table_name
    ))
    .bind(scope_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    let changed = before_move
        .iter()
        .zip(&after_move)
        .filter(|(before, after)| before != after)
        .map(|(_, after)| after.0)
        .collect::<Vec<_>>();
    assert_eq!(changed, vec![third]);

    for field_code in ["parent_id", "sibling_rank"] {
        let error = RuntimeRecordRepository::update_record(
            &store,
            &metadata,
            Uuid::nil(),
            Some(scope_id),
            None,
            &second.to_string(),
            json!({ (field_code): "forbidden" }),
        )
        .await
        .unwrap_err();
        assert_eq!(
            typed_error(error),
            OrderedTreeCommandError::FieldNotWritable(field_code.to_owned())
        );
    }

    let error = store
        .delete_ordered_tree_leaf(
            &metadata,
            OrderedTreeLeafDeleteInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: first,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(typed_error(error).code(), "tree_node_has_children");

    let count_before: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = scope_id",
        model.physical_table_name
    ))
    .bind(scope_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let error = store
        .delete_ordered_tree_subtree(
            &metadata,
            OrderedTreeSubtreeDeleteInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: first,
                expected_affected_count: 2,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        typed_error(error),
        OrderedTreeCommandError::ExpectedAffectedCountMismatch { .. }
    ));
    let count_after: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = scope_id",
        model.physical_table_name
    ))
    .bind(scope_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count_after, count_before);

    let deleted = store
        .delete_ordered_tree_subtree(
            &metadata,
            OrderedTreeSubtreeDeleteInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: first,
                expected_affected_count: 4,
            },
        )
        .await
        .unwrap();
    assert_eq!(deleted.deleted_count, 4);
    assert!(store
        .delete_ordered_tree_leaf(
            &metadata,
            OrderedTreeLeafDeleteInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: second,
            },
        )
        .await
        .unwrap());
}

// AC-008: opposite concurrent moves serialize on the model/scope lock; exactly one can win.
#[tokio::test]
async fn concurrent_opposite_moves_return_one_cycle_conflict() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "opposite").await;
    let metadata = metadata(&model);
    let left = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let right = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let barrier = Arc::new(Barrier::new(2));

    let left_move = {
        let store = store.clone();
        let metadata = metadata.clone();
        let barrier = barrier.clone();
        async move {
            barrier.wait().await;
            store
                .move_ordered_tree_node(
                    &metadata,
                    OrderedTreeMoveInput {
                        actor_user_id: Uuid::nil(),
                        scope_id,
                        tree_partition_id: scope_id,
                        node_id: left,
                        position: move_position(Some(right), None, None),
                    },
                )
                .await
        }
    };
    let right_move = {
        let store = store.clone();
        let metadata = metadata.clone();
        let barrier = barrier.clone();
        async move {
            barrier.wait().await;
            store
                .move_ordered_tree_node(
                    &metadata,
                    OrderedTreeMoveInput {
                        actor_user_id: Uuid::nil(),
                        scope_id,
                        tree_partition_id: scope_id,
                        node_id: right,
                        position: move_position(Some(left), None, None),
                    },
                )
                .await
        }
    };
    let (left_result, right_result) = tokio::join!(left_move, right_move);
    assert_eq!(left_result.is_ok() as u8 + right_result.is_ok() as u8, 1);
    let error = left_result.err().or_else(|| right_result.err()).unwrap();
    assert_eq!(typed_error(error), OrderedTreeCommandError::Cycle);
}

// AC-007: an over-threshold allocation rebalances only the locked target sibling group, then retries.
#[tokio::test]
async fn create_rebalances_dense_target_siblings_before_retry() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "rebalance").await;
    let metadata = metadata(&model);
    let left = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let right = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    sqlx::query(&format!(
        "update \"{}\" set sibling_rank = case id when $1 then $3 else $4 end where scope_id = $2 and tree_partition_id = scope_id and id = any($5)",
        model.physical_table_name
    ))
    .bind(left)
    .bind(scope_id)
    .bind("U".repeat(33))
    .bind(format!("{}V", "U".repeat(32)))
    .bind(vec![left, right])
    .execute(store.pool())
    .await
    .unwrap();

    let inserted = create_node(
        &store,
        &metadata,
        scope_id,
        position(None, Some(right), None),
    )
    .await;
    let rows: Vec<(Uuid, String)> = sqlx::query_as(&format!(
        "select id, sibling_rank from \"{}\" where scope_id = $1 and tree_partition_id = scope_id order by sibling_rank collate \"C\", id",
        model.physical_table_name
    ))
    .bind(scope_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.0).collect::<Vec<_>>(),
        vec![left, inserted, right]
    );
    assert!(rows.iter().all(|row| row.1.len() < 32));
}

// AC-008/AC-011: commands waiting on the advisory lock re-read anchor membership and subtree size.
#[tokio::test]
async fn waiting_commands_revalidate_anchor_and_expected_subtree_count() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "revalidate").await;
    let metadata = metadata(&model);
    let parent = create_node(&store, &metadata, scope_id, position(None, None, None)).await;
    let anchor = create_node(&store, &metadata, scope_id, position(None, None, None)).await;

    let mut lock_tx = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text || ':' || $3::text, 0))")
        .bind(model.id)
        .bind(scope_id)
        .bind(scope_id)
        .execute(&mut *lock_tx)
        .await
        .unwrap();
    let waiting_create = {
        let store = store.clone();
        let metadata = metadata.clone();
        tokio::spawn(async move {
            store
                .create_ordered_tree_node(
                    &metadata,
                    OrderedTreeCreateInput {
                        actor_user_id: Uuid::nil(),
                        scope_id,
                        tree_partition_id: scope_id,
                        payload: json!({}),
                        position: position(None, Some(anchor), None),
                    },
                )
                .await
        })
    };
    tokio::task::yield_now().await;
    sqlx::query(&format!(
        "update \"{}\" set parent_id = $1 where scope_id = $2 and tree_partition_id = scope_id and id = $3",
        model.physical_table_name
    ))
    .bind(parent)
    .bind(scope_id)
    .bind(anchor)
    .execute(&mut *lock_tx)
    .await
    .unwrap();
    lock_tx.commit().await.unwrap();
    let error = waiting_create.await.unwrap().unwrap_err();
    assert_eq!(
        typed_error(error),
        OrderedTreeCommandError::AnchorSiblingGroupConflict
    );

    let mut count_lock_tx = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text || ':' || $3::text, 0))")
        .bind(model.id)
        .bind(scope_id)
        .bind(scope_id)
        .execute(&mut *count_lock_tx)
        .await
        .unwrap();
    let waiting_delete = {
        let store = store.clone();
        let metadata = metadata.clone();
        tokio::spawn(async move {
            store
                .delete_ordered_tree_subtree(
                    &metadata,
                    OrderedTreeSubtreeDeleteInput {
                        scope_id,
                        tree_partition_id: scope_id,
                        node_id: parent,
                        expected_affected_count: 2,
                    },
                )
                .await
        })
    };
    tokio::task::yield_now().await;
    let raced_child = Uuid::now_v7();
    sqlx::query(&format!(
        "insert into \"{}\" (id, scope_id, tree_partition_id, parent_id, sibling_rank) values ($1, $2, $2, $3, 'k')",
        model.physical_table_name
    ))
    .bind(raced_child)
    .bind(scope_id)
    .bind(parent)
    .execute(&mut *count_lock_tx)
    .await
    .unwrap();
    count_lock_tx.commit().await.unwrap();
    let error = waiting_delete.await.unwrap().unwrap_err();
    assert_eq!(
        typed_error(error),
        OrderedTreeCommandError::ExpectedAffectedCountMismatch {
            expected: 2,
            actual: 3,
        }
    );
    let still_present: i64 = sqlx::query_scalar(&format!(
        "select count(*)::bigint from \"{}\" where scope_id = $1 and tree_partition_id = scope_id and id = any($2)",
        model.physical_table_name
    ))
    .bind(scope_id)
    .bind(vec![parent, anchor, raced_child])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(still_present, 3);
}
