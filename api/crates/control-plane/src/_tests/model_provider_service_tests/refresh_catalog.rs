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
    let service = model_provider_service(repository.clone(), runtime, "provider-secret-master-key");

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
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-alpha".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-beta".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
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
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            },
            domain::ModelProviderConfiguredModel {
                model_id: "fixture_chat".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-gamma".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
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
    let service = model_provider_service(
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
    let runtime = MemoryProviderRuntime {
        operation_trace: repository.operation_trace.clone(),
        ..Default::default()
    };
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
    let service = model_provider_service(
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

    let old_cache = repository
        .get_catalog_cache(created.instance.id)
        .await
        .unwrap()
        .unwrap();
    let original_audits = repository.audit_events().await;
    for (catalog_error, load_error, list_error, expected_trace, expected_error) in [
        (
            true,
            false,
            false,
            vec!["get_secret_json", "get_catalog_cache"],
            "catalog read failed for test",
        ),
        (
            false,
            true,
            false,
            vec![
                "get_secret_json",
                "get_catalog_cache",
                "ensure_loaded",
                "cache_failed",
                "audit",
            ],
            "ensure loaded failed for test",
        ),
        (
            false,
            false,
            true,
            vec![
                "get_secret_json",
                "get_catalog_cache",
                "ensure_loaded",
                "list_models",
                "cache_failed",
                "audit",
            ],
            "refresh failed for test",
        ),
    ] {
        *repository.catalog_read_error.write().await = catalog_error;
        *runtime.ensure_loaded_error.write().await = load_error;
        runtime
            .set_list_models_error(list_error.then_some("refresh failed for test"))
            .await;
        repository.operation_trace.write().await.clear();
        let list_count = runtime.list_model_call_count().await;
        let error = service
            .refresh_models(repository.actor.user_id, created.instance.id)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected_error);
        assert_eq!(*repository.operation_trace.read().await, expected_trace);
        assert_eq!(runtime.list_model_call_count().await, list_count);
        *repository.catalog_read_error.write().await = false;
        let cache = repository
            .get_catalog_cache(created.instance.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cache.models_json, old_cache.models_json);
        assert_eq!(cache.refreshed_at, old_cache.refreshed_at);
        if catalog_error {
            assert_eq!(cache.refresh_status, old_cache.refresh_status);
            assert_eq!(repository.audit_events().await, original_audits);
        } else {
            assert_eq!(
                cache.refresh_status,
                ModelProviderCatalogRefreshStatus::Failed
            );
            assert_eq!(cache.last_error_message.as_deref(), Some(expected_error));
        }
    }
    let mut expected_audits = original_audits;
    expected_audits.extend(vec!["model_provider.models_refresh_failed".to_string(); 2]);
    assert_eq!(repository.audit_events().await, expected_audits);
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
    runtime.set_list_models_error(None).await;
    repository.operation_trace.write().await.clear();
    let list_count = runtime.list_model_call_count().await;
    let refreshed = service
        .refresh_models(repository.actor.user_id, created.instance.id)
        .await
        .unwrap();
    assert_eq!(
        refreshed.refresh_status,
        ModelProviderCatalogRefreshStatus::Ready
    );
    assert_eq!(runtime.list_model_call_count().await, list_count + 1);
    assert_eq!(
        *repository.operation_trace.read().await,
        vec![
            "get_secret_json",
            "get_catalog_cache",
            "ensure_loaded",
            "list_models",
            "cache_ready",
            "audit",
        ]
    );
    expected_audits.push("model_provider.models_refreshed".to_string());
    assert_eq!(repository.audit_events().await, expected_audits);
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
    let bootstrap_service = model_provider_service(
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
    let service = model_provider_service(repository.clone(), runtime, "provider-secret-master-key")
        .with_node_artifact_context("node-without-artifact", current_node_root);

    let audits = repository.audit_events().await;
    let cache = repository
        .get_catalog_cache(created.instance.id)
        .await
        .unwrap()
        .unwrap();
    repository.operation_trace.write().await.clear();
    let error = service
        .refresh_models(repository.actor.user_id, created.instance.id)
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginUnavailable)
    ));
    assert!(repository.operation_trace.read().await.is_empty());
    assert_eq!(repository.audit_events().await, audits);
    let unchanged = repository
        .get_catalog_cache(created.instance.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.models_json, cache.models_json);
    assert_eq!(unchanged.refreshed_at, cache.refreshed_at);
    assert_eq!(unchanged.refresh_status, cache.refresh_status);
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
    let service = model_provider_service(
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
    let service = model_provider_service(
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
    let service = model_provider_service(
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
