use super::*;

#[tokio::test]
async fn update_model_status_rejects_model_outside_actor_workspace() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let foreign_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(model_in_workspace(model_id, foreign_workspace_id));
    let service = ModelDefinitionService::new(repository.clone());

    let error = service
        .update_model_status(UpdateModelDefinitionStatusCommand {
            actor_user_id,
            model_id,
            status: DataModelStatus::Draft,
            api_exposure_status: ApiExposureStatus::Draft,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("model_definition"));
    let stored = repository
        .models
        .lock()
        .expect("model lock poisoned")
        .get(&model_id)
        .cloned()
        .unwrap();
    assert_eq!(stored.status, DataModelStatus::Published);
    assert_eq!(
        stored.api_exposure_status,
        ApiExposureStatus::PublishedNotExposed
    );
}

#[tokio::test]
async fn non_root_admin_cannot_mutate_protected_data_model() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = protected_extension_model(model_id);
    let field_id = model.fields[0].id;
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        actor_workspace_id,
    ))
    .with_model(model);
    let service = ModelDefinitionService::new(repository.clone());

    let status_error = service
        .update_model_status(UpdateModelDefinitionStatusCommand {
            actor_user_id,
            model_id,
            status: DataModelStatus::Disabled,
            api_exposure_status: ApiExposureStatus::PublishedNotExposed,
        })
        .await
        .unwrap_err();
    assert!(status_error.to_string().contains("protected_data_model"));

    let update_field_error = service
        .update_field(UpdateModelFieldCommand {
            actor_user_id,
            model_id,
            field_id,
            title: "Work Email".into(),
            description: None,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_options: json!({}),
        })
        .await
        .unwrap_err();
    assert!(update_field_error
        .to_string()
        .contains("protected_data_model"));

    let delete_field_error = service
        .delete_field(DeleteModelFieldCommand {
            actor_user_id,
            model_id,
            field_id,
            confirmed: true,
        })
        .await
        .unwrap_err();
    assert!(delete_field_error
        .to_string()
        .contains("protected_data_model"));

    let publish_error = match service
        .publish_model(PublishModelCommand {
            actor_user_id,
            model_id,
        })
        .await
    {
        Ok(_) => panic!("protected publish should be rejected for non-root admin"),
        Err(error) => error,
    };
    assert!(publish_error.to_string().contains("protected_data_model"));

    let delete_model_error = service
        .delete_model(DeleteModelDefinitionCommand {
            actor_user_id,
            model_id,
            confirmed: true,
        })
        .await
        .unwrap_err();
    assert!(delete_model_error
        .to_string()
        .contains("protected_data_model"));

    let stored = repository
        .models
        .lock()
        .expect("model lock poisoned")
        .get(&model_id)
        .cloned()
        .unwrap();
    assert_eq!(stored.status, DataModelStatus::Published);
    assert_eq!(stored.fields[0].title, "Email");
}

#[tokio::test]
async fn root_can_override_protected_data_model_enforcement() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = protected_extension_model(model_id);
    let field_id = model.fields[0].id;
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(model);
    let service = ModelDefinitionService::new(repository.clone());

    let updated = service
        .update_field(UpdateModelFieldCommand {
            actor_user_id,
            model_id,
            field_id,
            title: "Emergency Email".into(),
            description: None,
            is_required: true,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_options: json!({}),
        })
        .await
        .unwrap();
    assert_eq!(updated.title, "Emergency Email");

    service
        .delete_model(DeleteModelDefinitionCommand {
            actor_user_id,
            model_id,
            confirmed: true,
        })
        .await
        .unwrap();

    assert!(!repository
        .models
        .lock()
        .expect("model lock poisoned")
        .contains_key(&model_id));
}

#[tokio::test]
async fn advisor_findings_report_protection_and_field_risks() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let mut model = protected_extension_model(model_id);
    model.audit_namespace = "".into();
    model.api_exposure_status = ApiExposureStatus::ApiExposedReady;
    model.fields.push(ModelFieldRecord {
        id: Uuid::now_v7(),
        data_model_id: model_id,
        code: "email".into(),
        title: "Email Duplicate".into(),
        description: None,
        physical_column_name: "email_dup".into(),
        external_field_key: Some("email".into()),
        field_kind: ModelFieldKind::Json,
        is_system: false,
        is_writable: true,
        is_required: false,
        is_unique: true,
        default_value: None,
        display_interface: None,
        display_options: json!({}),
        relation_target_model_id: None,
        relation_options: json!({}),
        sort_order: 1,
        availability_status: domain::MetadataAvailabilityStatus::Available,
    });
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(model);
    let service = ModelDefinitionService::new(repository);

    let findings = service
        .advisor_findings(actor_user_id, model_id)
        .await
        .unwrap();
    let codes = findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"protected_model_exposure_attempt"));
    assert!(codes.contains(&"duplicate_risky_field_configuration"));
    assert!(findings
        .iter()
        .any(|finding| finding.severity == domain::DataModelAdvisorSeverity::Blocking));
}

#[tokio::test]
async fn advisor_findings_report_unsafe_external_source() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let unsafe_external_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(unsafe_external_system_model(unsafe_external_id));
    let service = ModelDefinitionService::new(repository);

    let unsafe_external = service
        .advisor_findings(actor_user_id, unsafe_external_id)
        .await
        .unwrap();
    assert!(unsafe_external.iter().any(|finding| {
        finding.code == "unsafe_external_source"
            && finding.severity == domain::DataModelAdvisorSeverity::Blocking
    }));
}
