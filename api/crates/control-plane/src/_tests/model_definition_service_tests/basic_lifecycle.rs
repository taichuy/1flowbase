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
            description: Some("Payment lifecycle state".into()),
            external_field_key: None,
            field_kind: ModelFieldKind::Enum,
            is_required: true,
            api_required: None,
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
    assert_eq!(
        field.description.as_deref(),
        Some("Payment lifecycle state")
    );

    let updated = service.get_model(Uuid::nil(), created.id).await.unwrap();
    assert_eq!(updated.fields.len(), 1);
    assert_eq!(
        updated.fields[0].description.as_deref(),
        Some("Payment lifecycle state")
    );
}

#[tokio::test]
async fn update_field_saves_user_owned_description_metadata() {
    let service = ModelDefinitionService::for_tests();
    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "orders_with_notes".into(),
            title: "Orders".into(),
            status: None,
        })
        .await
        .unwrap();
    let field = service
        .add_field(AddModelFieldCommand {
            actor_user_id: Uuid::nil(),
            model_id: created.id,
            code: "note".into(),
            title: "Note".into(),
            description: None,
            external_field_key: None,
            field_kind: ModelFieldKind::Text,
            is_required: false,
            api_required: None,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
        })
        .await
        .unwrap();

    let updated = service
        .update_field(UpdateModelFieldCommand {
            actor_user_id: Uuid::nil(),
            model_id: created.id,
            field_id: field.id,
            title: "Note".into(),
            description: Some("Operator-facing note".into()),
            is_required: false,
            api_required: None,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_options: json!({}),
        })
        .await
        .unwrap();

    assert_eq!(updated.description.as_deref(), Some("Operator-facing note"));
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
    for code in ["attachments", "users", "roles"] {
        let model_id = Uuid::now_v7();
        let model = ModelDefinitionRecord {
            id: model_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            code: code.into(),
            title: code.into(),
            physical_table_name: code.into(),
            acl_namespace: format!("state_model.{code}"),
            audit_namespace: format!("audit.state_model.{code}"),
            fields: vec![],
            availability_status: domain::MetadataAvailabilityStatus::Available,
            status: DataModelStatus::Published,
            protection: DataModelProtection {
                owner_kind: DataModelOwnerKind::Core,
                owner_id: None,
                is_protected: true,
            },
        };
        let repository = ScopedModelDefinitionRepository::new(ActorContext::root(
            Uuid::nil(),
            Uuid::nil(),
            "root",
        ))
        .with_model(model);
        let service = ModelDefinitionService::new(repository);

        let error = service
            .delete_model(DeleteModelDefinitionCommand {
                actor_user_id: Uuid::nil(),
                model_id,
                confirmed: true,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("builtin_data_model"));
        assert!(service
            .get_model(Uuid::nil(), model_id)
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
async fn create_workspace_model_uses_current_workspace_scope_and_grant() {
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

    assert_eq!(created.scope_kind, DataModelScopeKind::Workspace);
    assert_eq!(created.scope_id, Uuid::nil());
    assert!(created.physical_table_name.starts_with("rtm_workspace_"));

    for action in [
        runtime_core::runtime_acl::RuntimeDataAction::View,
        runtime_core::runtime_acl::RuntimeDataAction::Create,
        runtime_core::runtime_acl::RuntimeDataAction::Update,
        runtime_core::runtime_acl::RuntimeDataAction::Delete,
    ] {
        let grant = service
            .load_runtime_scope_grant(
                &ActorContext::root(Uuid::nil(), Uuid::nil(), "root"),
                created.id,
                action,
            )
            .await
            .unwrap()
            .expect("workspace owner grant should authorize every runtime CRUD action");
        assert_eq!(grant.scope_kind, DataModelScopeKind::Workspace);
        assert_eq!(grant.scope_id, Uuid::nil());
        assert_eq!(
            grant.permission_profile,
            domain::ScopeDataModelPermissionProfile::ScopeAll
        );
    }

    let persisted_grants = service
        .list_scope_grants(Uuid::nil(), created.id)
        .await
        .unwrap();
    assert_eq!(persisted_grants.len(), 1);
    let replayed = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: Uuid::nil(),
            data_model_id: created.id,
            enabled: true,
            permission_profile: "scope_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();
    assert_eq!(replayed.id, persisted_grants[0].id);
}

#[tokio::test]
async fn create_model_defaults_to_main_source_published_status() {
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
    assert_eq!(created.data_source_instance_id, None);
}

#[tokio::test]
async fn create_model_inherits_main_source_defaults() {
    let repository =
        InMemoryModelDefinitionRepository::with_main_source_defaults(DataSourceDefaults {
            data_model_status: DataModelStatus::Draft,
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
}
