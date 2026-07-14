use super::*;

#[tokio::test]
async fn ac_003_validate_instance_marks_ready_without_discovering_resources() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::ready();
    let service = DataSourceService::new(repository.clone(), runtime, "test-master-key");

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret" }),
        })
        .await
        .unwrap();

    let validated = service
        .validate_instance(ValidateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap();

    assert_eq!(validated.instance.status, DataSourceInstanceStatus::Ready);
    assert!(repository
        .cached_catalog(created.instance.id)
        .await
        .is_none());
    assert_eq!(
        repository.stored_secret_json(created.instance.id).await,
        json!({ "client_secret": "secret" })
    );
}

#[tokio::test]
async fn ac_004_only_ready_connections_discover_and_read_cached_resources() {
    let repository = InMemoryDataSourceRepository::default();
    let service = DataSourceService::new(
        repository.clone(),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    );
    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret" }),
        })
        .await
        .unwrap();

    let draft_error = service
        .discover_resources(DiscoverDataSourceResourcesCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap_err();
    assert!(draft_error.to_string().contains("invalid state transition"));

    service
        .validate_instance(ValidateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap();
    let discovered = service
        .discover_resources(DiscoverDataSourceResourcesCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap();
    assert_eq!(
        discovered.refresh_status,
        DataSourceCatalogRefreshStatus::Ready
    );
    assert_eq!(discovered.entries[0].resource_key, "contacts");

    let cached = service
        .list_resources(user_id(), workspace_id(), created.instance.id)
        .await
        .unwrap();
    assert_eq!(cached.entries, discovered.entries);
}

#[tokio::test]
async fn create_instance_requires_data_source_console_policy_not_legacy_permissions() {
    let state_model_actor = ActorContext::scoped_in_scope(
        user_id(),
        tenant_id(),
        workspace_id(),
        "member",
        ["state_model.manage.all".to_string()],
    );
    let denied_repository = InMemoryDataSourceRepository::with_actor(state_model_actor);
    let denied_service = DataSourceService::new(
        denied_repository,
        StubDataSourceRuntime::ready(),
        "test-master-key",
    );

    let denied = denied_service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({}),
        })
        .await
        .unwrap_err();
    assert!(denied.to_string().contains("permission_denied"));

    let legacy_actor = ActorContext::scoped_in_scope(
        user_id(),
        tenant_id(),
        workspace_id(),
        "member",
        ["external_data_source.configure.all".to_string()],
    );
    let legacy_service = DataSourceService::new(
        InMemoryDataSourceRepository::with_actor(legacy_actor),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    );
    let legacy_denied = legacy_service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "Legacy HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({}),
        })
        .await
        .unwrap_err();
    assert!(legacy_denied.to_string().contains("permission_denied"));

    let policy_actor =
        ActorContext::scoped_in_scope(user_id(), tenant_id(), workspace_id(), "member", []);
    let allowed_repository = InMemoryDataSourceRepository::with_actor(policy_actor);
    allowed_repository
        .set_console_policies(vec![domain::RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![domain::RoleConsoleGroupPolicy::custom(
                domain::ConsolePolicyGroup::settings_feature(
                    access_control::SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID,
                )
                .unwrap(),
                vec![domain::ConsoleOperationPolicy::simple(
                    domain::ConsoleOperationId::try_from(
                        access_control::DATA_SOURCES_CREATE_OPERATION_ID,
                    )
                    .unwrap(),
                    true,
                )],
            )],
        )])
        .await;
    let allowed_service = DataSourceService::new(
        allowed_repository.clone(),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    );

    let created = allowed_service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({}),
        })
        .await
        .unwrap();
    assert_eq!(created.instance.display_name, "HubSpot");
    assert!(created.instance.secret_ref.is_none());
    assert!(created.instance.secret_version.is_none());
    assert!(allowed_repository
        .get_secret_record(created.instance.id)
        .await
        .unwrap()
        .is_none());
    let created_audit = allowed_repository
        .audit_events()
        .await
        .into_iter()
        .find(|event| event.event_code == "data_source.instance_created")
        .unwrap();
    assert!(created_audit.payload["secret_ref"].is_null());
    assert!(created_audit.payload["secret_version"].is_null());
}
