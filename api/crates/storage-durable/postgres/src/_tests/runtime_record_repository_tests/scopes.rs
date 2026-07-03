use super::*;

#[tokio::test]
async fn runtime_record_repository_scopes_dynamic_rows_without_workspace_row() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let default_scope_id = domain::DEFAULT_SCOPE_ID;
    let alternate_scope_id = Uuid::now_v7();
    assert_ne!(default_scope_id, domain::SYSTEM_SCOPE_ID);

    let workspace_count: i64 = sqlx::query_scalar("select count(*) from workspaces")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(workspace_count, 0);

    let model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::System,
            scope_id: domain::SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: "default_scope_orders".into(),
            title: "Default Scope Orders".into(),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "title".into(),
            title: "Title".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            api_required: true,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
        },
    )
    .await
    .unwrap();

    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_code == "default_scope_orders")
        .unwrap();
    assert_eq!(metadata.scope_kind, DataModelScopeKind::System);
    assert_eq!(metadata.scope_id, domain::SYSTEM_SCOPE_ID);
    assert_eq!(metadata.scope_column_name, "scope_id");

    let default_record = RuntimeRecordRepository::create_record(
        &store,
        &metadata,
        Uuid::nil(),
        default_scope_id,
        json!({ "title": "default-scope" }),
    )
    .await
    .unwrap();
    let alternate_record = RuntimeRecordRepository::create_record(
        &store,
        &metadata,
        Uuid::nil(),
        alternate_scope_id,
        json!({ "title": "alternate-scope" }),
    )
    .await
    .unwrap();
    let default_record_id = default_record["id"].as_str().unwrap().to_string();
    let alternate_record_id = alternate_record["id"].as_str().unwrap().to_string();

    let default_list = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(default_scope_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(default_list.total, 1);
    assert_eq!(default_list.items[0]["title"], json!("default-scope"));

    let alternate_list = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(alternate_scope_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(alternate_list.total, 1);
    assert_eq!(alternate_list.items[0]["title"], json!("alternate-scope"));

    let cross_scope_get = RuntimeRecordRepository::get_record(
        &store,
        &metadata,
        Some(alternate_scope_id),
        None,
        &default_record_id,
    )
    .await
    .unwrap();
    assert!(cross_scope_get.is_none());

    let cross_scope_update = RuntimeRecordRepository::update_record(
        &store,
        &metadata,
        Uuid::nil(),
        Some(alternate_scope_id),
        None,
        &default_record_id,
        json!({ "title": "blocked" }),
    )
    .await;
    assert!(cross_scope_update.is_err());

    let cross_scope_delete = RuntimeRecordRepository::delete_record(
        &store,
        &metadata,
        Some(alternate_scope_id),
        None,
        &default_record_id,
    )
    .await
    .unwrap();
    assert!(!cross_scope_delete);

    let updated = RuntimeRecordRepository::update_record(
        &store,
        &metadata,
        Uuid::nil(),
        Some(default_scope_id),
        None,
        &default_record_id,
        json!({ "title": "default-scope-updated" }),
    )
    .await
    .unwrap();
    assert_eq!(updated["title"], json!("default-scope-updated"));

    let alternate_deleted = RuntimeRecordRepository::delete_record(
        &store,
        &metadata,
        Some(alternate_scope_id),
        None,
        &alternate_record_id,
    )
    .await
    .unwrap();
    assert!(alternate_deleted);
}

#[tokio::test]
async fn runtime_record_repository_uses_default_scope_id_without_workspace_row() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let future_scope_id = Uuid::now_v7();

    let missing_workspace_count: i64 =
        sqlx::query_scalar("select count(*)::bigint from workspaces where id = any($1)")
            .bind(vec![DEFAULT_SCOPE_ID, future_scope_id])
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(missing_workspace_count, 0);

    let model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: DEFAULT_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: "default_scope_orders".into(),
            title: "Default Scope Orders".into(),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "title".into(),
            title: "Title".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            api_required: true,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
        },
    )
    .await
    .unwrap();

    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| {
            model.model_code == "default_scope_orders" && model.scope_id == DEFAULT_SCOPE_ID
        })
        .unwrap();
    assert_eq!(metadata.scope_column_name, "scope_id");

    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = $1
        order by ordinal_position
        "#,
    )
    .bind(&metadata.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert!(columns.contains(&"scope_id".to_string()));
    assert!(!columns.contains(&"workspace_id".to_string()));

    let default_record = RuntimeRecordRepository::create_record(
        &store,
        &metadata,
        Uuid::nil(),
        DEFAULT_SCOPE_ID,
        json!({ "title": "default-scope" }),
    )
    .await
    .unwrap();
    let default_record_id = default_record["id"].as_str().unwrap().to_string();
    RuntimeRecordRepository::create_record(
        &store,
        &metadata,
        Uuid::nil(),
        future_scope_id,
        json!({ "title": "future-provider-scope" }),
    )
    .await
    .unwrap();

    let default_list = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(DEFAULT_SCOPE_ID),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(default_list.total, 1);
    assert_eq!(default_list.items[0]["title"], json!("default-scope"));

    let future_scope_list = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(future_scope_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(future_scope_list.total, 1);
    assert_eq!(
        future_scope_list.items[0]["title"],
        json!("future-provider-scope")
    );

    let blocked_cross_scope_get = RuntimeRecordRepository::get_record(
        &store,
        &metadata,
        Some(future_scope_id),
        None,
        &default_record_id,
    )
    .await
    .unwrap();
    assert!(blocked_cross_scope_get.is_none());

    let updated = RuntimeRecordRepository::update_record(
        &store,
        &metadata,
        Uuid::nil(),
        Some(DEFAULT_SCOPE_ID),
        None,
        &default_record_id,
        json!({ "title": "default-scope-updated" }),
    )
    .await
    .unwrap();
    assert_eq!(updated["title"], json!("default-scope-updated"));

    let deleted = RuntimeRecordRepository::delete_record(
        &store,
        &metadata,
        Some(DEFAULT_SCOPE_ID),
        None,
        &default_record_id,
    )
    .await
    .unwrap();
    assert!(deleted);
}
