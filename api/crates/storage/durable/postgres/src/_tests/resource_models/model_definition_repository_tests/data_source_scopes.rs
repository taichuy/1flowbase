use super::*;

#[tokio::test]
async fn model_definition_repository_blocks_duplicate_code_inside_same_data_source() {
    let pool = isolated_database().await.connect().await.unwrap();
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
        bound_storage_id: None,
        actor_user_id,
        scope_kind: DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: Some(data_source_instance_id),
        source_kind: DataModelSourceKind::ExternalSource,
        external_resource_key: Some("orders".into()),
        external_table_id: None,
        external_capability_snapshot: None,
        template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
        template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
        template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
        code,
        title: "Orders".into(),
        description: None,
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
    let pool = isolated_database().await.connect().await.unwrap();
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
        bound_storage_id: None,
        actor_user_id: Uuid::nil(),
        scope_kind: DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
        template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
        template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
        code,
        title: "Orders".into(),
        description: None,
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
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let policy =
        crate::RuntimeTableNamePolicy::from_config(true, "rtm_workspace_[a-z0-9]{8}").unwrap();
    let store = PgControlPlaneStore::new(pool).with_runtime_table_name_policy(policy);
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
            bound_storage_id: None,
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(first_data_source_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("orders".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
            code: code.clone(),
            title: "Orders".into(),
            description: None,
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await
    .unwrap();

    let second = ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            bound_storage_id: None,
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(second_data_source_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("orders".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
            code: code.clone(),
            title: "Orders Copy".into(),
            description: None,
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
    assert_ne!(first.physical_table_name, second.physical_table_name);
}

#[tokio::test]
async fn ac_005_model_definition_repository_exhausts_regex_prefix_collisions() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let policy = crate::RuntimeTableNamePolicy::from_config(true, "fixed_prefix").unwrap();
    let store = PgControlPlaneStore::new(pool).with_runtime_table_name_policy(policy);
    let (workspace_id, actor_user_id, installation_id) =
        seed_data_source_workspace(&store, "regex-prefix-collision", "main_source").await;
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

    for data_source_instance_id in [first_data_source_id, second_data_source_id] {
        let result = ModelDefinitionRepository::create_model_definition(
            &store,
            &CreateModelDefinitionInput {
                bound_storage_id: None,
                actor_user_id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: workspace_id,
                data_source_instance_id: Some(data_source_instance_id),
                source_kind: DataModelSourceKind::ExternalSource,
                external_resource_key: Some("orders".into()),
                external_table_id: None,
                external_capability_snapshot: None,
                template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
                template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
                template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
                code: code.clone(),
                title: "Orders".into(),
                description: None,
                status: DataModelStatus::Published,
                protection: DataModelProtection::default(),
            },
        )
        .await;

        if data_source_instance_id == first_data_source_id {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().to_string().contains("after 8 attempts"));
        }
    }
}

#[tokio::test]
async fn model_definition_repository_rejects_workspace_model_with_foreign_data_source() {
    let pool = isolated_database().await.connect().await.unwrap();
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
            bound_storage_id: None,
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_source_instance_id: Some(foreign_data_source_id),
            source_kind: DataModelSourceKind::ExternalSource,
            external_resource_key: Some("orders".into()),
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
            code: format!("orders_{}", Uuid::now_v7().simple()),
            title: "Orders".into(),
            description: None,
            status: DataModelStatus::Published,
            protection: DataModelProtection::default(),
        },
    )
    .await;

    assert!(created.is_err());
}

#[tokio::test]
async fn model_definition_repository_reads_data_source_defaults_only_inside_workspace() {
    let pool = isolated_database().await.connect().await.unwrap();
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
    let pool = isolated_database().await.connect().await.unwrap();
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
            bound_storage_id: None,
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
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
            code: format!("external_contacts_{}", Uuid::now_v7().simple()),
            title: "External Contacts".into(),
            description: None,
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
    let pool = isolated_database().await.connect().await.unwrap();
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
            bound_storage_id: None,
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: foreign_workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
            code: format!("foreign_orders_{}", Uuid::now_v7().simple()),
            title: "Foreign Orders".into(),
            description: None,
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
