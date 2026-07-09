use super::*;

#[tokio::test]
async fn model_provider_service_enforces_permissions_and_audits_delete_conflict() {
    let workspace_id = Uuid::now_v7();
    let manager_repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let package_root = std::env::temp_dir().join(format!("provider-model-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = manager_repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let manager_service = ModelProviderService::new(
        manager_repository.clone(),
        MemoryProviderRuntime::default(),
        "provider-secret-master-key",
    );
    let created = manager_service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: manager_repository.actor.user_id,
            installation_id,
            display_name: "Fixture Prod".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();
    manager_repository
        .set_reference_count(created.instance.id, 2)
        .await;

    let error = manager_service
        .delete_instance(DeleteModelProviderInstanceCommand {
            actor_user_id: manager_repository.actor.user_id,
            instance_id: created.instance.id,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("model_provider_in_use"))
    ));
    assert_eq!(
        manager_repository.audit_events().await,
        vec!["model_provider.created", "model_provider.delete_conflict"]
    );

    let viewer_repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all"],
    ));
    let viewer_service = ModelProviderService::new(
        viewer_repository.clone(),
        MemoryProviderRuntime::default(),
        "provider-secret-master-key",
    );
    let catalog = viewer_service
        .list_catalog(
            viewer_repository.actor.user_id,
            RequestedLocales::new("en_US", "en_US"),
        )
        .await
        .unwrap();
    assert!(catalog.entries.is_empty());

    let error = viewer_service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: viewer_repository.actor.user_id,
            installation_id: Uuid::now_v7(),
            display_name: "Nope".to_string(),
            config_json: json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("permission_denied"))
    ));
}

#[tokio::test]
async fn model_provider_service_rejects_validating_disabled_instance() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let package_root =
        std::env::temp_dir().join(format!("provider-model-disabled-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let service = ModelProviderService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        "provider-secret-master-key",
    );

    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Prod".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();
    repository
        .set_instance_status(created.instance.id, ModelProviderInstanceStatus::Disabled)
        .await;

    let error = service
        .validate_instance(repository.actor.user_id, created.instance.id)
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidStateTransition { resource, from, to, .. })
            if *resource == "model_provider_instance" && from == "disabled" && to == "ready"
    ));
}

#[tokio::test]
async fn memory_model_provider_repository_scopes_main_instance_settings_by_workspace() {
    let workspace_a = Uuid::now_v7();
    let workspace_b = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_a,
        &["state_model.manage.all"],
    ));

    repository
        .upsert_main_instance(&UpsertModelProviderMainInstanceInput {
            workspace_id: workspace_a,
            provider_code: "fixture_provider".to_string(),
            auto_include_new_instances: false,
            model_distribution_rules: None,
            updated_by: repository.actor.user_id,
        })
        .await
        .unwrap();

    assert!(repository
        .get_main_instance(workspace_a, "fixture_provider")
        .await
        .unwrap()
        .is_some());
    assert!(repository
        .get_main_instance(workspace_b, "fixture_provider")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn model_provider_service_get_main_instance_defaults_to_auto_include_true_and_enforces_access_checks(
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let package_root = std::env::temp_dir().join(format!("provider-model-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let service = ModelProviderService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        "test-master-key",
    );

    let main_instance = service
        .get_main_instance(repository.actor.user_id, "fixture_provider")
        .await
        .unwrap();
    assert_eq!(main_instance.provider_code, "fixture_provider");
    assert!(main_instance.auto_include_new_instances);

    let missing_provider_error = service
        .get_main_instance(repository.actor.user_id, "missing_provider")
        .await
        .unwrap_err();
    assert!(matches!(
        missing_provider_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("model_provider"))
    ));

    let no_permission_repository =
        MemoryModelProviderRepository::new(actor_with_permissions(workspace_id, &[]));
    no_permission_repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let no_permission_service = ModelProviderService::new(
        no_permission_repository.clone(),
        MemoryProviderRuntime::default(),
        "test-master-key",
    );

    let permission_error = no_permission_service
        .get_main_instance(no_permission_repository.actor.user_id, "fixture_provider")
        .await
        .unwrap_err();
    assert!(matches!(
        permission_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("permission_denied"))
    ));
}

#[tokio::test]
async fn model_provider_service_updates_provider_main_instance_settings_without_touching_child_secrets_or_config(
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let package_root = std::env::temp_dir().join(format!("provider-model-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let service = ModelProviderService::new(repository.clone(), runtime, "test-master-key");

    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Stable".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string()],
            included_in_main: Some(true),
            preview_token: None,
        })
        .await
        .unwrap();
    let secret_before = repository.secret_json(created.instance.id).await;
    let config_before = repository
        .get_instance(repository.actor.current_workspace_id, created.instance.id)
        .await
        .unwrap()
        .unwrap()
        .config_json;

    let updated = service
        .update_main_instance(UpdateModelProviderMainInstanceCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".to_string(),
            auto_include_new_instances: false,
            model_distribution_rules: None,
        })
        .await
        .unwrap();

    assert_eq!(updated.provider_code, "fixture_provider");
    assert!(!updated.auto_include_new_instances);
    assert_eq!(
        repository.secret_json(created.instance.id).await,
        secret_before
    );
    assert_eq!(
        repository
            .get_instance(repository.actor.current_workspace_id, created.instance.id)
            .await
            .unwrap()
            .unwrap()
            .config_json,
        config_before
    );
    assert!(
        repository
            .get_instance(repository.actor.current_workspace_id, created.instance.id)
            .await
            .unwrap()
            .unwrap()
            .included_in_main
    );
}
