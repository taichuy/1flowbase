use super::*;

#[tokio::test]
async fn model_provider_service_options_group_models_by_model_id_with_ordered_instance_targets() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let package_root = std::env::temp_dir().join(format!(
        "provider-options-enabled-models-{}",
        Uuid::now_v7()
    ));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let service = ModelProviderService::new(repository.clone(), runtime, "test-master-key");

    let alpha = repository
        .create_instance(&CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: repository.actor.current_workspace_id,
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Alpha".to_string(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://alpha.example.com/v1"
            }),
            configured_models: vec![domain::ModelProviderConfiguredModel {
                model_id: "fixture_chat".to_string(),
                enabled: true,
                context_window_override_tokens: Some(256_000),
                supports_multimodal: None,
            }],
            enabled_model_ids: vec!["fixture_chat".to_string(), "custom-enabled".to_string()],
            included_in_main: Some(true),
            created_by: repository.actor.user_id,
        })
        .await
        .unwrap();
    let beta = repository
        .create_instance(&CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: repository.actor.current_workspace_id,
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Beta".to_string(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://beta.example.com/v1"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: vec!["fixture_chat".to_string()],
            included_in_main: Some(true),
            created_by: repository.actor.user_id,
        })
        .await
        .unwrap();
    repository
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
            enabled_model_ids: vec!["excluded-model".to_string()],
            included_in_main: Some(false),
            created_by: repository.actor.user_id,
        })
        .await
        .unwrap();

    repository
        .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
            provider_instance_id: alpha.id,
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            refresh_status: ModelProviderCatalogRefreshStatus::Ready,
            source: domain::ModelProviderCatalogSource::Hybrid,
            models_json: serde_json::to_value(vec![
                ProviderModelDescriptor {
                    model_id: "fixture_chat".to_string(),
                    display_name: "Fixture Chat".to_string(),
                    source: ProviderModelSource::Dynamic,
                    supports_streaming: true,
                    supports_tool_call: false,
                    supports_multimodal: false,
                    context_window: Some(128000),
                    max_output_tokens: Some(4096),
                    provider_metadata: json!({}),
                },
                ProviderModelDescriptor {
                    model_id: "candidate-only".to_string(),
                    display_name: "Candidate Only".to_string(),
                    source: ProviderModelSource::Dynamic,
                    supports_streaming: true,
                    supports_tool_call: false,
                    supports_multimodal: false,
                    context_window: Some(64000),
                    max_output_tokens: Some(2048),
                    provider_metadata: json!({}),
                },
            ])
            .unwrap(),
            last_error_message: None,
            refreshed_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();

    repository
        .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
            provider_instance_id: beta.id,
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            refresh_status: ModelProviderCatalogRefreshStatus::Ready,
            source: domain::ModelProviderCatalogSource::Hybrid,
            models_json: serde_json::to_value(vec![ProviderModelDescriptor {
                model_id: "fixture_chat".to_string(),
                display_name: "Fixture Chat Backup".to_string(),
                source: ProviderModelSource::Dynamic,
                supports_streaming: true,
                supports_tool_call: false,
                supports_multimodal: false,
                context_window: Some(64000),
                max_output_tokens: Some(2048),
                provider_metadata: json!({}),
            }])
            .unwrap(),
            last_error_message: None,
            refreshed_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();

    let options = service
        .options(
            repository.actor.user_id,
            RequestedLocales::new("zh_Hans", "en_US"),
        )
        .await
        .unwrap();

    assert_eq!(options.providers.len(), 1);
    assert_eq!(options.providers[0].icon.as_deref(), Some("icon.svg"));
    assert_eq!(
        options.providers[0].main_instance.provider_code,
        "fixture_provider"
    );
    assert_eq!(options.providers[0].main_instance.group_count, 2);
    assert_eq!(options.providers[0].main_instance.model_count, 2);
    assert_eq!(options.providers[0].model_groups.len(), 2);

    let fixture_chat_group = options.providers[0]
        .model_groups
        .iter()
        .find(|group| group.model_id == "fixture_chat")
        .unwrap();
    assert_eq!(fixture_chat_group.model.descriptor.model_id, "fixture_chat");
    assert_eq!(
        fixture_chat_group
            .targets
            .iter()
            .map(|target| (
                target.source_instance_id,
                target.source_instance_display_name.as_str(),
                target.model.descriptor.display_name.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (alpha.id, "Alpha", "Fixture Chat"),
            (beta.id, "Beta", "Fixture Chat Backup")
        ]
    );
    assert_eq!(
        fixture_chat_group.model.display_name_fallback.as_deref(),
        Some("Fixture Chat")
    );
    assert_eq!(
        fixture_chat_group.model.descriptor.context_window,
        Some(256_000)
    );

    let custom_group = options.providers[0]
        .model_groups
        .iter()
        .find(|group| group.model_id == "custom-enabled")
        .unwrap();
    assert_eq!(
        custom_group
            .targets
            .iter()
            .map(|target| (
                target.source_instance_id,
                target.source_instance_display_name.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(alpha.id, "Alpha")]
    );
    assert_eq!(
        custom_group.model.display_name_fallback.as_deref(),
        Some("custom-enabled")
    );
    assert_eq!(custom_group.model.label_key, None);
}

#[tokio::test]
async fn model_provider_service_persists_configured_models_and_derives_enabled_model_ids() {
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
            configured_models: vec![
                ModelProviderConfiguredModelInput {
                    model_id: " fixture_chat ".to_string(),
                    enabled: true,
                    context_window_override_tokens: Some(128_000),
                    supports_multimodal: Some(true),
                },
                ModelProviderConfiguredModelInput {
                    model_id: " custom-disabled ".to_string(),
                    enabled: false,
                    context_window_override_tokens: None,
                    supports_multimodal: Some(false),
                },
                ModelProviderConfiguredModelInput {
                    model_id: "".to_string(),
                    enabled: true,
                    context_window_override_tokens: Some(64_000),
                    supports_multimodal: None,
                },
            ],
            enabled_model_ids: vec!["legacy-should-be-ignored".to_string()],
            included_in_main: None,
            preview_token: None,
        })
        .await
        .unwrap();

    assert_eq!(created.instance.status, ModelProviderInstanceStatus::Ready);
    assert_eq!(
        created.instance.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "fixture_chat".to_string(),
                enabled: true,
                context_window_override_tokens: Some(128_000),
                supports_multimodal: Some(true),
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-disabled".to_string(),
                enabled: false,
                context_window_override_tokens: None,
                supports_multimodal: Some(false),
            },
        ]
    );
    assert_eq!(
        created.instance.enabled_model_ids,
        vec!["fixture_chat".to_string()]
    );

    let updated = service
        .update_instance(UpdateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            display_name: "Fixture Ready".to_string(),
            config_json: json!({}),
            configured_models: vec![
                ModelProviderConfiguredModelInput {
                    model_id: "fixture_chat".to_string(),
                    enabled: false,
                    context_window_override_tokens: Some(64_000),
                    supports_multimodal: None,
                },
                ModelProviderConfiguredModelInput {
                    model_id: " custom-enabled ".to_string(),
                    enabled: true,
                    context_window_override_tokens: Some(256_000),
                    supports_multimodal: Some(true),
                },
                ModelProviderConfiguredModelInput {
                    model_id: "custom-enabled".to_string(),
                    enabled: true,
                    context_window_override_tokens: None,
                    supports_multimodal: None,
                },
            ],
            enabled_model_ids: vec!["legacy-should-be-ignored".to_string()],
            included_in_main: created.instance.included_in_main,
            preview_token: None,
        })
        .await
        .unwrap();

    assert_eq!(updated.instance.status, ModelProviderInstanceStatus::Ready);
    assert_eq!(
        updated.instance.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "fixture_chat".to_string(),
                enabled: false,
                context_window_override_tokens: Some(64_000),
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "custom-enabled".to_string(),
                enabled: true,
                context_window_override_tokens: Some(256_000),
                supports_multimodal: Some(true),
            },
        ]
    );
    assert_eq!(
        updated.instance.enabled_model_ids,
        vec!["custom-enabled".to_string()]
    );
}
