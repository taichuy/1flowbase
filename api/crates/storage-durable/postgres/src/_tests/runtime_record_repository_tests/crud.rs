use super::*;

#[tokio::test]
async fn runtime_record_repository_supports_crud_filter_sort_and_relation_expansion() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    let workspace_name = format!("Core Workspace {}", workspace_id.simple());
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(&workspace_name)
    .execute(store.pool())
    .await
    .unwrap();

    let customer_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "customers".into(),
            title: "Customers".into(),
        },
    )
    .await
    .unwrap();
    let order_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "orders".into(),
            title: "Orders".into(),
        },
    )
    .await
    .unwrap();

    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: customer_model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "name".into(),
            title: "Name".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
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
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: order_model.id,
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
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: order_model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "status".into(),
            title: "Status".into(),
            description: None,
            field_kind: ModelFieldKind::Enum,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: Some("select".into()),
            display_options: json!({ "options": ["draft", "paid"] }),
            relation_target_model_id: None,
            relation_options: json!({}),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: order_model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "customer".into(),
            title: "Customer".into(),
            description: None,
            field_kind: ModelFieldKind::ManyToOne,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: Some("select".into()),
            display_options: json!({}),
            relation_target_model_id: Some(customer_model.id),
            relation_options: json!({}),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: customer_model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "orders".into(),
            title: "Orders".into(),
            description: None,
            field_kind: ModelFieldKind::OneToMany,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: Some(order_model.id),
            relation_options: json!({ "mapped_by": "customer" }),
        },
    )
    .await
    .unwrap();

    let mut metadata = store.list_runtime_model_metadata().await.unwrap();
    metadata.sort_by(|left, right| left.model_code.cmp(&right.model_code));
    let customer_metadata = metadata
        .iter()
        .find(|model| model.model_code == "customers" && model.scope_id == workspace_id)
        .unwrap()
        .clone();
    let order_metadata = metadata
        .iter()
        .find(|model| model.model_code == "orders" && model.scope_id == workspace_id)
        .unwrap()
        .clone();
    assert_eq!(customer_metadata.scope_column_name, "scope_id");
    assert_eq!(order_metadata.scope_column_name, "scope_id");

    let alice = RuntimeRecordRepository::create_record(
        &store,
        &customer_metadata,
        Uuid::nil(),
        workspace_id,
        json!({ "name": "Alice" }),
    )
    .await
    .unwrap();
    let bob = RuntimeRecordRepository::create_record(
        &store,
        &customer_metadata,
        Uuid::nil(),
        workspace_id,
        json!({ "name": "Bob" }),
    )
    .await
    .unwrap();

    let alice_id = alice["id"].as_str().unwrap().to_string();
    let bob_id = bob["id"].as_str().unwrap().to_string();

    let first = RuntimeRecordRepository::create_record(
        &store,
        &order_metadata,
        Uuid::nil(),
        workspace_id,
        json!({ "title": "A-001", "status": "draft", "customer": alice_id }),
    )
    .await
    .unwrap();
    let created = RuntimeRecordRepository::create_record(
        &store,
        &order_metadata,
        Uuid::nil(),
        workspace_id,
        json!({ "title": "A-002", "status": "paid", "customer": bob_id.clone() }),
    )
    .await
    .unwrap();

    let first_id = first["id"].as_str().unwrap().to_string();
    let order_id = created["id"].as_str().unwrap().to_string();

    let listed = RuntimeRecordRepository::list_records(
        &store,
        &order_metadata,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::Field {
                field: "status".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("paid"),
            },
            sorts: vec![RuntimeSortInput {
                field_code: "title".into(),
                direction: "desc".into(),
            }],
            expand_relations: vec!["customer".into()],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0]["title"], json!("A-002"));
    assert_eq!(listed.items[0]["customer"]["name"], json!("Bob"));

    let filtered_by_ast = RuntimeRecordRepository::list_records(
        &store,
        &order_metadata,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![
                domain::ResourceFilterExpr::Field {
                    field: "title".into(),
                    operator: domain::ResourceFilterOperator::Includes,
                    value: json!("A-00"),
                },
                domain::ResourceFilterExpr::Field {
                    field: "status".into(),
                    operator: domain::ResourceFilterOperator::In,
                    value: json!(["paid"]),
                },
            ]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(filtered_by_ast.total, 1);
    assert_eq!(filtered_by_ast.items[0]["title"], json!("A-002"));

    let fetched = RuntimeRecordRepository::get_record(
        &store,
        &order_metadata,
        Some(workspace_id),
        None,
        &first_id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(fetched["title"], json!("A-001"));

    let updated = RuntimeRecordRepository::update_record(
        &store,
        &order_metadata,
        Uuid::nil(),
        Some(workspace_id),
        None,
        &order_id,
        json!({ "title": "A-002X", "status": "paid", "customer": bob_id }),
    )
    .await
    .unwrap();
    assert_eq!(updated["title"], json!("A-002X"));

    let customers = RuntimeRecordRepository::list_records(
        &store,
        &customer_metadata,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec!["orders".into()],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    let alice_row = customers
        .items
        .iter()
        .find(|item| item["name"] == json!("Alice"))
        .unwrap();
    assert_eq!(alice_row["orders"].as_array().unwrap().len(), 1);

    let deleted = RuntimeRecordRepository::delete_record(
        &store,
        &order_metadata,
        Some(workspace_id),
        None,
        &order_id,
    )
    .await
    .unwrap();
    assert!(deleted);
}

#[tokio::test]
async fn runtime_record_repository_blocks_expanding_draft_relation_targets() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    insert_workspace(&store, workspace_id).await;

    let customer_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Draft,
            api_exposure_status: domain::ApiExposureStatus::Draft,
            protection: domain::DataModelProtection::default(),
            code: "draft_relation_customers".into(),
            title: "Draft Relation Customers".into(),
        },
    )
    .await
    .unwrap();
    let order_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "draft_relation_orders".into(),
            title: "Draft Relation Orders".into(),
        },
    )
    .await
    .unwrap();

    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: customer_model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "name".into(),
            title: "Name".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
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
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: order_model.id,
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
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: order_model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "customer".into(),
            title: "Customer".into(),
            description: None,
            field_kind: ModelFieldKind::ManyToOne,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: Some("select".into()),
            display_options: json!({}),
            relation_target_model_id: Some(customer_model.id),
            relation_options: json!({}),
        },
    )
    .await
    .unwrap();

    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let customer_metadata = metadata
        .iter()
        .find(|model| model.model_code == "draft_relation_customers")
        .unwrap()
        .clone();
    let order_metadata = metadata
        .iter()
        .find(|model| model.model_code == "draft_relation_orders")
        .unwrap()
        .clone();

    let customer = RuntimeRecordRepository::create_record(
        &store,
        &customer_metadata,
        Uuid::nil(),
        workspace_id,
        json!({ "name": "Draft Customer" }),
    )
    .await
    .unwrap();
    let customer_id = customer["id"].as_str().unwrap().to_string();
    RuntimeRecordRepository::create_record(
        &store,
        &order_metadata,
        Uuid::nil(),
        workspace_id,
        json!({ "title": "Published Order", "customer": customer_id }),
    )
    .await
    .unwrap();

    let error = RuntimeRecordRepository::list_records(
        &store,
        &order_metadata,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec!["customer".into()],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap_err();

    let model_error = error.downcast_ref::<RuntimeModelError>().unwrap();
    assert_eq!(
        model_error,
        &RuntimeModelError::not_published("draft_relation_customers")
    );
}

#[tokio::test]
async fn runtime_record_repository_enforces_owner_scope() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let owner_user_id = Uuid::now_v7();
    let other_user_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    let workspace_name = format!("Core Workspace {}", workspace_id.simple());

    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(&workspace_name)
    .execute(store.pool())
    .await
    .unwrap();
    insert_user(&store, owner_user_id, "owner-user").await;
    insert_user(&store, other_user_id, "other-user").await;

    let order_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "orders_acl".into(),
            title: "Orders ACL".into(),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: order_model.id,
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
        .find(|model| model.model_code == "orders_acl" && model.scope_id == workspace_id)
        .unwrap();
    assert_eq!(metadata.scope_column_name, "scope_id");

    let owner_record = RuntimeRecordRepository::create_record(
        &store,
        &metadata,
        owner_user_id,
        workspace_id,
        json!({ "title": "owner-record" }),
    )
    .await
    .unwrap();
    let other_record = RuntimeRecordRepository::create_record(
        &store,
        &metadata,
        other_user_id,
        workspace_id,
        json!({ "title": "other-record" }),
    )
    .await
    .unwrap();
    let owner_record_id = owner_record["id"].as_str().unwrap().to_string();
    let other_record_id = other_record["id"].as_str().unwrap().to_string();

    let own_list = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
            owner_user_id: Some(owner_user_id),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(own_list.total, 1);
    assert_eq!(own_list.items[0]["title"], json!("owner-record"));

    let own_get = RuntimeRecordRepository::get_record(
        &store,
        &metadata,
        Some(workspace_id),
        Some(owner_user_id),
        &owner_record_id,
    )
    .await
    .unwrap();
    assert!(own_get.is_some());
    let blocked_get = RuntimeRecordRepository::get_record(
        &store,
        &metadata,
        Some(workspace_id),
        Some(owner_user_id),
        &other_record_id,
    )
    .await
    .unwrap();
    assert!(blocked_get.is_none());

    let updated = RuntimeRecordRepository::update_record(
        &store,
        &metadata,
        owner_user_id,
        Some(workspace_id),
        Some(owner_user_id),
        &owner_record_id,
        json!({ "title": "owner-record-updated" }),
    )
    .await
    .unwrap();
    assert_eq!(updated["title"], json!("owner-record-updated"));

    let blocked_update = RuntimeRecordRepository::update_record(
        &store,
        &metadata,
        owner_user_id,
        Some(workspace_id),
        Some(owner_user_id),
        &other_record_id,
        json!({ "title": "blocked-update" }),
    )
    .await;
    assert!(blocked_update.is_err());

    let blocked_delete = RuntimeRecordRepository::delete_record(
        &store,
        &metadata,
        Some(workspace_id),
        Some(owner_user_id),
        &other_record_id,
    )
    .await
    .unwrap();
    assert!(!blocked_delete);

    let all_list = RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        RuntimeListQuery {
            scope_id: Some(workspace_id),
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
    assert_eq!(all_list.total, 2);

    let all_get = RuntimeRecordRepository::get_record(
        &store,
        &metadata,
        Some(workspace_id),
        None,
        &other_record_id,
    )
    .await
    .unwrap();
    assert!(all_get.is_some());

    let all_updated = RuntimeRecordRepository::update_record(
        &store,
        &metadata,
        owner_user_id,
        Some(workspace_id),
        None,
        &other_record_id,
        json!({ "title": "other-record-updated" }),
    )
    .await
    .unwrap();
    assert_eq!(all_updated["title"], json!("other-record-updated"));

    let all_deleted = RuntimeRecordRepository::delete_record(
        &store,
        &metadata,
        Some(workspace_id),
        None,
        &other_record_id,
    )
    .await
    .unwrap();
    assert!(all_deleted);
}
