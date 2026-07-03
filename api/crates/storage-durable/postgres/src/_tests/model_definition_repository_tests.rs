use control_plane::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, CreateScopeDataModelGrantInput,
    ModelDefinitionRepository, UpdateModelDefinitionStatusInput, UpdateScopeDataModelGrantInput,
};
use domain::{
    DataModelProtection, DataModelScopeKind, DataModelSourceKind, DataModelStatus, ModelFieldKind,
    ScopeDataModelPermissionProfile, SYSTEM_SCOPE_ID,
};
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const BUILTIN_SYSTEM_TABLE_CONTRACT_METADATA_SQL: &str = include_str!(
    "../../migrations/20260630104000_preserve_builtin_system_table_contract_metadata.sql"
);

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

async fn seed_data_source_workspace(
    store: &PgControlPlaneStore,
    workspace_name: &str,
    provider_code: &str,
) -> (Uuid, Uuid, Uuid) {
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, workspace_name)
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
    control_plane::ports::PluginRepository::upsert_installation(
        store,
        &control_plane::ports::UpsertPluginInstallationInput {
            installation_id,
            provider_code: provider_code.into(),
            plugin_id: format!("{provider_code}@builtin"),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.data_source/v1".into(),
            protocol: "builtin".into(),
            display_name: "Main Source".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            artifact_status: domain::PluginArtifactStatus::Ready,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: format!("/tmp/{provider_code}"),
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

    (workspace.id, actor.id, installation_id)
}

async fn seed_data_source_instance(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    installation_id: Uuid,
    source_code: &str,
    display_name: &str,
) -> Uuid {
    let data_source_instance_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into data_source_instances (
            id, workspace_id, installation_id, source_code, display_name, status,
            config_json, metadata_json, created_by
        ) values ($1, $2, $3, $4, $5, 'ready', '{}', '{}', $6)
        "#,
    )
    .bind(data_source_instance_id)
    .bind(workspace_id)
    .bind(installation_id)
    .bind(source_code)
    .bind(display_name)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    data_source_instance_id
}

#[tokio::test]
async fn model_definition_repository_creates_scope_bound_metadata_without_publish_state() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    let workspace_name = format!("Core Workspace {}", workspace_id.simple());
    let code = format!("orders_{}", Uuid::now_v7().simple());
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
            code: code.clone(),
            title: "Orders".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    assert_eq!(created.scope_kind, DataModelScopeKind::Workspace);
    assert_eq!(created.scope_id, workspace_id);
    assert_eq!(created.code, code);
    assert_eq!(created.title, "Orders");
    assert!(created.physical_table_name.starts_with("rtm_workspace_"));
    let system_fields: Vec<_> = created
        .fields
        .iter()
        .map(|field| {
            (
                field.code.as_str(),
                field.physical_column_name.as_str(),
                field.field_kind,
                field.is_system,
                field.is_writable,
            )
        })
        .collect();
    assert_eq!(
        system_fields,
        vec![
            ("id", "id", ModelFieldKind::String, true, false),
            (
                "created_by",
                "created_by",
                ModelFieldKind::String,
                true,
                false
            ),
            (
                "updated_by",
                "updated_by",
                ModelFieldKind::String,
                true,
                false
            ),
            (
                "created_at",
                "created_at",
                ModelFieldKind::Datetime,
                true,
                false
            ),
            (
                "updated_at",
                "updated_at",
                ModelFieldKind::Datetime,
                true,
                false
            ),
        ]
    );

    let system_created = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            code: format!("system_{}", Uuid::now_v7().simple()),
            title: "System Orders".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    assert_eq!(system_created.scope_kind, DataModelScopeKind::System);
    assert_eq!(system_created.scope_id, SYSTEM_SCOPE_ID);
    assert!(system_created
        .physical_table_name
        .starts_with("rtm_system_"));
}

#[tokio::test]
async fn model_definition_repository_binds_core_system_models_to_registered_tables() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    for code in ["attachments", "users", "roles"] {
        let created = ModelDefinitionRepository::create_model_definition(
            &store,
            &CreateModelDefinitionInput {
                actor_user_id: Uuid::nil(),
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_source_instance_id: None,
                source_kind: DataModelSourceKind::MainSource,
                external_resource_key: None,
                external_table_id: None,
                external_capability_snapshot: None,
                code: code.into(),
                title: code.into(),
                status: DataModelStatus::Published,
                protection: DataModelProtection {
                    owner_kind: domain::DataModelOwnerKind::Core,
                    owner_id: None,
                    is_protected: true,
                },
            },
        )
        .await
        .unwrap();

        assert_eq!(created.physical_table_name, code);
        assert!(!created.physical_table_name.starts_with("rtm_system_"));
        assert!(created.fields.is_empty());
    }
}

#[tokio::test]
async fn builtin_system_table_contract_migration_preserves_metadata_and_user_fields() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            code: "attachments".into(),
            title: "Custom attachments".into(),
            status: DataModelStatus::Draft,
            protection: DataModelProtection {
                owner_kind: domain::DataModelOwnerKind::Core,
                owner_id: None,
                is_protected: true,
            },
        },
    )
    .await
    .unwrap();
    let filename_field_id = Uuid::now_v7();
    let custom_field_id = Uuid::now_v7();
    let grant_id = Uuid::now_v7();

    sqlx::query(
        r#"
        update model_definitions
        set physical_table_name = 'broken_attachments'
        where id = $1
        "#,
    )
    .bind(model.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into model_fields (
            id, data_model_id, scope_id, code, title, physical_column_name,
            external_field_key, field_kind, is_system, is_writable, is_required,
            is_unique, default_value, display_interface, display_options,
            relation_target_model_id, relation_options, sort_order,
            availability_status
        )
        values
            (
                $1, $3, $4, 'filename', 'Custom filename', 'broken_filename',
                null, 'string', false, true, false, false, null,
                'input', '{"placeholder":"custom"}'::jsonb, null, '{}'::jsonb,
                70, 'available'
            ),
            (
                $2, $3, $4, 'custom_tag', 'Custom tag', 'custom_tag',
                null, 'string', false, true, false, false, null,
                'input', '{}'::jsonb, null, '{}'::jsonb, 80, 'available'
            )
        "#,
    )
    .bind(filename_field_id)
    .bind(custom_field_id)
    .bind(model.id)
    .bind(SYSTEM_SCOPE_ID)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into scope_data_model_grants (
            id, scope_kind, scope_id, data_model_id, enabled, permission_profile
        )
        values ($1, 'system', $2, $3, false, 'owner')
        "#,
    )
    .bind(grant_id)
    .bind(SYSTEM_SCOPE_ID)
    .bind(model.id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::raw_sql(BUILTIN_SYSTEM_TABLE_CONTRACT_METADATA_SQL)
        .execute(store.pool())
        .await
        .unwrap();

    let (title, status, physical_table_name): (String, String, String) = sqlx::query_as(
        r#"
        select title, status, physical_table_name
        from model_definitions
        where id = $1
        "#,
    )
    .bind(model.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(title, "Custom attachments");
    assert_eq!(status, "published");
    assert_eq!(physical_table_name, "attachments");

    let (field_title, physical_column_name, is_required, display_interface, display_options): (
        String,
        String,
        bool,
        Option<String>,
        serde_json::Value,
    ) = sqlx::query_as(
        r#"
        select title, physical_column_name, is_required, display_interface, display_options
        from model_fields
        where id = $1
        "#,
    )
    .bind(filename_field_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(field_title, "Custom filename");
    assert_eq!(physical_column_name, "filename");
    assert!(is_required);
    assert_eq!(display_interface.as_deref(), Some("input"));
    assert_eq!(
        display_options,
        serde_json::json!({ "placeholder": "custom" })
    );

    let custom_field_exists: bool =
        sqlx::query_scalar("select exists(select 1 from model_fields where id = $1)")
            .bind(custom_field_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let attachment_scope_field_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from model_fields where data_model_id = $1 and code = 'scope_id')",
    )
    .bind(model.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(custom_field_exists);
    assert!(attachment_scope_field_exists);

    let (enabled, permission_profile): (bool, String) = sqlx::query_as(
        r#"
        select enabled, permission_profile
        from scope_data_model_grants
        where id = $1
        "#,
    )
    .bind(grant_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(!enabled);
    assert_eq!(permission_profile, "owner");
}

#[tokio::test]
async fn model_definition_repository_persists_status_owner_and_scope_grants() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(format!("Core Workspace {}", workspace_id.simple()))
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
            code: format!("customers_{}", Uuid::now_v7().simple()),
            title: "Customers".into(),
            status: DataModelStatus::Draft,
            protection: DataModelProtection {
                is_protected: true,
                ..Default::default()
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(created.status, DataModelStatus::Draft);
    assert!(created.protection.is_protected);

    let published = ModelDefinitionRepository::update_model_definition_status(
        &store,
        &UpdateModelDefinitionStatusInput {
            actor_user_id: Uuid::nil(),
            workspace_id,
            model_id: created.id,
            status: DataModelStatus::Published,
        },
    )
    .await
    .unwrap();

    assert_eq!(published.status, DataModelStatus::Published);

    let system_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            code: format!("system_customers_{}", Uuid::now_v7().simple()),
            title: "System Customers".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let grant_id = Uuid::now_v7();
    let grant = ModelDefinitionRepository::create_scope_data_model_grant(
        &store,
        &CreateScopeDataModelGrantInput {
            grant_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_model_id: system_model.id,
            enabled: true,
            permission_profile: ScopeDataModelPermissionProfile::ScopeAll,
            created_by: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(grant.id, grant_id);
    assert_eq!(
        grant.permission_profile,
        ScopeDataModelPermissionProfile::ScopeAll
    );

    let grants = ModelDefinitionRepository::list_scope_data_model_grants(
        &store,
        DataModelScopeKind::Workspace,
        workspace_id,
    )
    .await
    .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].data_model_id, system_model.id);
}

#[tokio::test]
async fn model_definition_repository_rejects_scope_grant_for_workspace_model() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(format!("Grant Workspace {}", workspace_id.simple()))
    .execute(store.pool())
    .await
    .unwrap();

    let workspace_model = ModelDefinitionRepository::create_model_definition(
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
            code: format!("workspace_grant_{}", Uuid::now_v7().simple()),
            title: "Workspace Grant Model".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let created = ModelDefinitionRepository::create_scope_data_model_grant(
        &store,
        &CreateScopeDataModelGrantInput {
            grant_id: Uuid::now_v7(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_model_id: workspace_model.id,
            enabled: true,
            permission_profile: ScopeDataModelPermissionProfile::ScopeAll,
            created_by: None,
        },
    )
    .await;
    let error = created.unwrap_err();
    assert!(error.to_string().contains("model_definition"));
}

#[tokio::test]
async fn model_definition_repository_rejects_scope_grant_update_for_workspace_model() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(format!("Grant Update Workspace {}", workspace_id.simple()))
    .execute(store.pool())
    .await
    .unwrap();

    let workspace_model = ModelDefinitionRepository::create_model_definition(
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
            code: format!("workspace_grant_update_{}", Uuid::now_v7().simple()),
            title: "Workspace Grant Update Model".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let updated = ModelDefinitionRepository::update_scope_data_model_grant(
        &store,
        &UpdateScopeDataModelGrantInput {
            data_model_id: workspace_model.id,
            grant_id: Uuid::now_v7(),
            enabled: false,
            permission_profile: ScopeDataModelPermissionProfile::ScopeAll,
        },
    )
    .await;
    let error = updated.unwrap_err();
    assert!(error.to_string().contains("model_definition"));
}

#[tokio::test]
async fn model_definition_repository_blocks_duplicate_code_inside_same_data_source() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (workspace_id, actor_user_id, installation_id) =
        seed_data_source_workspace(&store, "duplicate-code-workspace", "main_source").await;
    let data_source_instance_id = seed_data_source_instance(
        &store,
        workspace_id,
        actor_user_id,
        installation_id,
        "main_source",
        "Main Source",
    )
    .await;

    let code = format!("orders_{}", Uuid::now_v7().simple());
    let input = CreateModelDefinitionInput {
        actor_user_id,
        scope_kind: DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: Some(data_source_instance_id),
        source_kind: DataModelSourceKind::ExternalSource,
        external_resource_key: Some("orders".into()),
        external_table_id: None,
        external_capability_snapshot: None,
        code,
        title: "Orders".into(),
        status: DataModelStatus::Published,
        protection: DataModelProtection::default(),
    };
    ModelDefinitionRepository::create_model_definition(&store, &input)
        .await
        .unwrap();

    let duplicate = ModelDefinitionRepository::create_model_definition(&store, &input).await;
    assert!(duplicate.is_err());
}

#[tokio::test]
async fn model_definition_repository_blocks_duplicate_code_inside_main_source() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(format!("Main Source Workspace {}", workspace_id.simple()))
    .execute(store.pool())
    .await
    .unwrap();

    let code = format!("orders_{}", Uuid::now_v7().simple());
    let input = CreateModelDefinitionInput {
        actor_user_id: Uuid::nil(),
        scope_kind: DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        code,
        title: "Orders".into(),
        status: DataModelStatus::Published,
        protection: DataModelProtection::default(),
    };
    ModelDefinitionRepository::create_model_definition(&store, &input)
        .await
        .unwrap();

    let duplicate = ModelDefinitionRepository::create_model_definition(&store, &input).await;
    assert!(duplicate.is_err());
}

#[tokio::test]
async fn model_definition_repository_allows_duplicate_code_across_data_sources_in_same_workspace() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (workspace_id, actor_user_id, installation_id) =
        seed_data_source_workspace(&store, "duplicate-code-across-sources", "main_source").await;
    let first_data_source_id = seed_data_source_instance(
        &store,
        workspace_id,
        actor_user_id,
        installation_id,
        "main_source",
        "Main Source",
    )
    .await;
    let second_data_source_id = seed_data_source_instance(
        &store,
        workspace_id,
        actor_user_id,
        installation_id,
        "main_source",
        "Secondary Source",
    )
    .await;

    let code = format!("orders_{}", Uuid::now_v7().simple());
    let first = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(first_data_source_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("orders".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            code: code.clone(),
            title: "Orders".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let second = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(second_data_source_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("orders".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            code: code.clone(),
            title: "Orders Copy".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    assert_eq!(first.code, code);
    assert_eq!(second.code, code);
    assert_ne!(
        first.data_source_instance_id,
        second.data_source_instance_id
    );
}

#[tokio::test]
async fn model_definition_repository_rejects_workspace_model_with_foreign_data_source() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (workspace_id, actor_user_id, _installation_id) =
        seed_data_source_workspace(&store, "model-source-current-workspace", "current_source")
            .await;
    let (foreign_workspace_id, foreign_actor_user_id, foreign_installation_id) =
        seed_data_source_workspace(&store, "model-source-foreign-workspace", "foreign_source")
            .await;
    let foreign_data_source_id = seed_data_source_instance(
        &store,
        foreign_workspace_id,
        foreign_actor_user_id,
        foreign_installation_id,
        "foreign_source",
        "Foreign Source",
    )
    .await;

    let created = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(foreign_data_source_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("orders".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            code: format!("orders_{}", Uuid::now_v7().simple()),
            title: "Orders".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await;

    assert!(created.is_err());
}

#[tokio::test]
async fn model_definition_repository_reads_data_source_defaults_only_inside_workspace() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (workspace_id, _actor_user_id, _installation_id) =
        seed_data_source_workspace(&store, "defaults-current-workspace", "current_source").await;
    let (foreign_workspace_id, foreign_actor_user_id, foreign_installation_id) =
        seed_data_source_workspace(&store, "defaults-foreign-workspace", "foreign_source").await;
    let foreign_data_source_id = seed_data_source_instance(
        &store,
        foreign_workspace_id,
        foreign_actor_user_id,
        foreign_installation_id,
        "foreign_source",
        "Foreign Source",
    )
    .await;

    let visible = ModelDefinitionRepository::get_data_source_defaults(
        &store,
        foreign_workspace_id,
        foreign_data_source_id,
    )
    .await
    .unwrap();
    assert_eq!(visible.data_model_status, DataModelStatus::Published);

    let foreign = ModelDefinitionRepository::get_data_source_defaults(
        &store,
        workspace_id,
        foreign_data_source_id,
    )
    .await;
    assert!(foreign.is_err());
}

#[tokio::test]
async fn model_definition_repository_deletes_external_source_field_without_local_ddl() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (workspace_id, actor_user_id, installation_id) =
        seed_data_source_workspace(&store, "external-mapping-workspace", "external_crm").await;
    let data_source_instance_id = seed_data_source_instance(
        &store,
        workspace_id,
        actor_user_id,
        installation_id,
        "external_crm",
        "External CRM",
    )
    .await;

    let created = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(data_source_instance_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("crm.contacts".into()),
            external_table_id: Some("crm.contacts".into()),
            external_capability_snapshot: Some(serde_json::json!({
            "supports_owner_filter": true,
            "supports_scope_filter": true,
                "supports_write": false
            })),
            code: format!("external_contacts_{}", Uuid::now_v7().simple()),
            title: "External Contacts".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let created_table_count: i64 =
        sqlx::query_scalar("select count(*) from information_schema.tables where table_name = $1")
            .bind(&created.physical_table_name)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(created.source_kind, DataModelSourceKind::ExternalSource);
    assert_eq!(
        created.external_resource_key.as_deref(),
        Some("crm.contacts")
    );
    assert_eq!(created.external_table_id.as_deref(), Some("crm.contacts"));
    assert_eq!(created_table_count, 0);

    let field = ModelDefinitionRepository::add_model_field(
        &store,
        &AddModelFieldInput {
            actor_user_id,
            model_id: created.id,
            physical_column_name: None,
            external_field_key: Some("properties.email".into()),
            code: "email".into(),
            title: "Email".into(),
            description: None,
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            apply_physical_schema: true,
            is_required: true,
            api_required: true,
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

    let column_count: i64 = sqlx::query_scalar(
        "select count(*) from information_schema.columns where table_name = $1 and column_name = $2",
    )
    .bind(&created.physical_table_name)
    .bind(&field.physical_column_name)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        field.external_field_key.as_deref(),
        Some("properties.email")
    );
    assert_eq!(column_count, 0);

    let reloaded =
        ModelDefinitionRepository::get_model_definition(&store, workspace_id, created.id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(reloaded.source_kind, DataModelSourceKind::ExternalSource);
    assert_eq!(
        reloaded.external_resource_key.as_deref(),
        Some("crm.contacts")
    );
    assert_eq!(reloaded.external_table_id.as_deref(), Some("crm.contacts"));
    assert_eq!(
        reloaded.external_capability_snapshot,
        Some(serde_json::json!({
            "supports_owner_filter": true,
            "supports_scope_filter": true,
            "supports_write": false
        }))
    );
    assert_eq!(reloaded.fields.len(), 1);
    assert_eq!(
        reloaded.fields[0].external_field_key.as_deref(),
        Some("properties.email")
    );

    ModelDefinitionRepository::delete_model_field(&store, actor_user_id, created.id, field.id)
        .await
        .unwrap();

    let reloaded_after_delete =
        ModelDefinitionRepository::get_model_definition(&store, workspace_id, created.id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(reloaded_after_delete.fields.len(), 0);
}

#[tokio::test]
async fn model_definition_repository_status_update_requires_visible_workspace() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = Uuid::now_v7();
    let foreign_workspace_id = Uuid::now_v7();
    let tenant_id = root_tenant_id(&store).await;
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(format!("Current Workspace {}", workspace_id.simple()))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(foreign_workspace_id)
    .bind(tenant_id)
    .bind(format!("Foreign Workspace {}", foreign_workspace_id.simple()))
    .execute(store.pool())
    .await
    .unwrap();

    let foreign_model = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: foreign_workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            code: format!("foreign_orders_{}", Uuid::now_v7().simple()),
            title: "Foreign Orders".into(),
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let update = ModelDefinitionRepository::update_model_definition_status(
        &store,
        &UpdateModelDefinitionStatusInput {
            actor_user_id: Uuid::nil(),
            workspace_id,
            model_id: foreign_model.id,
            status: DataModelStatus::Draft,
        },
    )
    .await;
    assert!(update.is_err());

    let stored = ModelDefinitionRepository::get_model_definition(
        &store,
        foreign_workspace_id,
        foreign_model.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(stored.status, DataModelStatus::Published);
}
