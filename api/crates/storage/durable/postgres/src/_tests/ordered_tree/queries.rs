use control_plane_contracts::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, ModelDefinitionRepository,
};
use domain::{DataModelScopeKind, ModelFieldKind};
use storage_durable::runtime_record_repository::{
    OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeDescendantsInput,
    OrderedTreeNodeInput, OrderedTreeQueryError, OrderedTreeQueryRepository,
    OrderedTreeSearchInput, OrderedTreeSubtreeImpactInput,
};
use uuid::Uuid;

use super::runtime_metadata as metadata;
use crate::{run_migrations, PgControlPlaneStore};

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
        .bind(format!("Ordered Tree Queries {}", workspace_id.simple()))
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
            code: format!("query_{suffix}_{}", workspace_id.simple()),
            title: "Ordered Tree Queries".to_owned(),
            description: None,
        },
    )
    .await
    .unwrap()
}

async fn add_search_field(
    store: &PgControlPlaneStore,
    model: &domain::ModelDefinitionRecord,
) -> (domain::ModelDefinitionRecord, domain::ModelFieldRecord) {
    let field = ModelDefinitionRepository::add_model_field(
        store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            external_field_key: None,
            code: "title".to_owned(),
            title: "Title".to_owned(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".to_owned()),
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            physical_column_name: None,
            relation_options: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    let model = ModelDefinitionRepository::get_model_definition(store, Uuid::nil(), model.id)
        .await
        .unwrap()
        .unwrap();
    (model, field)
}

struct TestNode<'a> {
    id: Uuid,
    parent_id: Option<Uuid>,
    sibling_rank: &'a str,
    title: &'a str,
}

impl<'a> TestNode<'a> {
    fn new(id: Uuid, parent_id: Option<Uuid>, sibling_rank: &'a str, title: &'a str) -> Self {
        Self {
            id,
            parent_id,
            sibling_rank,
            title,
        }
    }
}

async fn insert_node(
    store: &PgControlPlaneStore,
    model: &domain::ModelDefinitionRecord,
    field: &domain::ModelFieldRecord,
    scope_id: Uuid,
    node: TestNode<'_>,
) {
    insert_node_in_partition(store, model, field, scope_id, scope_id, node).await;
}

async fn insert_node_in_partition(
    store: &PgControlPlaneStore,
    model: &domain::ModelDefinitionRecord,
    field: &domain::ModelFieldRecord,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node: TestNode<'_>,
) {
    sqlx::query(&format!(
        "insert into \"{}\" (id, scope_id, tree_partition_id, parent_id, sibling_rank, \"{}\") values ($1, $2, $3, $4, $5, $6)",
        model.physical_table_name, field.physical_column_name
    ))
    .bind(node.id)
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(node.parent_id)
    .bind(node.sibling_rank)
    .bind(node.title)
    .execute(store.pool())
    .await
    .unwrap();
}

fn record_id(record: &serde_json::Value) -> Uuid {
    Uuid::parse_str(record["id"].as_str().unwrap()).unwrap()
}

fn typed_error(error: anyhow::Error) -> OrderedTreeQueryError {
    error
        .downcast::<OrderedTreeQueryError>()
        .expect("ordered-tree query should preserve its typed business error")
}

// Packet A1c fixture: the product-neutral query owner counts the target and every reachable
// descendant exactly once without crossing scope/partition boundaries, including corrupt cycles.
#[tokio::test]
async fn subtree_impact_counts_leaf_nested_partition_and_cycle_with_typed_not_found() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "impact").await;
    let (model, field) = add_search_field(&store, &model).await;
    let metadata = metadata(&model);
    let root = Uuid::now_v7();
    let child = Uuid::now_v7();
    let leaf = Uuid::now_v7();
    for node in [
        TestNode::new(root, None, "U", "Root"),
        TestNode::new(child, Some(root), "U", "Child"),
        TestNode::new(leaf, Some(child), "U", "Leaf"),
    ] {
        insert_node(&store, &model, &field, scope_id, node).await;
    }
    let other_partition_id = Uuid::now_v7();
    let partition_root = Uuid::now_v7();
    let partition_child = Uuid::now_v7();
    insert_node_in_partition(
        &store,
        &model,
        &field,
        scope_id,
        other_partition_id,
        TestNode::new(partition_root, None, "U", "Partition Root"),
    )
    .await;
    insert_node_in_partition(
        &store,
        &model,
        &field,
        scope_id,
        other_partition_id,
        TestNode::new(
            partition_child,
            Some(partition_root),
            "U",
            "Partition Child",
        ),
    )
    .await;

    let leaf_impact = store
        .get_ordered_tree_subtree_impact(
            &metadata,
            OrderedTreeSubtreeImpactInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: leaf,
            },
        )
        .await
        .unwrap();
    assert_eq!(leaf_impact.affected_count, 1);
    let nested_impact = store
        .get_ordered_tree_subtree_impact(
            &metadata,
            OrderedTreeSubtreeImpactInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: root,
            },
        )
        .await
        .unwrap();
    assert_eq!(nested_impact.affected_count, 3);
    let partition_impact = store
        .get_ordered_tree_subtree_impact(
            &metadata,
            OrderedTreeSubtreeImpactInput {
                scope_id,
                tree_partition_id: other_partition_id,
                node_id: partition_root,
            },
        )
        .await
        .unwrap();
    assert_eq!(partition_impact.affected_count, 2);

    for (tree_partition_id, node_id) in [(scope_id, partition_root), (scope_id, Uuid::now_v7())] {
        let error = store
            .get_ordered_tree_subtree_impact(
                &metadata,
                OrderedTreeSubtreeImpactInput {
                    scope_id,
                    tree_partition_id,
                    node_id,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(typed_error(error), OrderedTreeQueryError::NodeNotFound);
    }

    sqlx::query(&format!(
        "update \"{}\" set parent_id = $1 where scope_id = $2 and tree_partition_id = $2 and id = $3",
        model.physical_table_name
    ))
    .bind(leaf)
    .bind(scope_id)
    .bind(root)
    .execute(store.pool())
    .await
    .unwrap();
    let cycle_impact = store
        .get_ordered_tree_subtree_impact(
            &metadata,
            OrderedTreeSubtreeImpactInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: root,
            },
        )
        .await
        .unwrap();
    assert_eq!(cycle_impact.affected_count, 3);

    let limit_error = store
        .list_ordered_tree_roots(
            &metadata,
            OrderedTreeBoundedListInput {
                scope_id,
                tree_partition_id: scope_id,
                result_limit: 1_001,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        typed_error(limit_error),
        OrderedTreeQueryError::InvalidResultLimit { .. }
    ));
}

// AC-009/AC-014: bounded deep/wide projections preserve tree order and expose paths only opt-in.
#[tokio::test]
async fn bounded_tree_queries_cover_order_depth_path_scope_and_not_found() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let other_scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "bounds").await;
    let (model, field) = add_search_field(&store, &model).await;
    let metadata = metadata(&model);
    let other_partition_id = Uuid::now_v7();
    let partition_root = Uuid::now_v7();
    let partition_child = Uuid::now_v7();
    insert_node_in_partition(
        &store,
        &model,
        &field,
        scope_id,
        other_partition_id,
        TestNode::new(partition_root, None, "U", "Partition root"),
    )
    .await;
    insert_node_in_partition(
        &store,
        &model,
        &field,
        scope_id,
        other_partition_id,
        TestNode::new(
            partition_child,
            Some(partition_root),
            "U",
            "Partition child",
        ),
    )
    .await;

    let root_a = Uuid::now_v7();
    let root_b = Uuid::now_v7();
    let child_a = Uuid::now_v7();
    let child_b = Uuid::now_v7();
    let grandchild = Uuid::now_v7();
    let great_grandchild = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(root_b, None, "k", "Root B"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(root_a, None, "F", "Root A"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(child_b, Some(root_a), "k", "Child B"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(child_a, Some(root_a), "F", "Child A"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(grandchild, Some(child_a), "U", "Grandchild"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(great_grandchild, Some(grandchild), "U", "Great Grandchild"),
    )
    .await;
    let foreign_node = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        other_scope_id,
        TestNode::new(foreign_node, None, "U", "Foreign"),
    )
    .await;

    let roots = store
        .list_ordered_tree_roots(
            &metadata,
            OrderedTreeBoundedListInput {
                scope_id,
                tree_partition_id: scope_id,
                result_limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        roots
            .iter()
            .map(|node| record_id(&node.record))
            .collect::<Vec<_>>(),
        vec![root_a, root_b]
    );
    assert!(roots
        .iter()
        .all(|node| record_id(&node.record) != partition_root));
    let children = store
        .list_ordered_tree_children(
            &metadata,
            OrderedTreeChildrenInput {
                scope_id,
                tree_partition_id: scope_id,
                parent_id: root_a,
                result_limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        children
            .iter()
            .map(|node| record_id(&node.record))
            .collect::<Vec<_>>(),
        vec![child_a, child_b]
    );
    assert!(children
        .iter()
        .all(|node| record_id(&node.record) != partition_child));
    let mut wide_ids = Vec::new();
    for index in 0..20 {
        let node_id = Uuid::now_v7();
        wide_ids.push(node_id);
        insert_node(
            &store,
            &model,
            &field,
            scope_id,
            TestNode::new(node_id, Some(root_b), &format!("{index:04}U"), "Wide Child"),
        )
        .await;
    }
    let bounded_wide = store
        .list_ordered_tree_children(
            &metadata,
            OrderedTreeChildrenInput {
                scope_id,
                tree_partition_id: scope_id,
                parent_id: root_b,
                result_limit: 7,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        bounded_wide
            .iter()
            .map(|node| record_id(&node.record))
            .collect::<Vec<_>>(),
        wide_ids[..7]
    );

    let ancestors = store
        .list_ordered_tree_ancestors(
            &metadata,
            OrderedTreeNodeInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: great_grandchild,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        ancestors
            .iter()
            .map(|node| record_id(&node.record))
            .collect::<Vec<_>>(),
        vec![root_a, child_a, grandchild]
    );
    assert!(ancestors.iter().all(|node| {
        let id = record_id(&node.record);
        id != partition_root && id != partition_child
    }));

    let descendants = store
        .list_ordered_tree_descendants(
            &metadata,
            OrderedTreeDescendantsInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: root_a,
                max_depth: 2,
                result_limit: 10,
                include_path: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        descendants
            .iter()
            .map(|node| (record_id(&node.record), node.depth))
            .collect::<Vec<_>>(),
        vec![(child_a, 1), (grandchild, 2), (child_b, 1)]
    );
    assert!(descendants.iter().all(|node| {
        let id = record_id(&node.record);
        id != partition_root && id != partition_child
    }));
    assert_eq!(
        descendants[1].path.as_deref(),
        Some([root_a, child_a, grandchild].as_slice())
    );
    assert!(descendants[1].has_children);
    let without_paths = store
        .list_ordered_tree_descendants(
            &metadata,
            OrderedTreeDescendantsInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: root_a,
                max_depth: 1,
                result_limit: 1,
                include_path: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(without_paths.len(), 1);
    assert!(without_paths[0].path.is_none());

    let error = store
        .list_ordered_tree_descendants(
            &metadata,
            OrderedTreeDescendantsInput {
                scope_id,
                tree_partition_id: scope_id,
                node_id: root_a,
                max_depth: 0,
                result_limit: 10,
                include_path: false,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        typed_error(error),
        OrderedTreeQueryError::InvalidMaxDepth { .. }
    ));
    let error = store
        .list_ordered_tree_roots(
            &metadata,
            OrderedTreeBoundedListInput {
                scope_id,
                tree_partition_id: scope_id,
                result_limit: 1_001,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        typed_error(error),
        OrderedTreeQueryError::InvalidResultLimit { .. }
    ));
    let error = store
        .list_ordered_tree_children(
            &metadata,
            OrderedTreeChildrenInput {
                scope_id,
                tree_partition_id: scope_id,
                parent_id: foreign_node,
                result_limit: 10,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(typed_error(error), OrderedTreeQueryError::ParentNotFound);
}

// AC-010: prefix search is case-insensitive, bounded, deduplicates ancestors, and adds no descendants.
#[tokio::test]
async fn prefix_search_returns_match_markers_and_ancestor_context_only() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "search").await;
    let (model, field) = add_search_field(&store, &model).await;
    let metadata = metadata(&model);
    let partition_match = Uuid::now_v7();
    insert_node_in_partition(
        &store,
        &model,
        &field,
        scope_id,
        Uuid::now_v7(),
        TestNode::new(partition_match, None, "U", "Alpha in another partition"),
    )
    .await;
    let root = Uuid::now_v7();
    let match_parent = Uuid::now_v7();
    let match_child = Uuid::now_v7();
    let nonmatch_descendant = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(root, None, "U", "Projects"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(match_parent, Some(root), "U", "Alpha Document"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(match_child, Some(match_parent), "F", "ALPHA Notes"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        TestNode::new(
            nonmatch_descendant,
            Some(match_parent),
            "k",
            "Beta descendant",
        ),
    )
    .await;

    let output = store
        .search_ordered_tree_prefix(
            &metadata,
            OrderedTreeSearchInput {
                scope_id,
                tree_partition_id: scope_id,
                prefix: "aLpHa".to_owned(),
                match_limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        output
            .iter()
            .map(|node| (record_id(&node.record), node.is_match))
            .collect::<Vec<_>>(),
        vec![(root, false), (match_parent, true), (match_child, true)]
    );
    assert!(!output
        .iter()
        .any(|node| record_id(&node.record) == nonmatch_descendant));
    assert!(!output
        .iter()
        .any(|node| record_id(&node.record) == partition_match));
}

// AC-010/AC-014: real catalog and analyzed scale prove the matching expression index is used.
#[tokio::test]
async fn prefix_expression_index_matches_query_shape_and_avoids_seq_scan() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope_id = create_workspace(&store).await;
    let model = create_model(&store, scope_id, "index").await;
    let (model, field) = add_search_field(&store, &model).await;

    let index_definitions: Vec<String> = sqlx::query_scalar(
        "select indexdef from pg_indexes where schemaname = current_schema() and tablename = $1",
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert!(index_definitions.iter().any(|definition| {
        definition.contains(&format!(
            "lower({}) COLLATE \"C\"",
            field.physical_column_name
        )) && definition.contains("text_pattern_ops")
            && definition.contains("scope_id")
            && definition.contains("tree_partition_id")
    }));

    sqlx::query(&format!(
        r#"
        insert into "{}" (id, scope_id, tree_partition_id, sibling_rank, "{}")
        select gen_random_uuid(), $1, $1, lpad(series::text, 8, '0') || 'U',
               case when series = 4242 then 'Needle-4242' else 'row-' || series::text end
        from generate_series(1, 5000) series
        "#,
        model.physical_table_name, field.physical_column_name
    ))
    .bind(scope_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(&format!("analyze \"{}\"", model.physical_table_name))
        .execute(store.pool())
        .await
        .unwrap();
    let plan: Vec<String> = sqlx::query_scalar(&format!(
        "explain (analyze, costs off, format text) select id from \"{}\" where scope_id = $1 and tree_partition_id = $1 and (lower(\"{}\") collate \"C\") like lower($2) escape E'\\\\'",
        model.physical_table_name, field.physical_column_name
    ))
    .bind(scope_id)
    .bind("needle-4242%")
    .fetch_all(store.pool())
    .await
    .unwrap();
    let plan = plan.join("\n");
    assert!(!plan.contains("Seq Scan"), "{plan}");
    assert!(
        plan.contains("Index Scan") || plan.contains("Bitmap Index Scan"),
        "{plan}"
    );
}
