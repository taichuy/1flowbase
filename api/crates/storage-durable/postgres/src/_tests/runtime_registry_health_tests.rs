use control_plane::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, ModelDefinitionRepository, PluginRepository,
    UpdateModelDefinitionStatusInput, UpsertPluginInstallationInput,
};
use domain::DataModelScopeKind;
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_external_runtime_model(
    store: &PgControlPlaneStore,
    model_code: String,
) -> domain::ModelDefinitionRecord {
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(
            tenant.id,
            &format!("External Runtime Health {}", Uuid::now_v7().simple()),
        )
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();
    let provider_code = format!("external_health_{}", Uuid::now_v7().simple());
    PluginRepository::upsert_installation(
        store,
        &UpsertPluginInstallationInput {
            installation_id,
            provider_code: provider_code.clone(),
            plugin_id: "external_health@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.data_source/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "External Health".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            artifact_status: domain::PluginArtifactStatus::Ready,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: "/tmp/external-health".into(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: serde_json::json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    let data_source_instance_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into data_source_instances (
            id, workspace_id, installation_id, source_code, display_name, status,
            config_json, metadata_json, created_by
        ) values ($1, $2, $3, $4, 'External Health', 'ready', '{}', '{}', $5)
        "#,
    )
    .bind(data_source_instance_id)
    .bind(workspace.id)
    .bind(installation_id)
    .bind(&provider_code)
    .bind(actor.id)
    .execute(store.pool())
    .await
    .unwrap();

    let model = ModelDefinitionRepository::create_model_definition(
        store,
        &CreateModelDefinitionInput {
            actor_user_id: actor.id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace.id,
            data_source_instance_id: Some(data_source_instance_id),
            source_kind: domain::DataModelSourceKind::ExternalSource,
            external_resource_key: Some("contacts".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: model_code,
            title: "External Contacts".into(),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::add_model_field(
        store,
        &AddModelFieldInput {
            actor_user_id: actor.id,
            model_id: model.id,
            physical_column_name: None,
            external_field_key: Some("email_address".into()),
            code: "email".into(),
            title: "Email".into(),
            description: None,
            field_kind: domain::ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            relation_options: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    ModelDefinitionRepository::get_model_definition(store, workspace.id, model.id)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn list_runtime_model_metadata_marks_model_unavailable_when_table_is_missing() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    let workspace_name = format!("Runtime Health {}", workspace_id.simple());
    let model_code = format!("orders_{}", Uuid::now_v7().simple());

    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(&workspace_name)
    .execute(store.pool())
    .await
    .unwrap();

    let created = ModelDefinitionRepository::create_model_definition(
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
            protection: domain::DataModelProtection::default(),
            code: model_code.clone(),
            title: "Orders".into(),
        },
    )
    .await
    .unwrap();

    let initial_metadata = store.list_runtime_model_metadata().await.unwrap();
    assert!(initial_metadata
        .iter()
        .any(|model| model.model_id == created.id));

    let drop_statement = format!("drop table if exists \"{}\"", created.physical_table_name);
    sqlx::query(&drop_statement)
        .execute(store.pool())
        .await
        .unwrap();

    let metadata = store.list_runtime_model_metadata().await.unwrap();

    assert!(!metadata.iter().any(|model| model.model_id == created.id));
}

#[tokio::test]
async fn list_runtime_model_metadata_keeps_valid_external_source_metadata_available_without_local_table(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let model_code = format!("external_valid_{}", Uuid::now_v7().simple());
    let model = seed_external_runtime_model(&store, model_code).await;

    let metadata = store.list_runtime_model_metadata().await.unwrap();

    let external = metadata
        .iter()
        .find(|candidate| candidate.model_id == model.id)
        .unwrap();
    assert_eq!(
        external.source_kind,
        domain::DataModelSourceKind::ExternalSource
    );
    assert_eq!(external.fields.len(), 1);
    assert_eq!(external.fields[0].code, "email");
}

#[tokio::test]
async fn list_runtime_model_metadata_marks_external_source_metadata_unavailable_without_instance_id(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let model = seed_external_runtime_model(
        &store,
        format!("external_missing_instance_{}", Uuid::now_v7().simple()),
    )
    .await;

    sqlx::query("update model_definitions set data_source_instance_id = null where id = $1")
        .bind(model.id)
        .execute(store.pool())
        .await
        .unwrap();

    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let model_status: String =
        sqlx::query_scalar("select availability_status from model_definitions where id = $1")
            .bind(model.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let field_statuses: Vec<String> =
        sqlx::query_scalar("select availability_status from model_fields where data_model_id = $1")
            .bind(model.id)
            .fetch_all(store.pool())
            .await
            .unwrap();

    assert!(!metadata
        .iter()
        .any(|candidate| candidate.model_id == model.id));
    assert_eq!(model_status, "unavailable");
    assert_eq!(field_statuses, vec!["unavailable".to_string()]);
}

#[tokio::test]
async fn list_runtime_model_metadata_marks_external_source_metadata_unavailable_without_resource_key(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let model = seed_external_runtime_model(
        &store,
        format!("external_missing_resource_{}", Uuid::now_v7().simple()),
    )
    .await;

    sqlx::query("update model_definitions set external_resource_key = '' where id = $1")
        .bind(model.id)
        .execute(store.pool())
        .await
        .unwrap();

    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let model_status: String =
        sqlx::query_scalar("select availability_status from model_definitions where id = $1")
            .bind(model.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let field_statuses: Vec<String> =
        sqlx::query_scalar("select availability_status from model_fields where data_model_id = $1")
            .bind(model.id)
            .fetch_all(store.pool())
            .await
            .unwrap();

    assert!(!metadata
        .iter()
        .any(|candidate| candidate.model_id == model.id));
    assert_eq!(model_status, "unavailable");
    assert_eq!(field_statuses, vec!["unavailable".to_string()]);
}

#[tokio::test]
async fn list_runtime_model_metadata_marks_external_source_metadata_unavailable_without_external_field_key(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let model = seed_external_runtime_model(
        &store,
        format!("external_missing_field_{}", Uuid::now_v7().simple()),
    )
    .await;

    sqlx::query("update model_fields set external_field_key = null where data_model_id = $1")
        .bind(model.id)
        .execute(store.pool())
        .await
        .unwrap();

    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let model_status: String =
        sqlx::query_scalar("select availability_status from model_definitions where id = $1")
            .bind(model.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let field_statuses: Vec<String> =
        sqlx::query_scalar("select availability_status from model_fields where data_model_id = $1")
            .bind(model.id)
            .fetch_all(store.pool())
            .await
            .unwrap();

    assert!(!metadata
        .iter()
        .any(|candidate| candidate.model_id == model.id));
    assert_eq!(model_status, "unavailable");
    assert_eq!(field_statuses, vec!["unavailable".to_string()]);
}

#[tokio::test]
async fn list_runtime_model_metadata_preserves_data_model_status() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    let workspace_name = format!("Runtime Status {}", workspace_id.simple());
    let model_suffix = Uuid::now_v7().simple().to_string();
    let model_code = format!("draft_{}", &model_suffix[..8]);

    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(&workspace_name)
    .execute(store.pool())
    .await
    .unwrap();

    let created = ModelDefinitionRepository::create_model_definition(
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
            protection: domain::DataModelProtection::default(),
            code: model_code.clone(),
            title: "Draft Orders".into(),
        },
    )
    .await
    .unwrap();
    ModelDefinitionRepository::update_model_definition_status(
        &store,
        &UpdateModelDefinitionStatusInput {
            actor_user_id: Uuid::nil(),
            workspace_id,
            model_id: created.id,
            status: domain::DataModelStatus::Draft,
        },
    )
    .await
    .unwrap();

    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_id == created.id)
        .unwrap();

    assert_eq!(metadata.status, domain::DataModelStatus::Draft);
}
