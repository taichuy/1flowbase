use super::*;

#[tokio::test]
async fn model_provider_repository_persists_instances_catalog_cache_and_encrypted_secrets() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let empty_instance_id = Uuid::now_v7();
    let empty_instance = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: empty_instance_id,
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider Prod".into(),
            status: ModelProviderInstanceStatus::Draft,
            config_json: json!({ "base_url": "https://api.example.com" }),
            configured_models: vec![],
            enabled_model_ids: vec![],
            included_in_main: None,
            created_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(empty_instance.status, ModelProviderInstanceStatus::Draft);
    assert_eq!(empty_instance.enabled_model_ids, Vec::<String>::new());

    let empty_updated = ModelProviderRepository::update_instance(
        &store,
        &UpdateModelProviderInstanceInput {
            instance_id: empty_instance_id,
            workspace_id: workspace.id,
            display_name: "Fixture Provider Draft".into(),
            status: ModelProviderInstanceStatus::Draft,
            config_json: json!({ "base_url": "https://api.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec![],
            included_in_main: true,
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(empty_updated.status, ModelProviderInstanceStatus::Draft);
    assert_eq!(empty_updated.enabled_model_ids, Vec::<String>::new());

    let pair_instance_id = Uuid::now_v7();
    let pair_instance = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: pair_instance_id,
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider Ready".into(),
            status: ModelProviderInstanceStatus::Draft,
            config_json: json!({ "base_url": "https://api.example.com" }),
            configured_models: vec![
                domain::ModelProviderConfiguredModel {
                    model_id: "qwen-max".into(),
                    enabled: true,
                    context_window_override_tokens: Some(128_000),
                    supports_multimodal: None,
                },
                domain::ModelProviderConfiguredModel {
                    model_id: "qwen-plus".into(),
                    enabled: false,
                    context_window_override_tokens: None,
                    supports_multimodal: None,
                },
            ],
            enabled_model_ids: vec!["qwen-max".into(), "qwen-plus".into()],
            included_in_main: None,
            created_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        pair_instance.enabled_model_ids,
        vec!["qwen-max".to_string(), "qwen-plus".to_string()]
    );
    assert_eq!(
        pair_instance.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "qwen-max".to_string(),
                enabled: true,
                context_window_override_tokens: Some(128_000),
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "qwen-plus".to_string(),
                enabled: false,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
        ]
    );

    let pair_updated = ModelProviderRepository::update_instance(
        &store,
        &UpdateModelProviderInstanceInput {
            instance_id: pair_instance_id,
            workspace_id: workspace.id,
            display_name: "Fixture Provider Ready".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://api.example.com/v1" }),
            configured_models: vec![
                domain::ModelProviderConfiguredModel {
                    model_id: "qwen-max".into(),
                    enabled: true,
                    context_window_override_tokens: Some(256_000),
                    supports_multimodal: None,
                },
                domain::ModelProviderConfiguredModel {
                    model_id: "qwen-plus".into(),
                    enabled: false,
                    context_window_override_tokens: None,
                    supports_multimodal: None,
                },
            ],
            enabled_model_ids: vec!["qwen-max".into(), "qwen-plus".into()],
            included_in_main: true,
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(pair_updated.status, ModelProviderInstanceStatus::Ready);
    assert_eq!(
        pair_updated.enabled_model_ids,
        vec!["qwen-max".to_string(), "qwen-plus".to_string()]
    );
    assert_eq!(
        pair_updated.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "qwen-max".to_string(),
                enabled: true,
                context_window_override_tokens: Some(256_000),
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "qwen-plus".to_string(),
                enabled: false,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
        ]
    );

    let cache = ModelProviderRepository::upsert_catalog_cache(
        &store,
        &UpsertModelProviderCatalogCacheInput {
            provider_instance_id: pair_instance_id,
            model_discovery_mode: ModelProviderDiscoveryMode::Hybrid,
            refresh_status: ModelProviderCatalogRefreshStatus::Ready,
            source: ModelProviderCatalogSource::Hybrid,
            models_json: json!([
                {
                    "model_id": "fixture_chat",
                    "display_name": "Fixture Chat"
                }
            ]),
            last_error_message: None,
            refreshed_at: Some(time::OffsetDateTime::now_utc()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        cache.refresh_status,
        ModelProviderCatalogRefreshStatus::Ready
    );

    let secret = ModelProviderRepository::upsert_secret(
        &store,
        &UpsertModelProviderSecretInput {
            provider_instance_id: pair_instance_id,
            plaintext_secret_json: json!({ "api_key": "super-secret" }),
            secret_version: 1,
            master_key: "provider-secret-master-key".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(secret.secret_version, 1);

    let stored_secret: Value = sqlx::query_scalar(
        "select encrypted_secret_json from model_provider_instance_secrets where provider_instance_id = $1",
    )
    .bind(pair_instance_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(!stored_secret.to_string().contains("super-secret"));

    let decrypted = ModelProviderRepository::get_secret_json(
        &store,
        pair_instance_id,
        "provider-secret-master-key",
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(decrypted["api_key"], "super-secret");

    let instances = ModelProviderRepository::list_instances(&store, workspace.id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 2);
    let cache_record = ModelProviderRepository::get_catalog_cache(&store, pair_instance_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cache_record.models_json[0]["model_id"], "fixture_chat");
}

#[tokio::test]
async fn model_provider_repository_persists_main_instance_defaults_and_instance_inclusion_flags() {
    let (store, workspace, actor, installation_id) = seed_store().await;

    let default_include = ModelProviderRepository::upsert_main_instance(
        &store,
        &UpsertModelProviderMainInstanceInput {
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            auto_include_new_instances: true,
            model_distribution_rules: None,
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert!(default_include.auto_include_new_instances);
    assert!(
        ModelProviderRepository::get_main_instance(&store, workspace.id, "fixture_provider")
            .await
            .unwrap()
            .unwrap()
            .auto_include_new_instances
    );

    let inherits_true = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Primary".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://primary.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec!["gpt-4o-mini".into()],
            included_in_main: None,
            created_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert!(inherits_true.included_in_main);

    let default_exclude = ModelProviderRepository::upsert_main_instance(
        &store,
        &UpsertModelProviderMainInstanceInput {
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            auto_include_new_instances: false,
            model_distribution_rules: None,
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert!(!default_exclude.auto_include_new_instances);
    assert!(
        !ModelProviderRepository::get_main_instance(&store, workspace.id, "fixture_provider")
            .await
            .unwrap()
            .unwrap()
            .auto_include_new_instances
    );

    let inherits_false = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Backup".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://backup.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec!["gpt-4.1-mini".into()],
            included_in_main: None,
            created_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert!(!inherits_false.included_in_main);

    let updated_false = ModelProviderRepository::update_instance(
        &store,
        &UpdateModelProviderInstanceInput {
            instance_id: inherits_true.id,
            workspace_id: workspace.id,
            display_name: inherits_true.display_name.clone(),
            status: inherits_true.status,
            config_json: inherits_true.config_json.clone(),
            configured_models: inherits_true.configured_models.clone(),
            enabled_model_ids: inherits_true.enabled_model_ids.clone(),
            included_in_main: false,
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert!(!updated_false.included_in_main);

    let updated_true = ModelProviderRepository::update_instance(
        &store,
        &UpdateModelProviderInstanceInput {
            instance_id: inherits_false.id,
            workspace_id: workspace.id,
            display_name: inherits_false.display_name.clone(),
            status: inherits_false.status,
            config_json: inherits_false.config_json.clone(),
            configured_models: inherits_false.configured_models.clone(),
            enabled_model_ids: inherits_false.enabled_model_ids.clone(),
            included_in_main: true,
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert!(updated_true.included_in_main);
}

#[tokio::test]
async fn model_provider_repository_persists_main_model_distribution_rules_without_touching_instances(
) {
    // #1250 AC-007: retry_round_robin persists without rewriting provider instances.
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Primary".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://primary.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec!["fixture_chat".into(), "custom-stable".into()],
            included_in_main: Some(true),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    let record = ModelProviderRepository::upsert_main_instance(
        &store,
        &UpsertModelProviderMainInstanceInput {
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            auto_include_new_instances: true,
            model_distribution_rules: Some(vec![domain::ModelProviderMainModelDistributionRule {
                model_id: "fixture_chat".into(),
                distribution_rule: domain::ModelProviderDistributionRule::RetryRoundRobin,
            }]),
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(record.model_distribution_rules.len(), 1);
    assert_eq!(record.model_distribution_rules[0].model_id, "fixture_chat");
    assert_eq!(
        record.model_distribution_rules[0].distribution_rule,
        domain::ModelProviderDistributionRule::RetryRoundRobin
    );

    let fetched =
        ModelProviderRepository::get_main_instance(&store, workspace.id, "fixture_provider")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        fetched.model_distribution_rules,
        record.model_distribution_rules
    );

    let instance_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_name = 'model_provider_instances'
          and table_schema = current_schema()
        "#,
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert!(!instance_columns.contains(&"distribution_rule".to_string()));
    assert_eq!(
        ModelProviderRepository::get_instance(&store, workspace.id, instance.id)
            .await
            .unwrap()
            .unwrap()
            .enabled_model_ids,
        vec!["fixture_chat".to_string(), "custom-stable".to_string()]
    );
}

#[tokio::test]
async fn model_provider_repository_defaults_included_in_main_to_true_without_main_instance_row() {
    let (store, workspace, actor, installation_id) = seed_store().await;

    let created = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Implicit Include".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://implicit-include.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec!["gpt-4o-mini".into()],
            included_in_main: None,
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    assert!(created.included_in_main);
    assert!(
        ModelProviderRepository::get_main_instance(&store, workspace.id, "fixture_provider")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn model_provider_repository_backfills_main_instance_settings_when_upgrading_legacy_schema() {
    let (store, workspace, actor, installation_id) =
        seed_store_before_main_instance_aggregation().await;
    let primary_id = insert_legacy_instance(
        &store,
        workspace.id,
        installation_id,
        actor.id,
        "Primary",
        vec!["gpt-4o-mini".into()],
    )
    .await;
    let backup_id = insert_legacy_instance(
        &store,
        workspace.id,
        installation_id,
        actor.id,
        "Backup",
        vec!["gpt-4.1-mini".into()],
    )
    .await;

    sqlx::raw_sql(MAIN_INSTANCE_AGGREGATION_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::raw_sql(MAIN_MODEL_DISTRIBUTION_RULES_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();

    let main_instance =
        ModelProviderRepository::get_main_instance(&store, workspace.id, "fixture_provider")
            .await
            .unwrap()
            .unwrap();
    assert!(main_instance.auto_include_new_instances);
    let main_instance_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)::bigint
        from model_provider_main_instances
        where workspace_id = $1
          and provider_code = $2
        "#,
    )
    .bind(workspace.id)
    .bind("fixture_provider")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(main_instance_count, 1);

    let included_flags: Vec<bool> = sqlx::query_scalar(
        r#"
        select included_in_main
        from model_provider_instances
        where workspace_id = $1
          and id = any($2)
        order by display_name asc
        "#,
    )
    .bind(workspace.id)
    .bind(vec![primary_id, backup_id])
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(included_flags, vec![true, true]);

    let instances = ModelProviderRepository::list_instances(&store, workspace.id)
        .await
        .unwrap();
    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|instance| instance.included_in_main));
}

#[tokio::test]
async fn model_provider_repository_backfills_missing_context_window_override_tokens_when_upgrading_legacy_schema(
) {
    let (store, workspace, actor, installation_id) =
        seed_store_before_main_instance_aggregation().await;
    sqlx::raw_sql(MAIN_INSTANCE_AGGREGATION_MIGRATION_SQL)
        .execute(store.pool())
        .await
        .unwrap();
    let instance_id = insert_legacy_instance(
        &store,
        workspace.id,
        installation_id,
        actor.id,
        "Legacy",
        vec!["gpt-4o-mini".into(), "gpt-4.1-mini".into()],
    )
    .await;
    let migration_sql = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/20260423235000_add_model_provider_context_window_override.sql"),
    )
    .unwrap();

    sqlx::raw_sql(&migration_sql)
        .execute(store.pool())
        .await
        .unwrap();

    let instance = ModelProviderRepository::get_instance(&store, workspace.id, instance_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        instance.configured_models,
        vec![
            domain::ModelProviderConfiguredModel {
                model_id: "gpt-4o-mini".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
            domain::ModelProviderConfiguredModel {
                model_id: "gpt-4.1-mini".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            },
        ]
    );
}

#[tokio::test]
async fn model_provider_repository_lists_instances_from_instance_state_only() {
    let (store, workspace, actor, installation_id) = seed_store().await;

    let primary = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Primary".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://primary.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec!["gpt-4o-mini".into()],
            included_in_main: Some(true),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();
    let backup = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Backup".into(),
            status: ModelProviderInstanceStatus::Ready,
            config_json: json!({ "base_url": "https://backup.example.com/v1" }),
            configured_models: vec![],
            enabled_model_ids: vec!["gpt-4.1-mini".into()],
            included_in_main: Some(false),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    let listed = ModelProviderRepository::list_instances(&store, workspace.id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert!(
        listed
            .iter()
            .find(|instance| instance.id == primary.id)
            .unwrap()
            .included_in_main
    );
    assert!(
        !listed
            .iter()
            .find(|instance| instance.id == backup.id)
            .unwrap()
            .included_in_main
    );

    let listed_by_provider =
        ModelProviderRepository::list_instances_by_provider_code(&store, "fixture_provider")
            .await
            .unwrap();
    assert_eq!(listed_by_provider.len(), 2);
    assert!(
        listed_by_provider
            .iter()
            .find(|instance| instance.id == primary.id)
            .unwrap()
            .included_in_main
    );
    assert!(
        !listed_by_provider
            .iter()
            .find(|instance| instance.id == backup.id)
            .unwrap()
            .included_in_main
    );
}
