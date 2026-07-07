use super::*;

#[tokio::test]
async fn model_provider_service_masks_secret_in_views_and_reveals_on_demand() {
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

    let service =
        ModelProviderService::new(repository.clone(), runtime, "provider-secret-master-key");

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
    assert_eq!(created.instance.status, ModelProviderInstanceStatus::Draft);
    assert_eq!(
        created.instance.config_json["base_url"],
        "https://api.example.com"
    );
    assert_eq!(created.instance.config_json["api_key"], "supe****cret");
    assert_eq!(
        repository.secret_json(created.instance.id).await["api_key"],
        "super-secret"
    );

    let listed = service
        .list_instances(repository.actor.user_id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].instance.config_json["api_key"], "supe****cret");

    let revealed = service
        .reveal_secret(repository.actor.user_id, created.instance.id, "api_key")
        .await
        .unwrap();
    assert_eq!(revealed, "super-secret");

    let validated = service
        .validate_instance(repository.actor.user_id, created.instance.id)
        .await
        .unwrap();
    assert_eq!(
        validated.instance.status,
        ModelProviderInstanceStatus::Draft
    );
    assert_eq!(validated.instance.config_json["api_key"], "supe****cret");
    assert_eq!(validated.instance.enabled_model_ids, Vec::<String>::new());
    assert_eq!(
        validated.cache.refresh_status,
        ModelProviderCatalogRefreshStatus::Ready
    );
    assert_eq!(validated.output["sanitized"]["api_key"], "***");

    let options = service
        .options(
            repository.actor.user_id,
            RequestedLocales::new("zh_Hans", "en_US"),
        )
        .await
        .unwrap();
    assert!(options.providers.is_empty());
    assert!(options.i18n_catalog.is_empty());

    let refreshed = service
        .refresh_models(repository.actor.user_id, created.instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.models.len(), 1);
    assert_eq!(refreshed.models[0].context_window, Some(128000));
    assert_eq!(
        repository.audit_events().await,
        vec![
            "model_provider.created",
            "model_provider.validated",
            "model_provider.models_refreshed"
        ]
    );
}

#[tokio::test]
async fn model_provider_service_create_instance_inherits_provider_main_instance_default() {
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

    let main_instance = service
        .update_main_instance(UpdateModelProviderMainInstanceCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".to_string(),
            auto_include_new_instances: false,
            model_distribution_rules: None,
        })
        .await
        .unwrap();
    assert!(!main_instance.auto_include_new_instances);

    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Excluded".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string()],
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();

    assert!(!created.instance.included_in_main);
}

#[tokio::test]
async fn model_provider_service_update_instance_can_flip_included_in_main_without_changing_enabled_model_ids(
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
            display_name: "Fixture Included".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string(), "custom-alpha".to_string()],
            included_in_main: Some(true),
            preview_token: None,
        })
        .await
        .unwrap();

    let updated = service
        .update_instance(UpdateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            display_name: "Fixture Included".to_string(),
            config_json: json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string(), "custom-alpha".to_string()],
            included_in_main: false,
            preview_token: None,
        })
        .await
        .unwrap();

    assert!(!updated.instance.included_in_main);
    assert_eq!(
        updated.instance.enabled_model_ids,
        vec!["fixture_chat".to_string(), "custom-alpha".to_string()]
    );
}

#[tokio::test]
async fn model_provider_service_list_instances_returns_included_in_main_without_primary_flags() {
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

    let included = repository
        .create_instance(&CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: repository.actor.current_workspace_id,
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Included".to_string(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://included.example.com/v1"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string()],
            included_in_main: Some(true),
            created_by: repository.actor.user_id,
        })
        .await
        .unwrap();
    let excluded = repository
        .create_instance(&CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: repository.actor.current_workspace_id,
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Excluded".to_string(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://excluded.example.com/v1"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["custom-excluded".to_string()],
            included_in_main: Some(false),
            created_by: repository.actor.user_id,
        })
        .await
        .unwrap();

    let service = ModelProviderService::new(repository.clone(), runtime, "test-master-key");
    let instances = service
        .list_instances(repository.actor.user_id)
        .await
        .unwrap();

    let included_view = instances
        .into_iter()
        .find(|view| view.instance.id == included.id)
        .unwrap();
    let crate::model_provider::ModelProviderInstanceView { instance, cache } = included_view;
    assert!(cache.is_none());
    assert!(instance.included_in_main);

    let excluded_view = service
        .list_instances(repository.actor.user_id)
        .await
        .unwrap()
        .into_iter()
        .find(|view| view.instance.id == excluded.id)
        .unwrap();
    let crate::model_provider::ModelProviderInstanceView { instance, cache } = excluded_view;
    assert!(cache.is_none());
    assert!(!instance.included_in_main);
}

#[tokio::test]
async fn model_provider_service_list_instances_does_not_read_global_install_path_when_current_node_artifact_missing(
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let install_root =
        std::env::temp_dir().join(format!("provider-model-node-missing-{}", Uuid::now_v7()));
    let installation_id = repository
        .seed_installation(
            "/path/from/another/api/node",
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let instance = repository
        .create_instance(&CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: repository.actor.current_workspace_id,
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Fixture Missing Local Artifact".to_string(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://api.example.com"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: Some(true),
            created_by: repository.actor.user_id,
        })
        .await
        .unwrap();
    let service = ModelProviderService::new(repository.clone(), runtime, "test-master-key")
        .with_node_artifact_context("test-node", install_root);

    let instances = service
        .list_instances(repository.actor.user_id)
        .await
        .unwrap();

    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].instance.id, instance.id);
    assert_eq!(
        instances[0].instance.config_json["base_url"],
        "https://api.example.com"
    );
}

#[tokio::test]
async fn model_provider_service_update_instance_blocks_when_current_node_artifact_missing() {
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
    let bootstrap_service = ModelProviderService::new(
        repository.clone(),
        runtime.clone(),
        "provider-secret-master-key",
    );
    let created = bootstrap_service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Prod".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string()],
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();
    let current_node_root =
        std::env::temp_dir().join(format!("provider-node-missing-{}", Uuid::now_v7()));
    let service =
        ModelProviderService::new(repository.clone(), runtime, "provider-secret-master-key")
            .with_node_artifact_context("node-without-artifact", current_node_root);

    let error = service
        .update_instance(UpdateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            display_name: "Fixture Prod".to_string(),
            config_json: json!({}),
            configured_models: created.instance.configured_models.clone(),
            enabled_model_ids: created.instance.enabled_model_ids.clone(),
            included_in_main: created.instance.included_in_main,
            preview_token: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("plugin_artifact_missing"))
    ));
}

#[tokio::test]
async fn model_provider_service_blocks_previously_failed_current_node_runtime_without_refreshing_it(
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
    let bootstrap_service = ModelProviderService::new(
        repository.clone(),
        runtime.clone(),
        "provider-secret-master-key",
    );
    let created = bootstrap_service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Prod".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string()],
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();
    let current_node_root =
        std::env::temp_dir().join(format!("provider-load-failed-{}", Uuid::now_v7()));
    let expected_local_path = current_node_root
        .join("installed")
        .join("fixture_provider")
        .join("0.1.0");
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: "test-node".to_string(),
            installation_id,
            local_version: Some("0.1.0".to_string()),
            local_checksum: None,
            installed_path: Some(expected_local_path.display().to_string()),
            artifact_status: domain::PluginArtifactInstanceStatus::LoadFailed,
            runtime_status: PluginRuntimeStatus::LoadFailed,
            checked_at: OffsetDateTime::now_utc(),
            last_error: Some("runtime load failed for test".to_string()),
        })
        .await
        .unwrap();
    let service =
        ModelProviderService::new(repository.clone(), runtime, "provider-secret-master-key")
            .with_node_artifact_context("test-node", current_node_root);

    let error = service
        .validate_instance(repository.actor.user_id, created.instance.id)
        .await
        .unwrap_err();
    let artifact = repository
        .get_artifact_instance("test-node", installation_id)
        .await
        .unwrap()
        .expect("artifact snapshot should remain");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("plugin_runtime_load_failed"))
    ));
    assert_eq!(
        artifact.artifact_status,
        domain::PluginArtifactInstanceStatus::LoadFailed
    );
}

#[tokio::test]
async fn model_provider_service_create_and_update_allow_empty_enabled_model_ids() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let package_root =
        std::env::temp_dir().join(format!("provider-model-preview-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let service =
        ModelProviderService::new(repository.clone(), runtime, "provider-secret-master-key");

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

    assert_eq!(created.instance.status, ModelProviderInstanceStatus::Draft);
    assert!(created.instance.enabled_model_ids.is_empty());

    let updated = service
        .update_instance(UpdateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            display_name: "Fixture Draft".to_string(),
            config_json: json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["  ".to_string(), "".to_string()],
            included_in_main: created.instance.included_in_main,
            preview_token: None,
        })
        .await
        .unwrap();

    assert_eq!(updated.instance.status, ModelProviderInstanceStatus::Draft);
    assert!(updated.instance.enabled_model_ids.is_empty());
}
