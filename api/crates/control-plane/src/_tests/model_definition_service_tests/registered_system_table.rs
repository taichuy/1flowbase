use super::*;

fn registered_system_table_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        code: "users".into(),
        title: "Users".into(),
        physical_table_name: "users".into(),
        protection: DataModelProtection {
            owner_kind: DataModelOwnerKind::Core,
            owner_id: None,
            is_protected: true,
        },
        fields: vec![ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: model_id,
            code: "status".into(),
            title: "Status".into(),
            description: None,
            physical_column_name: "status".into(),
            external_field_key: None,
            field_kind: ModelFieldKind::String,
            is_system: true,
            is_writable: false,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
            sort_order: 0,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        }],
        ..system_model(model_id)
    }
}

#[tokio::test]
async fn registered_system_table_rejects_system_field_physical_update_and_delete() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = registered_system_table_model(model_id);
    let field_id = model.fields[0].id;
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        actor_workspace_id,
    ))
    .with_model(model);
    let service = ModelDefinitionService::new(repository.clone());

    let update_error = service
        .update_field(UpdateModelFieldCommand {
            actor_user_id,
            model_id,
            field_id,
            title: "Status".into(),
            description: None,
            is_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_options: json!({}),
        })
        .await
        .unwrap_err();
    assert!(update_error
        .to_string()
        .contains("builtin_data_model_physical_fields_readonly"));

    let delete_error = service
        .delete_field(DeleteModelFieldCommand {
            actor_user_id,
            model_id,
            field_id,
            confirmed: true,
        })
        .await
        .unwrap_err();
    assert!(delete_error.to_string().contains("model_field"));

    let stored = repository
        .models
        .lock()
        .expect("model lock poisoned")
        .get(&model_id)
        .cloned()
        .unwrap();
    assert_eq!(stored.fields.len(), 1);
    assert!(stored.fields[0].is_required);
}

#[tokio::test]
async fn registered_system_table_allows_user_added_field_lifecycle() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        actor_workspace_id,
    ))
    .with_model(registered_system_table_model(model_id));
    let service = ModelDefinitionService::new(repository.clone());

    let field = service
        .add_field(AddModelFieldCommand {
            actor_user_id,
            model_id,
            code: "timezone".into(),
            title: "Timezone".into(),
            description: Some("User profile timezone".into()),
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
        .unwrap();

    let updated = service
        .update_field(UpdateModelFieldCommand {
            actor_user_id,
            model_id,
            field_id: field.id,
            title: "Timezone".into(),
            description: Some("Preferred timezone".into()),
            is_required: true,
            is_unique: false,
            default_value: Some(json!("UTC")),
            display_interface: Some("input".into()),
            display_options: json!({}),
            relation_options: json!({}),
        })
        .await
        .unwrap();
    assert!(updated.is_required);
    assert_eq!(updated.default_value, Some(json!("UTC")));
    assert_eq!(updated.description.as_deref(), Some("Preferred timezone"));

    service
        .delete_field(DeleteModelFieldCommand {
            actor_user_id,
            model_id,
            field_id: field.id,
            confirmed: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn registered_system_table_allows_non_physical_field_metadata_update() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = registered_system_table_model(model_id);
    let field_id = model.fields[0].id;
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        actor_workspace_id,
    ))
    .with_model(model);
    let service = ModelDefinitionService::new(repository.clone());

    let updated = service
        .update_field(UpdateModelFieldCommand {
            actor_user_id,
            model_id,
            field_id,
            title: "Status display".into(),
            description: None,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: Some("badge".into()),
            display_options: json!({ "tone": "neutral" }),
            relation_options: json!({}),
        })
        .await
        .unwrap();

    assert_eq!(updated.title, "Status display");
    assert_eq!(updated.display_interface.as_deref(), Some("badge"));
    assert!(updated.is_required);
}
