use super::*;
use control_plane_contracts::ports::AddModelFieldInput;
use domain::ModelFieldKind;
use serde_json::{json, Value};
use storage_durable::{
    runtime_model_availability::RuntimeModelError, runtime_record_repository::*,
};

async fn add_field(store: &PgControlPlaneStore, model_id: Uuid, code: &str, target: Option<Uuid>) {
    store
        .add_model_field(&AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id,
            physical_column_name: None,
            external_field_key: None,
            code: code.into(),
            title: code.into(),
            description: None,
            field_kind: if target.is_some() {
                ModelFieldKind::ManyToMany
            } else {
                ModelFieldKind::String
            },
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: target,
            relation_options: json!({}),
        })
        .await
        .unwrap();
}

async fn general(
    store: &PgControlPlaneStore,
    scope: Uuid,
    code: &str,
) -> domain::ModelDefinitionRecord {
    store
        .create_model_definition(&CreateModelDefinitionInput {
            bound_storage_id: None,
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: scope,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: "core".into(),
            template_code: "general".into(),
            template_version: "v1".into(),
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: code.into(),
            title: code.into(),
            description: None,
        })
        .await
        .unwrap()
}

async fn refreshed(store: &PgControlPlaneStore, id: Uuid) -> ModelMetadata {
    runtime_metadata(
        &store
            .get_model_definition(Uuid::nil(), id)
            .await
            .unwrap()
            .unwrap(),
    )
}

fn list_query(scope: Uuid, expand: bool) -> RuntimeListQuery {
    RuntimeListQuery {
        scope_id: Some(scope),
        owner_user_id: None,
        filter: domain::ResourceFilterExpr::All(vec![]),
        sorts: vec![],
        expand_relations: if expand { vec!["tags".into()] } else { vec![] },
        page: 1,
        page_size: 100,
    }
}

#[tokio::test]
async fn many_to_many_general_roundtrip_replacement_atomicity_and_scope() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let tags = runtime_metadata(&general(&store, scope, "tags").await);
    let articles = general(&store, scope, "articles").await;
    add_field(&store, articles.id, "title", None).await;
    add_field(&store, articles.id, "tags", Some(tags.model_id)).await;
    let articles = refreshed(&store, articles.id).await;
    let first = store
        .create_record(&tags, Uuid::nil(), scope, json!({}))
        .await
        .unwrap()["id"]
        .clone();
    let second = store
        .create_record(&tags, Uuid::nil(), scope, json!({}))
        .await
        .unwrap()["id"]
        .clone();
    let foreign = store
        .create_record(&tags, Uuid::nil(), Uuid::now_v7(), json!({}))
        .await
        .unwrap()["id"]
        .clone();
    let created = store
        .create_record(
            &articles,
            Uuid::nil(),
            scope,
            json!({"title":"original", "tags":[first,second,first]}),
        )
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["tags"].as_array().unwrap().len(), 2);
    let expanded = store
        .list_records(&articles, list_query(scope, true))
        .await
        .unwrap();
    assert_eq!(expanded.items[0]["tags"].as_array().unwrap().len(), 2);
    assert!(expanded.items[0]["tags"][0]["id"].is_string());
    assert_eq!(
        store
            .list_records(&articles, list_query(scope, false))
            .await
            .unwrap()
            .items[0]["tags"],
        created["tags"]
    );
    for invalid in [
        Value::Null,
        json!("bad"),
        json!(["bad"]),
        json!([Uuid::now_v7()]),
        json!([first, foreign]),
    ] {
        let error = store
            .update_record(
                &articles,
                Uuid::nil(),
                Some(scope),
                None,
                id,
                json!({"title":"must rollback", "tags":invalid}),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<RuntimeModelError>(),
            Some(&RuntimeModelError::InvalidOperationField("tags".into()))
        );
        let reread = store
            .get_record(&articles, Some(scope), None, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reread["title"], "original");
        assert_eq!(reread["tags"], created["tags"]);
    }
    assert!(store
        .create_record(
            &articles,
            Uuid::nil(),
            scope,
            json!({"title":"rollback create", "tags":[foreign]})
        )
        .await
        .is_err());
    assert_eq!(
        store
            .list_records(&articles, list_query(scope, false))
            .await
            .unwrap()
            .total,
        1
    );
    let replaced = store
        .update_record(
            &articles,
            Uuid::nil(),
            Some(scope),
            None,
            id,
            json!({"tags":[second]}),
        )
        .await
        .unwrap();
    assert_eq!(replaced["tags"], json!([second]));
    let preserved = store
        .update_record(
            &articles,
            Uuid::nil(),
            Some(scope),
            None,
            id,
            json!({"title":"changed"}),
        )
        .await
        .unwrap();
    assert_eq!(preserved["tags"], json!([second]));
    // Wrong source scope must not alter the association set.
    assert!(store
        .update_record(
            &articles,
            Uuid::nil(),
            Some(Uuid::now_v7()),
            None,
            id,
            json!({"tags":[]})
        )
        .await
        .is_err());
    assert_eq!(
        store
            .get_record(&articles, Some(scope), None, id)
            .await
            .unwrap()
            .unwrap()["tags"],
        json!([second])
    );
    store
        .delete_record(&tags, Some(scope), None, second.as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(
        store
            .get_record(&articles, Some(scope), None, id)
            .await
            .unwrap()
            .unwrap()["tags"],
        json!([])
    );
    store
        .update_record(
            &articles,
            Uuid::nil(),
            Some(scope),
            None,
            id,
            json!({"tags":[first]}),
        )
        .await
        .unwrap();
    let cleared = store
        .update_record(
            &articles,
            Uuid::nil(),
            Some(scope),
            None,
            id,
            json!({"tags":[]}),
        )
        .await
        .unwrap();
    assert_eq!(cleared["tags"], json!([]));
    let empty = store
        .create_record(&articles, Uuid::nil(), scope, json!({"title":"empty"}))
        .await
        .unwrap();
    assert_eq!(empty["tags"], json!([]));
    store
        .update_model_definition_status(
            &control_plane_contracts::ports::UpdateModelDefinitionStatusInput {
                actor_user_id: Uuid::nil(),
                workspace_id: scope,
                model_id: tags.model_id,
                status: domain::DataModelStatus::Disabled,
            },
        )
        .await
        .unwrap();
    // Disabling a target must not turn an otherwise valid source create/read into an error
    // after the source has committed. Expansion still enforces target availability.
    let disabled_target = store
        .create_record(&articles, Uuid::nil(), scope, json!({"tags":[first]}))
        .await
        .unwrap();
    assert_eq!(disabled_target["tags"], json!([first]));
    let error = store
        .list_records(&articles, list_query(scope, true))
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<RuntimeModelError>(),
        Some(RuntimeModelError::Disabled(_))
    ));
}

#[tokio::test]
async fn many_to_many_ordered_tree_create_query_and_failed_create_are_atomic() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let tags = runtime_metadata(&general(&store, scope, "tags").await);
    let tree = create_ordered_tree_model(&store, scope).await;
    add_field(&store, tree.id, "tags", Some(tags.model_id)).await;
    add_field(&store, tree.id, "title", None).await;
    let tree = refreshed(&store, tree.id).await;
    let tag = store
        .create_record(&tags, Uuid::nil(), scope, json!({}))
        .await
        .unwrap()["id"]
        .clone();
    let partition = Uuid::now_v7();
    let input = |payload| OrderedTreeCreateInput {
        actor_user_id: Uuid::nil(),
        scope_id: scope,
        tree_partition_id: partition,
        payload,
        position: OrderedTreeCreatePosition {
            parent_id: None,
            before_id: None,
            after_id: None,
        },
    };
    let node = store
        .create_ordered_tree_node(&tree, input(json!({"tags":[tag]})))
        .await
        .unwrap();
    assert!(store
        .create_ordered_tree_node(&tree, input(json!({"tags":[Uuid::now_v7()]})))
        .await
        .is_err());
    let roots = store
        .list_ordered_tree_roots(
            &tree,
            OrderedTreeBoundedListInput {
                scope_id: scope,
                tree_partition_id: partition,
                result_limit: 20,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(roots.items.len(), 1);
    assert_eq!(roots.items[0].record["tags"], json!([tag]));
    assert_eq!(
        store
            .get_record(&tree, Some(scope), None, &node.node_id.to_string())
            .await
            .unwrap()
            .unwrap()["tags"],
        json!([tag])
    );
    let mut child_input = input(json!({"title":"Child", "tags":[tag]}));
    child_input.position.parent_id = Some(node.node_id);
    let child = store
        .create_ordered_tree_node(&tree, child_input)
        .await
        .unwrap();
    let children = store
        .list_ordered_tree_children(
            &tree,
            OrderedTreeChildrenInput {
                scope_id: scope,
                tree_partition_id: partition,
                parent_id: node.node_id,
                result_limit: 20,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(children.items[0].record["tags"], json!([tag]));
    let ancestors = store
        .list_ordered_tree_ancestors(
            &tree,
            OrderedTreeNodeInput {
                scope_id: scope,
                tree_partition_id: partition,
                node_id: child.node_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(ancestors[0].record["tags"], json!([tag]));
    let descendants = store
        .list_ordered_tree_descendants(
            &tree,
            OrderedTreeDescendantsInput {
                scope_id: scope,
                tree_partition_id: partition,
                node_id: node.node_id,
                max_depth: None,
                result_limit: 20,
                include_path: false,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(descendants.items[0].record["tags"], json!([tag]));
    let search = store
        .search_ordered_tree_prefix(
            &tree,
            OrderedTreeSearchInput {
                scope_id: scope,
                tree_partition_id: partition,
                prefix: "Chi".into(),
                match_limit: 20,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(search.items.len(), 2);
    assert!(search
        .items
        .iter()
        .all(|item| item.record["tags"] == json!([tag])));
}

#[tokio::test]
async fn many_to_many_concurrent_replacement_and_owner_visibility() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let owner = Uuid::now_v7();
    sqlx::query("insert into users (id, account, email, password_hash, name, nickname, status) values ($1, 'relation-owner', 'relation-owner@example.test', 'hash', 'Owner', 'Owner', 'active')")
        .bind(owner).execute(store.pool()).await.unwrap();
    let tags = runtime_metadata(&general(&store, scope, "tags").await);
    let articles = general(&store, scope, "articles").await;
    add_field(&store, articles.id, "tags", Some(tags.model_id)).await;
    let articles = refreshed(&store, articles.id).await;
    let own = store
        .create_record(&tags, owner, scope, json!({}))
        .await
        .unwrap()["id"]
        .clone();
    let other = store
        .create_record(&tags, Uuid::nil(), scope, json!({}))
        .await
        .unwrap()["id"]
        .clone();
    let record = store
        .create_record(&articles, owner, scope, json!({"tags":[own,other]}))
        .await
        .unwrap();
    let id = record["id"].as_str().unwrap();
    let visible = store
        .get_record(&articles, Some(scope), Some(owner), id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(visible["tags"], json!([own]));
    let mut query = list_query(scope, true);
    query.owner_user_id = Some(owner);
    let visible = store.list_records(&articles, query).await.unwrap();
    assert_eq!(visible.items[0]["tags"].as_array().unwrap().len(), 1);
    assert_eq!(visible.items[0]["tags"][0]["id"], own);
    let error = store
        .update_record(
            &articles,
            owner,
            Some(scope),
            Some(owner),
            id,
            json!({"tags":[other]}),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<RuntimeModelError>(),
        Some(RuntimeModelError::InvalidOperationField(_))
    ));
    let (left, right) = tokio::join!(
        store.update_record(
            &articles,
            owner,
            Some(scope),
            None,
            id,
            json!({"tags":[own]})
        ),
        store.update_record(
            &articles,
            owner,
            Some(scope),
            None,
            id,
            json!({"tags":[other]})
        ),
    );
    left.unwrap();
    right.unwrap();
    let saved = store
        .get_record(&articles, Some(scope), None, id)
        .await
        .unwrap()
        .unwrap();
    assert!(saved["tags"] == json!([own]) || saved["tags"] == json!([other]));
    assert!(store
        .delete_record(&articles, Some(scope), None, id)
        .await
        .unwrap());
    let join = crate::physical_schema_repository::join_table_name(
        &articles.model_code,
        articles.model_id,
        &tags.model_code,
        tags.model_id,
    );
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "select count(*) from {}",
        crate::runtime_record_repository::quote_identifier(&join).unwrap()
    )))
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn many_to_many_transaction_storage_failure_releases_single_connection_before_health_write() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let model = runtime_metadata(&general(&store, scope, "missing_table").await);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(store.pool().connect_options().as_ref().clone())
        .await
        .unwrap();
    let single = PgControlPlaneStore::new(pool);
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "drop table {}",
        crate::runtime_record_repository::quote_identifier(&model.physical_table_name).unwrap()
    )))
    .execute(single.pool())
    .await
    .unwrap();
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        single.create_record(&model, Uuid::nil(), scope, json!({})),
    )
    .await
    .expect("health mapping must not wait for its own transaction connection")
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<RuntimeModelError>(),
        Some(RuntimeModelError::Unavailable(_))
    ));
    let id = Uuid::now_v7().to_string();
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        single.update_record(
            &model,
            Uuid::nil(),
            Some(scope),
            None,
            &id,
            json!({"updated_at":"2026-10-24T00:00:00Z"}),
        ),
    )
    .await
    .expect("health mapping must not wait for its own transaction connection")
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<RuntimeModelError>(),
        Some(RuntimeModelError::Unavailable(_))
    ));
}
