use super::*;

#[tokio::test]
async fn model_provider_repository_persists_catalog_sources_sync_runs_and_entries() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance = create_ready_instance(
        &store,
        workspace.id,
        actor.id,
        installation_id,
        "Catalog Instance",
        vec!["gpt-4o-mini".into()],
    )
    .await;
    let source_id = Uuid::now_v7();

    let source = ModelProviderRepository::create_catalog_source(
        &store,
        &CreateModelProviderCatalogSourceInput {
            source_id,
            workspace_id: workspace.id,
            source_kind: "runtime_extension".into(),
            plugin_id: "fixture_provider@0.1.0".into(),
            provider_code: "fixture_provider".into(),
            display_name: "Fixture Catalog".into(),
            base_url_ref: Some("secret://base-url".into()),
            auth_secret_ref: Some("secret://api-key".into()),
            protocol: "openai_compatible".into(),
            status: "active".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(source.id, source_id);
    assert_eq!(source.base_url_ref.as_deref(), Some("secret://base-url"));
    assert!(source.last_sync_run_id.is_none());

    let sync_run_id = Uuid::now_v7();
    let started_at = time::OffsetDateTime::now_utc();
    let finished_at = started_at + time::Duration::seconds(3);
    let sync_run = ModelProviderRepository::create_catalog_sync_run(
        &store,
        &CreateModelCatalogSyncRunInput {
            sync_run_id,
            catalog_source_id: source_id,
            status: "succeeded".into(),
            error_message_ref: None,
            discovered_count: 2,
            imported_count: 1,
            disabled_count: 1,
            started_at,
            finished_at: Some(finished_at),
        },
    )
    .await
    .unwrap();
    assert_eq!(sync_run.id, sync_run_id);
    assert_eq!(sync_run.imported_count, 1);

    let linked_sync_run_id: Uuid = sqlx::query_scalar(
        "select last_sync_run_id from model_provider_catalog_sources where id = $1",
    )
    .bind(source_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(linked_sync_run_id, sync_run_id);

    let entry = ModelProviderRepository::upsert_catalog_entry(
        &store,
        &UpsertModelProviderCatalogEntryInput {
            provider_instance_id: Some(instance.id),
            catalog_source_id: source_id,
            upstream_model_id: "gpt-4o-mini".into(),
            display_label: "GPT-4o mini".into(),
            protocol: "openai_compatible".into(),
            capability_snapshot: json!({ "chat": true }),
            parameter_schema_ref: Some("schema://openai-compatible".into()),
            context_window: Some(128_000),
            max_output_tokens: Some(16_384),
            pricing_ref: Some("pricing://gpt-4o-mini".into()),
            status: "active".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(entry.provider_instance_id, Some(instance.id));
    assert_eq!(entry.context_window, Some(128_000));

    let updated_entry = ModelProviderRepository::upsert_catalog_entry(
        &store,
        &UpsertModelProviderCatalogEntryInput {
            provider_instance_id: None,
            catalog_source_id: source_id,
            upstream_model_id: "gpt-4o-mini".into(),
            display_label: "GPT-4o mini updated".into(),
            protocol: "openai_compatible".into(),
            capability_snapshot: json!({ "chat": true, "vision": true }),
            parameter_schema_ref: None,
            context_window: Some(256_000),
            max_output_tokens: Some(32_768),
            pricing_ref: None,
            status: "deprecated".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated_entry.id, entry.id);
    assert_eq!(updated_entry.provider_instance_id, None);
    assert_eq!(updated_entry.display_label, "GPT-4o mini updated");

    let source_entries = ModelProviderRepository::list_catalog_entries(&store, source_id)
        .await
        .unwrap();
    assert_eq!(source_entries, vec![updated_entry.clone()]);

    let instance_entries =
        ModelProviderRepository::list_catalog_entries_for_provider_instance(&store, instance.id)
            .await
            .unwrap();
    assert!(instance_entries.is_empty());
}

#[tokio::test]
async fn model_provider_repository_persists_failover_queue_templates_items_and_snapshots() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let primary = create_ready_instance(
        &store,
        workspace.id,
        actor.id,
        installation_id,
        "Primary",
        vec!["gpt-4o-mini".into()],
    )
    .await;
    let backup = create_ready_instance(
        &store,
        workspace.id,
        actor.id,
        installation_id,
        "Backup",
        vec!["gpt-4.1-mini".into()],
    )
    .await;
    let queue_template_id = Uuid::now_v7();

    let template = ModelProviderRepository::create_failover_queue_template(
        &store,
        &CreateModelFailoverQueueTemplateInput {
            queue_template_id,
            workspace_id: workspace.id,
            name: "Production failover".into(),
            version: 1,
            status: "active".into(),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(template.id, queue_template_id);

    let fetched_template =
        ModelProviderRepository::get_failover_queue_template(&store, queue_template_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(fetched_template, template);

    ModelProviderRepository::create_failover_queue_item(
        &store,
        &CreateModelFailoverQueueItemInput {
            queue_item_id: Uuid::now_v7(),
            queue_template_id,
            sort_index: 2,
            provider_instance_id: backup.id,
            provider_code: "fixture_provider".into(),
            upstream_model_id: "gpt-4.1-mini".into(),
            protocol: "openai_compatible".into(),
            enabled: false,
        },
    )
    .await
    .unwrap();
    ModelProviderRepository::create_failover_queue_item(
        &store,
        &CreateModelFailoverQueueItemInput {
            queue_item_id: Uuid::now_v7(),
            queue_template_id,
            sort_index: 1,
            provider_instance_id: primary.id,
            provider_code: "fixture_provider".into(),
            upstream_model_id: "gpt-4o-mini".into(),
            protocol: "openai_compatible".into(),
            enabled: true,
        },
    )
    .await
    .unwrap();

    let items = ModelProviderRepository::list_failover_queue_items(&store, queue_template_id)
        .await
        .unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item.upstream_model_id.as_str())
            .collect::<Vec<_>>(),
        vec!["gpt-4o-mini", "gpt-4.1-mini"]
    );
    assert!(items[0].enabled);
    assert!(!items[1].enabled);

    let first_snapshot = ModelProviderRepository::create_failover_queue_snapshot(
        &store,
        &CreateModelFailoverQueueSnapshotInput {
            snapshot_id: Uuid::now_v7(),
            queue_template_id,
            version: 1,
            items: json!([{ "provider_instance_id": primary.id, "model": "gpt-4o-mini" }]),
        },
    )
    .await
    .unwrap();
    let second_snapshot = ModelProviderRepository::create_failover_queue_snapshot(
        &store,
        &CreateModelFailoverQueueSnapshotInput {
            snapshot_id: Uuid::now_v7(),
            queue_template_id,
            version: 2,
            items: json!([
                { "provider_instance_id": primary.id, "model": "gpt-4o-mini" },
                { "provider_instance_id": backup.id, "model": "gpt-4.1-mini" }
            ]),
        },
    )
    .await
    .unwrap();

    let snapshots =
        ModelProviderRepository::list_failover_queue_snapshots(&store, queue_template_id)
            .await
            .unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(
        snapshots
            .iter()
            .map(|snapshot| snapshot.version)
            .collect::<Vec<_>>(),
        vec![second_snapshot.version, first_snapshot.version]
    );
}

#[tokio::test]
async fn model_provider_repository_reassigns_all_instances_for_a_provider() {
    let (store, workspace, actor, installation_v1) = seed_store().await;
    let installation_v2 = control_plane::ports::PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@0.2.0".into(),
            plugin_version: "0.2.0".into(),
            contract_version: "1flowbase.provider/v1".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture Provider".into(),
            source_kind: "official_registry".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap()
    .id;
    let instance = ModelProviderRepository::create_instance(
        &store,
        &CreateModelProviderInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id: installation_v1,
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

    let moved = ModelProviderRepository::reassign_instances_to_installation(
        &store,
        &ReassignModelProviderInstancesInput {
            workspace_id: workspace.id,
            provider_code: "fixture_provider".into(),
            target_installation_id: installation_v2,
            target_protocol: "openai_compatible".into(),
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].id, instance.id);
    assert_eq!(moved[0].installation_id, installation_v2);
}
