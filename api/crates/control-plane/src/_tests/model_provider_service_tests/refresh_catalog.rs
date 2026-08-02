use super::*;

#[tokio::test]
async fn model_provider_service_normalizes_multiple_enabled_model_ids_and_allows_unknown_ids() {
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
            display_name: "Fixture Ready".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec![
                " fixture_chat ".to_string(),
                "".to_string(),
                "custom-alpha".to_string(),
                "fixture_chat".to_string(),
                " custom-alpha ".to_string(),
                "custom-beta".to_string(),
            ],
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();

    assert_eq!(created.instance.status, ModelProviderInstanceStatus::Ready);
    assert_eq!(
        created.instance.enabled_model_ids,
        vec![
            "fixture_chat".to_string(),
            "custom-alpha".to_string(),
            "custom-beta".to_string(),
        ]
    );
    assert_eq!(
        created.instance.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "fixture_chat".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-alpha".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-beta".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
        ]
    );

    let updated = service
        .update_instance(UpdateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            display_name: "Fixture Ready".to_string(),
            config_json: json!({}),
            configured_models: Vec::new(),
            enabled_model_ids: vec![
                " custom-beta ".to_string(),
                "fixture_chat".to_string(),
                "custom-beta".to_string(),
                "custom-gamma".to_string(),
                "  ".to_string(),
            ],
            included_in_main: created.instance.included_in_main,
            preview_token: None,
        })
        .await
        .unwrap();

    assert_eq!(updated.instance.status, ModelProviderInstanceStatus::Ready);
    assert_eq!(
        updated.instance.enabled_model_ids,
        vec![
            "custom-beta".to_string(),
            "fixture_chat".to_string(),
            "custom-gamma".to_string(),
        ]
    );
    assert_eq!(
        updated.instance.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "custom-beta".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "fixture_chat".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-gamma".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
        ]
    );
}

#[tokio::test]
async fn model_provider_service_reuses_preview_token_only_to_persist_candidate_cache() {
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
    let service = ModelProviderService::new(
        repository.clone(),
        runtime.clone(),
        "provider-secret-master-key",
    );

    let preview = service
        .preview_models(PreviewModelProviderModelsCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: Some(installation_id),
            instance_id: None,
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
        })
        .await
        .unwrap();
    assert_eq!(runtime.list_model_call_count().await, 1);

    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Preview".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string(), "custom-preview".to_string()],
            included_in_main: None,
            preview_token: Some(preview.preview_token),
        })
        .await
        .unwrap();

    assert_eq!(runtime.list_model_call_count().await, 1);
    assert_eq!(created.instance.status, ModelProviderInstanceStatus::Ready);
    assert_eq!(
        created.instance.enabled_model_ids,
        vec!["fixture_chat".to_string(), "custom-preview".to_string()]
    );
    assert_eq!(
        created
            .cache
            .as_ref()
            .map(|cache| cache.models_json[0]["model_id"].clone()),
        Some(json!("fixture_chat"))
    );
    assert!(repository
        .get_preview_session(workspace_id, preview.preview_token)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn model_provider_service_refresh_failure_does_not_clear_enabled_model_ids() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let package_root =
        std::env::temp_dir().join(format!("provider-model-refresh-{}", Uuid::now_v7()));
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
        runtime.clone(),
        "provider-secret-master-key",
    );

    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Fixture Refresh".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "super-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string(), "custom-refresh".to_string()],
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();

    runtime
        .set_list_models_error(Some("refresh failed for test"))
        .await;
    let error = service
        .refresh_models(repository.actor.user_id, created.instance.id)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("refresh failed for test"));
    assert_eq!(
        repository
            .get_instance(workspace_id, created.instance.id)
            .await
            .unwrap()
            .expect("instance should still exist")
            .enabled_model_ids,
        vec!["fixture_chat".to_string(), "custom-refresh".to_string()]
    );
    assert_eq!(
        repository
            .get_catalog_cache(created.instance.id)
            .await
            .unwrap()
            .expect("refresh failure should record cache state")
            .refresh_status,
        ModelProviderCatalogRefreshStatus::Failed
    );
}

#[tokio::test]
async fn model_provider_service_refresh_models_blocks_when_current_node_artifact_missing() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let package_root =
        std::env::temp_dir().join(format!("provider-model-refresh-{}", Uuid::now_v7()));
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
            display_name: "Fixture Refresh".to_string(),
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
        std::env::temp_dir().join(format!("provider-refresh-node-missing-{}", Uuid::now_v7()));
    let service =
        ModelProviderService::new(repository.clone(), runtime, "provider-secret-master-key")
            .with_node_artifact_context("node-without-artifact", current_node_root);

    let error = service
        .refresh_models(repository.actor.user_id, created.instance.id)
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("plugin_artifact_missing"))
    ));
}

#[tokio::test]
async fn list_catalog_returns_i18n_namespace_and_keys() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all"],
    ));
    let package_root = std::env::temp_dir().join(format!("provider-catalog-{}", Uuid::now_v7()));
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
        "provider-secret-master-key",
    );

    let entries = service
        .list_catalog(
            repository.actor.user_id,
            RequestedLocales::new("zh_Hans", "en_US"),
        )
        .await
        .unwrap();

    assert!(entries.i18n_catalog["plugin.fixture_provider"].contains_key("zh_Hans"));
    assert_eq!(entries.entries[0].namespace, "plugin.fixture_provider");
    assert_eq!(entries.entries[0].label_key, "provider.label");
    assert_eq!(
        entries.entries[0].predefined_models[0].label_key.as_deref(),
        Some("models.fixture_chat.label")
    );
    assert_eq!(repository.artifact_snapshot_update_count().await, 0);
}

#[tokio::test]
async fn list_catalog_returns_missing_projection_without_package_read() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all"],
    ));
    let package_root =
        std::env::temp_dir().join(format!("provider-catalog-no-projection-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    repository.remove_catalog_projection(installation_id).await;
    fs::remove_dir_all(&package_root).unwrap();
    let service = ModelProviderService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        "provider-secret-master-key",
    );

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            RequestedLocales::new("zh_Hans", "en_US"),
        )
        .await
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].catalog_refresh_status, "missing");
    assert_eq!(catalog.entries[0].display_name, "Fixture Provider");
    assert!(catalog.entries[0].form_schema.is_empty());
    assert!(catalog.entries[0].predefined_models.is_empty());
    assert_eq!(repository.artifact_snapshot_update_count().await, 0);
}

#[tokio::test]
async fn list_catalog_uses_persisted_missing_artifact_snapshot() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all"],
    ));
    let package_root =
        std::env::temp_dir().join(format!("provider-catalog-missing-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    fs::remove_dir_all(&package_root).unwrap();
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: "test-node".to_string(),
            installation_id,
            local_version: Some("0.1.0".to_string()),
            local_checksum: None,
            local_path: Some(package_root.display().to_string()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: PluginArtifactInstanceStatus::Missing,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::ArtifactMissing,
            checked_at: OffsetDateTime::now_utc(),
            last_error: None,
            is_current: true,
        })
        .await
        .unwrap();
    let maintenance_update_count = repository.artifact_snapshot_update_count().await;
    let service = ModelProviderService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        "provider-secret-master-key",
    )
    .with_node_artifact_context("test-node", package_root.parent().unwrap());

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            RequestedLocales::new("zh_Hans", "en_US"),
        )
        .await
        .unwrap();
    let artifact = repository
        .get_artifact_instance("test-node", installation_id)
        .await
        .unwrap()
        .unwrap();

    assert!(catalog.entries.is_empty());
    assert_eq!(
        artifact.artifact_status,
        PluginArtifactInstanceStatus::Missing
    );
    assert_eq!(
        artifact.availability_status,
        PluginAvailabilityStatus::ArtifactMissing
    );
    assert_eq!(
        repository.artifact_snapshot_update_count().await,
        maintenance_update_count
    );
}
