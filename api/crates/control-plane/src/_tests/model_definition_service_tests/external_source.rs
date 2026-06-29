use super::*;

#[tokio::test]
async fn external_model_missing_scope_filter_capability_keeps_published_exposure_visible() {
    let model_id = Uuid::now_v7();
    let mut model = model_in_workspace(model_id, Uuid::nil());
    model.data_source_instance_id = Some(Uuid::now_v7());
    model.source_kind = domain::DataModelSourceKind::ExternalSource;
    model.external_resource_key = Some("contacts".into());
    model.external_capability_snapshot = Some(json!({
        "supports_owner_filter": false,
        "supports_write": false
    }));
    let repository =
        ScopedModelDefinitionRepository::new(ActorContext::root(Uuid::nil(), Uuid::nil(), "root"))
            .with_model(model)
            .with_grant(scope_grant(
                Uuid::now_v7(),
                model_id,
                DataModelScopeKind::Workspace,
                Uuid::nil(),
            ));
    let service = ModelDefinitionService::new(repository.clone());

    let unsafe_external = service.get_model(Uuid::nil(), model_id).await.unwrap();

    assert_eq!(
        unsafe_external.api_exposure_status,
        ApiExposureStatus::ApiExposedReady
    );
}

#[tokio::test]
async fn external_model_with_scope_filter_capability_can_be_api_exposed_ready() {
    let model_id = Uuid::now_v7();
    let mut model = model_in_workspace(model_id, Uuid::nil());
    model.data_source_instance_id = Some(Uuid::now_v7());
    model.source_kind = domain::DataModelSourceKind::ExternalSource;
    model.external_resource_key = Some("contacts".into());
    model.external_capability_snapshot = Some(json!({
        "supports_owner_filter": false,
        "supports_scope_filter": true,
        "supports_write": false
    }));
    let repository =
        ScopedModelDefinitionRepository::new(ActorContext::root(Uuid::nil(), Uuid::nil(), "root"))
            .with_model(model)
            .with_grant(scope_grant(
                Uuid::now_v7(),
                model_id,
                DataModelScopeKind::Workspace,
                Uuid::nil(),
            ));
    let service = ModelDefinitionService::new(repository.clone());

    let effective = service.get_model(Uuid::nil(), model_id).await.unwrap();

    assert_eq!(
        effective.external_capability_snapshot,
        Some(json!({
            "supports_owner_filter": false,
            "supports_scope_filter": true,
            "supports_write": false
        }))
    );
    assert_eq!(
        effective.api_exposure_status,
        ApiExposureStatus::ApiExposedReady
    );
}

#[tokio::test]
async fn main_source_ready_path_is_not_blocked_by_external_source_safety() {
    let repository = InMemoryModelDefinitionRepository::default();
    let service = ModelDefinitionService::new(repository);
    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "main_source_ready_orders".into(),
            title: "Main Source Ready Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    let effective = service.get_model(Uuid::nil(), created.id).await.unwrap();

    assert_eq!(
        effective.api_exposure_status,
        ApiExposureStatus::ApiExposedReady
    );
}

#[tokio::test]
async fn create_model_persists_explicit_draft_status_in_initial_create_path() {
    let service = ModelDefinitionService::for_tests();

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "explicit_draft_orders".into(),
            title: "Explicit Draft Orders".into(),
            status: Some(DataModelStatus::Draft),
        })
        .await
        .unwrap();

    assert_eq!(created.status, DataModelStatus::Draft);
    assert_eq!(created.api_exposure_status, ApiExposureStatus::Draft);
}

#[tokio::test]
async fn create_model_inherits_data_source_defaults_when_instance_is_selected() {
    let data_source_instance_id = Uuid::now_v7();
    let repository = InMemoryModelDefinitionRepository::with_data_source_defaults(
        data_source_instance_id,
        DataSourceDefaults {
            data_model_status: DataModelStatus::Draft,
            api_exposure_status: ApiExposureStatus::Draft,
        },
    );
    let service = ModelDefinitionService::new(repository);

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: Some(data_source_instance_id),
            external_resource_key: Some("contacts".into()),
            external_table_id: None,
            code: "external_contacts".into(),
            title: "External Contacts".into(),
            status: None,
        })
        .await
        .unwrap();

    assert_eq!(
        created.data_source_instance_id,
        Some(data_source_instance_id)
    );
    assert_eq!(created.status, DataModelStatus::Draft);
    assert_eq!(created.api_exposure_status, ApiExposureStatus::Draft);
}

#[tokio::test]
async fn external_create_requires_external_resource_key_and_main_source_rejects_it() {
    let data_source_instance_id = Uuid::now_v7();
    let repository = InMemoryModelDefinitionRepository::with_data_source_defaults(
        data_source_instance_id,
        DataSourceDefaults::default(),
    );
    let service = ModelDefinitionService::new(repository);

    let missing_external_key = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: Some(data_source_instance_id),
            external_resource_key: None,
            external_table_id: None,
            code: "external_missing_key".into(),
            title: "External Missing Key".into(),
            status: None,
        })
        .await
        .unwrap_err();
    assert!(missing_external_key
        .to_string()
        .contains("external_resource_key"));

    let main_source_external_key = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: Some("contacts".into()),
            external_table_id: None,
            code: "main_source_external_key".into(),
            title: "Main Source External Key".into(),
            status: None,
        })
        .await
        .unwrap_err();
    assert!(main_source_external_key
        .to_string()
        .contains("external_resource_key"));
}

#[tokio::test]
async fn external_add_field_requires_external_field_key_and_main_source_rejects_it() {
    let data_source_instance_id = Uuid::now_v7();
    let repository = InMemoryModelDefinitionRepository::with_data_source_defaults(
        data_source_instance_id,
        DataSourceDefaults::default(),
    );
    let service = ModelDefinitionService::new(repository);
    let external_model = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: Some(data_source_instance_id),
            external_resource_key: Some("contacts".into()),
            external_table_id: None,
            code: "external_contacts_fields".into(),
            title: "External Contacts Fields".into(),
            status: None,
        })
        .await
        .unwrap();

    let missing_external_field_key = service
        .add_field(AddModelFieldCommand {
            actor_user_id: Uuid::nil(),
            model_id: external_model.id,
            code: "email".into(),
            title: "Email".into(),
            description: None,
            external_field_key: None,
            field_kind: ModelFieldKind::String,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
        })
        .await
        .unwrap_err();
    assert!(missing_external_field_key
        .to_string()
        .contains("external_field_key"));

    let main_model = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "main_source_fields".into(),
            title: "Main Source Fields".into(),
            status: None,
        })
        .await
        .unwrap();
    let main_source_external_field_key = service
        .add_field(AddModelFieldCommand {
            actor_user_id: Uuid::nil(),
            model_id: main_model.id,
            code: "email".into(),
            title: "Email".into(),
            description: None,
            external_field_key: Some("properties.email".into()),
            field_kind: ModelFieldKind::String,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
        })
        .await
        .unwrap_err();
    assert!(main_source_external_field_key
        .to_string()
        .contains("external_field_key"));
}

#[tokio::test]
async fn create_model_rejects_data_source_defaults_outside_actor_workspace() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let foreign_workspace_id = Uuid::now_v7();
    let data_source_instance_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_data_source_defaults(
                foreign_workspace_id,
                data_source_instance_id,
                DataSourceDefaults {
                    data_model_status: DataModelStatus::Draft,
                    api_exposure_status: ApiExposureStatus::Draft,
                },
            );
    let service = ModelDefinitionService::new(repository);

    let error = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: Some(data_source_instance_id),
            external_resource_key: Some("contacts".into()),
            external_table_id: None,
            code: "external_contacts".into(),
            title: "External Contacts".into(),
            status: None,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("data_source_instance"));
}
