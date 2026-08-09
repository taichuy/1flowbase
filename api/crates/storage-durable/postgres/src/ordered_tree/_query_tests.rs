use control_plane::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, ModelDefinitionRepository,
};
use domain::{DataModelScopeKind, ModelFieldKind};
use runtime_core::{
    model_metadata::ModelMetadata,
    resource_descriptor::ResourceDescriptor,
    runtime_record_repository::{
        OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeDescendantsInput,
        OrderedTreeNodeInput, OrderedTreeQueryError, OrderedTreeQueryRepository,
        OrderedTreeSearchInput,
    },
};
use uuid::Uuid;

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

fn metadata(model: &domain::ModelDefinitionRecord) -> ModelMetadata {
    ModelMetadata {
        model_id: model.id,
        model_code: model.code.clone(),
        status: model.status,
        scope_kind: model.scope_kind,
        scope_id: model.scope_id,
        data_source_instance_id: model.data_source_instance_id,
        source_kind: model.source_kind,
        external_resource_key: model.external_resource_key.clone(),
        template_provider: model.template_provider.clone(),
        template_code: model.template_code.clone(),
        template_version: model.template_version.clone(),
        physical_table_name: model.physical_table_name.clone(),
        scope_column_name: "scope_id".to_owned(),
        fields: model.fields.clone(),
        record_capabilities: domain::data_model_capabilities(model).record,
        resource: ResourceDescriptor::runtime_model(&model.code, model.scope_kind),
    }
}

async fn insert_node(
    store: &PgControlPlaneStore,
    model: &domain::ModelDefinitionRecord,
    field: &domain::ModelFieldRecord,
    scope_id: Uuid,
    node_id: Uuid,
    parent_id: Option<Uuid>,
    sibling_rank: &str,
    title: &str,
) {
    sqlx::query(&format!(
        "insert into \"{}\" (id, scope_id, parent_id, sibling_rank, \"{}\") values ($1, $2, $3, $4, $5)",
        model.physical_table_name, field.physical_column_name
    ))
    .bind(node_id)
    .bind(scope_id)
    .bind(parent_id)
    .bind(sibling_rank)
    .bind(title)
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

    let root_a = Uuid::now_v7();
    let root_b = Uuid::now_v7();
    let child_a = Uuid::now_v7();
    let child_b = Uuid::now_v7();
    let grandchild = Uuid::now_v7();
    let great_grandchild = Uuid::now_v7();
    insert_node(
        &store, &model, &field, scope_id, root_b, None, "k", "Root B",
    )
    .await;
    insert_node(
        &store, &model, &field, scope_id, root_a, None, "F", "Root A",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        child_b,
        Some(root_a),
        "k",
        "Child B",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        child_a,
        Some(root_a),
        "F",
        "Child A",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        grandchild,
        Some(child_a),
        "U",
        "Grandchild",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        great_grandchild,
        Some(grandchild),
        "U",
        "Great Grandchild",
    )
    .await;
    let foreign_node = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        other_scope_id,
        foreign_node,
        None,
        "U",
        "Foreign",
    )
    .await;

    let roots = store
        .list_ordered_tree_roots(
            &metadata,
            OrderedTreeBoundedListInput {
                scope_id,
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
    let children = store
        .list_ordered_tree_children(
            &metadata,
            OrderedTreeChildrenInput {
                scope_id,
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
    let mut wide_ids = Vec::new();
    for index in 0..20 {
        let node_id = Uuid::now_v7();
        wide_ids.push(node_id);
        insert_node(
            &store,
            &model,
            &field,
            scope_id,
            node_id,
            Some(root_b),
            &format!("{index:04}U"),
            "Wide Child",
        )
        .await;
    }
    let bounded_wide = store
        .list_ordered_tree_children(
            &metadata,
            OrderedTreeChildrenInput {
                scope_id,
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

    let descendants = store
        .list_ordered_tree_descendants(
            &metadata,
            OrderedTreeDescendantsInput {
                scope_id,
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
    let root = Uuid::now_v7();
    let match_parent = Uuid::now_v7();
    let match_child = Uuid::now_v7();
    let nonmatch_descendant = Uuid::now_v7();
    insert_node(
        &store, &model, &field, scope_id, root, None, "U", "Projects",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        match_parent,
        Some(root),
        "U",
        "Alpha Document",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        match_child,
        Some(match_parent),
        "F",
        "ALPHA Notes",
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope_id,
        nonmatch_descendant,
        Some(match_parent),
        "k",
        "Beta descendant",
    )
    .await;

    let output = store
        .search_ordered_tree_prefix(
            &metadata,
            OrderedTreeSearchInput {
                scope_id,
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
    }));

    sqlx::query(&format!(
        r#"
        insert into "{}" (id, scope_id, sibling_rank, "{}")
        select gen_random_uuid(), $1, lpad(series::text, 8, '0') || 'U',
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
        "explain (analyze, costs off, format text) select id from \"{}\" where scope_id = $1 and (lower(\"{}\") collate \"C\") like lower($2) escape E'\\\\'",
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
