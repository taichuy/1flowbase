use super::*;

#[tokio::test]
async fn add_field_returns_immediately_usable_metadata_without_publish_step() {
    let service = ModelDefinitionService::for_tests();
    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "orders".into(),
            title: "Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    let field = service
        .add_field(AddModelFieldCommand {
            actor_user_id: Uuid::nil(),
            model_id: created.id,
            code: "status".into(),
            title: "Status".into(),
            external_field_key: None,
            field_kind: ModelFieldKind::Enum,
            is_required: true,
            is_unique: false,
            default_value: Some(json!("draft")),
            display_interface: Some("select".into()),
            display_options: json!({ "options": ["draft", "paid"] }),
            relation_target_model_id: None,
            relation_options: json!({}),
        })
        .await
        .unwrap();

    assert_eq!(field.physical_column_name, "status");

    let updated = service.get_model(Uuid::nil(), created.id).await.unwrap();
    assert_eq!(updated.fields.len(), 1);
}

#[tokio::test]
async fn delete_model_requires_explicit_confirmation() {
    let service = ModelDefinitionService::for_tests();
    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "orders".into(),
            title: "Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    let error = service
        .delete_model(DeleteModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            model_id: created.id,
            confirmed: false,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("confirmation"));
}

#[tokio::test]
async fn delete_model_rejects_builtin_main_source_models() {
    let service = ModelDefinitionService::for_tests();

    for code in ["attachments", "users", "roles"] {
        let created = service
            .create_model(CreateModelDefinitionCommand {
                actor_user_id: Uuid::nil(),
                scope_kind: DataModelScopeKind::System,
                data_source_instance_id: None,
                external_resource_key: None,
                external_table_id: None,
                code: code.into(),
                title: code.into(),
                status: None,
            })
            .await
            .unwrap();

        let error = service
            .delete_model(DeleteModelDefinitionCommand {
                actor_user_id: Uuid::nil(),
                model_id: created.id,
                confirmed: true,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("builtin_data_model"));
        assert!(service
            .get_model(Uuid::nil(), created.id)
            .await
            .unwrap()
            .code
            .eq(code));
    }
}

#[tokio::test]
async fn create_system_model_uses_fixed_system_scope_id() {
    let service = ModelDefinitionService::for_tests();

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::System,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "system_orders".into(),
            title: "System Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    assert_eq!(created.scope_kind, DataModelScopeKind::System);
    assert_eq!(created.scope_id, SYSTEM_SCOPE_ID);
}

#[tokio::test]
async fn create_workspace_model_creates_system_model_and_workspace_grant() {
    let service = ModelDefinitionService::for_tests();

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "workspace_orders".into(),
            title: "Workspace Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    assert_eq!(created.scope_kind, DataModelScopeKind::System);
    assert_eq!(created.scope_id, SYSTEM_SCOPE_ID);

    let grant = service
        .load_runtime_scope_grant(
            &ActorContext::root(Uuid::nil(), Uuid::nil(), "root"),
            created.id,
        )
        .await
        .unwrap()
        .expect("workspace create path should persist a workspace grant");
    assert_eq!(grant.scope_kind, DataModelScopeKind::Workspace);
    assert_eq!(grant.scope_id, Uuid::nil());
    assert_eq!(
        grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::ScopeAll
    );
}

#[tokio::test]
async fn create_model_defaults_to_main_source_published_exposure() {
    let service = ModelDefinitionService::for_tests();

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "main_source_orders".into(),
            title: "Main Source Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    assert_eq!(created.status, DataModelStatus::Published);
    assert_eq!(
        created.api_exposure_status,
        ApiExposureStatus::ApiExposedReady
    );
    assert_eq!(created.data_source_instance_id, None);
}

#[tokio::test]
async fn create_model_inherits_main_source_defaults() {
    let repository =
        InMemoryModelDefinitionRepository::with_main_source_defaults(DataSourceDefaults {
            data_model_status: DataModelStatus::Draft,
            api_exposure_status: ApiExposureStatus::Draft,
        });
    let service = ModelDefinitionService::new(repository.clone());

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "main_source_draft_orders".into(),
            title: "Main Source Draft Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    assert_eq!(created.status, DataModelStatus::Draft);
    assert_eq!(created.api_exposure_status, ApiExposureStatus::Draft);
}
