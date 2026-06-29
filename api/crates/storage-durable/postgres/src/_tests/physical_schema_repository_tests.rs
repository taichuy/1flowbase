use control_plane::errors::ControlPlaneError;
use control_plane::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, ModelDefinitionRepository,
    UpdateModelDefinitionInput, UpdateModelFieldInput,
};
use domain::{DataModelScopeKind, ModelFieldKind};
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

async fn root_tenant_id(store: &PgControlPlaneStore) -> Uuid {
    sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn create_test_workspace(store: &PgControlPlaneStore) -> Uuid {
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(store).await;
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
    workspace_id
}

async fn create_main_source_model(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    code: &str,
    title: &str,
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
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: code.into(),
            title: title.into(),
        },
    )
    .await
    .unwrap()
}

async fn runtime_columns(store: &PgControlPlaneStore, table_name: &str) -> Vec<String> {
    sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_name = $1
        order by ordinal_position
        "#,
    )
    .bind(table_name)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

async fn runtime_index_defs(store: &PgControlPlaneStore, table_name: &str) -> Vec<String> {
    sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = current_schema()
          and tablename = $1
        "#,
    )
    .bind(table_name)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

async fn add_string_field(
    store: &PgControlPlaneStore,
    model_id: Uuid,
    code: &str,
) -> domain::ModelFieldRecord {
    ModelDefinitionRepository::add_model_field(
        store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id,
            external_field_key: None,
            code: code.into(),
            title: code.into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            physical_column_name: None,
            relation_options: serde_json::json!({}),
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn create_main_source_table_adds_platform_columns_and_scope_indexes() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = create_test_workspace(&store).await;

    let model = create_main_source_model(&store, workspace_id, "main_orders", "Main Orders").await;

    let columns = runtime_columns(&store, &model.physical_table_name).await;
    for expected in [
        "id",
        "scope_id",
        "created_by",
        "updated_by",
        "created_at",
        "updated_at",
    ] {
        assert!(columns.contains(&expected.to_string()));
    }
    assert!(!columns.contains(&"workspace_id".to_string()));
    assert!(!columns.contains(&"team_id".to_string()));
    assert!(!columns.contains(&"app_id".to_string()));

    let index_defs = runtime_index_defs(&store, &model.physical_table_name).await;
    assert!(index_defs
        .iter()
        .any(|def| def.contains("(scope_id, created_at, id)")));
    assert!(index_defs
        .iter()
        .any(|def| def.contains("(scope_id, created_by)")));

    let model_id = model.id.simple().to_string();
    assert!(index_defs
        .iter()
        .any(|def| def.contains(&format!("idx_scope_created_{model_id}"))));
    assert!(index_defs
        .iter()
        .any(|def| def.contains(&format!("idx_scope_creator_{model_id}"))));
}

#[tokio::test]
async fn update_model_and_field_keep_physical_names_immutable() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = create_test_workspace(&store).await;
    let model = create_main_source_model(&store, workspace_id, "immutable_orders", "Orders").await;
    let field = add_string_field(&store, model.id, "external_no").await;

    let updated_model = ModelDefinitionRepository::update_model_definition(
        &store,
        &UpdateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            title: "Renamed Orders".into(),
            external_table_id: None,
        },
    )
    .await
    .unwrap();
    let updated_field = ModelDefinitionRepository::update_model_field(
        &store,
        &UpdateModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            field_id: field.id,
            title: "Renamed External No".into(),
            description: None,
            is_required: true,
            is_unique: true,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: serde_json::json!({ "placeholder": "External No" }),
            relation_options: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    let stored_table_name: String =
        sqlx::query_scalar("select physical_table_name from model_definitions where id = $1")
            .bind(model.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let stored_column_name: String =
        sqlx::query_scalar("select physical_column_name from model_fields where id = $1")
            .bind(field.id)
            .fetch_one(store.pool())
            .await
            .unwrap();

    assert_eq!(updated_model.physical_table_name, model.physical_table_name);
    assert_eq!(stored_table_name, model.physical_table_name);
    assert_eq!(
        updated_field.physical_column_name,
        field.physical_column_name
    );
    assert_eq!(stored_column_name, field.physical_column_name);
}

#[tokio::test]
async fn delete_model_field_drops_dynamic_columns_but_rejects_platform_columns() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = create_test_workspace(&store).await;
    let model = create_main_source_model(&store, workspace_id, "delete_orders", "Orders").await;
    let dynamic_field = add_string_field(&store, model.id, "temporary_note").await;

    ModelDefinitionRepository::delete_model_field(&store, Uuid::nil(), model.id, dynamic_field.id)
        .await
        .unwrap();
    let columns_after_dynamic_delete = runtime_columns(&store, &model.physical_table_name).await;
    assert!(!columns_after_dynamic_delete.contains(&dynamic_field.physical_column_name));
    assert!(columns_after_dynamic_delete.contains(&"created_at".to_string()));

    let platform_field = model
        .fields
        .iter()
        .find(|field| field.physical_column_name == "created_at")
        .expect("main source model should expose created_at platform field metadata");
    assert!(platform_field.is_system);

    let delete_result = ModelDefinitionRepository::delete_model_field(
        &store,
        Uuid::nil(),
        model.id,
        platform_field.id,
    )
    .await;

    assert!(delete_result.is_err());
    let columns_after_platform_delete = runtime_columns(&store, &model.physical_table_name).await;
    assert!(columns_after_platform_delete.contains(&"created_at".to_string()));
}

#[tokio::test]
async fn add_model_field_rejects_codes_that_sanitize_to_platform_columns_without_metadata() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = create_test_workspace(&store).await;
    let model =
        create_main_source_model(&store, workspace_id, "reserved_field_orders", "Orders").await;

    let result = ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "created-at".into(),
            title: "Created At".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            relation_options: serde_json::json!({}),
        },
    )
    .await;

    let error = result.unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidInput("physical_column_name"))
    ));

    let field_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from model_fields where data_model_id = $1 and code = $2",
    )
    .bind(model.id)
    .bind("created-at")
    .fetch_one(store.pool())
    .await
    .unwrap();

    assert_eq!(field_count, 0);
}

#[tokio::test]
async fn add_scalar_field_creates_real_postgres_column_and_unique_index() {
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

    let model = ModelDefinitionRepository::create_model_definition(
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

    let field = ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: model.id,
            physical_column_name: None,
            external_field_key: None,
            code: "external_no".into(),
            title: "External No".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            is_unique: true,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            relation_options: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_name = $1
        order by ordinal_position
        "#,
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();

    let index_defs: Vec<String> = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = current_schema()
          and tablename = $1
        "#,
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert!(columns.contains(&field.physical_column_name));
    assert!(index_defs
        .iter()
        .any(|def| { def.contains("UNIQUE") && def.contains(&field.physical_column_name) }));
}

#[tokio::test]
async fn add_one_to_many_field_only_writes_metadata_without_creating_column() {
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

    let parent = ModelDefinitionRepository::create_model_definition(
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
    let child = ModelDefinitionRepository::create_model_definition(
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
            code: "order_items".into(),
            title: "Order Items".into(),
        },
    )
    .await
    .unwrap();

    let field = ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: parent.id,
            physical_column_name: None,
            external_field_key: None,
            code: "items".into(),
            title: "Items".into(),
            description: None,
            field_kind: ModelFieldKind::OneToMany,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: serde_json::json!({}),
            relation_target_model_id: Some(child.id),
            relation_options: serde_json::json!({ "mapped_by": "order_id" }),
        },
    )
    .await
    .unwrap();

    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_name = $1
        order by ordinal_position
        "#,
    )
    .bind(&parent.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(field.field_kind, ModelFieldKind::OneToMany);
    assert!(!columns.contains(&"items".to_string()));
}

#[tokio::test]
async fn add_many_to_many_field_creates_host_managed_join_table() {
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

    let left = ModelDefinitionRepository::create_model_definition(
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
    let right = ModelDefinitionRepository::create_model_definition(
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
            code: "tags".into(),
            title: "Tags".into(),
        },
    )
    .await
    .unwrap();

    ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id: Uuid::nil(),
            model_id: left.id,
            physical_column_name: None,
            external_field_key: None,
            code: "tags".into(),
            title: "Tags".into(),
            description: None,
            field_kind: ModelFieldKind::ManyToMany,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: serde_json::json!({}),
            relation_target_model_id: Some(right.id),
            relation_options: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    let tables: Vec<String> = sqlx::query_scalar(
        r#"
        select table_name
        from information_schema.tables
        where table_schema = current_schema()
        "#,
    )
    .fetch_all(store.pool())
    .await
    .unwrap();

    let join_table_name = tables
        .iter()
        .find(|name| name.starts_with("rtm_rel_"))
        .expect("many_to_many field should create a runtime join table");

    let columns = runtime_columns(&store, join_table_name).await;
    for expected in [
        "id",
        "scope_id",
        "created_by",
        "updated_by",
        "created_at",
        "updated_at",
    ] {
        assert!(columns.contains(&expected.to_string()));
    }

    let index_defs = runtime_index_defs(&store, join_table_name).await;
    assert!(index_defs
        .iter()
        .any(|def| def.contains("(scope_id, created_at, id)")));
}

#[tokio::test]
async fn create_runtime_model_table_always_uses_scope_id_column() {
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

    let model = ModelDefinitionRepository::create_model_definition(
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
            code: "orders_scope_column".into(),
            title: "Orders Scope Column".into(),
        },
    )
    .await
    .unwrap();

    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_name = $1
        order by ordinal_position
        "#,
    )
    .bind(&model.physical_table_name)
    .fetch_all(store.pool())
    .await
    .unwrap();

    let scoped_columns: Vec<String> = columns
        .into_iter()
        .filter(|column| column.ends_with("_id"))
        .collect();

    assert_eq!(scoped_columns, vec!["scope_id".to_string()]);
}
