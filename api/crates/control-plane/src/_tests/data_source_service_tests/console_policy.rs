use super::*;

fn data_models_policy_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID,
    )
    .expect("compiled data models settings feature id must be valid")
}

fn data_source_operation_id(value: &str) -> domain::ConsoleOperationId {
    domain::ConsoleOperationId::try_from(value)
        .expect("compiled data source operation id must be valid")
}

fn data_models_console_policy(
    operations: Vec<domain::ConsoleOperationPolicy>,
) -> domain::RoleConsolePolicy {
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            data_models_policy_group(),
            operations,
        )],
    )
}

async fn seed_instance(
    repository: &InMemoryDataSourceRepository,
    workspace_id: Uuid,
    created_by: Uuid,
    status: DataSourceInstanceStatus,
) -> DataSourceInstanceRecord {
    DataSourceRepository::create_instance(
        repository,
        &CreateDataSourceInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id,
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: format!("HubSpot-{created_by}"),
            status,
            config_json: json!({ "client_id": "abc" }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn ac_005_data_source_view_own_filters_instance_enumeration_and_resource_reads() {
    let actor_user_id = user_id();
    let peer_user_id = Uuid::from_u128(0x301);
    let foreign_workspace_id = Uuid::from_u128(0x201);
    let actor =
        ActorContext::scoped_in_scope(actor_user_id, tenant_id(), workspace_id(), "member", []);
    let repository = InMemoryDataSourceRepository::with_actor(actor);
    repository
        .set_console_policies(vec![data_models_console_policy(vec![
            domain::ConsoleOperationPolicy::simple(
                data_source_operation_id(access_control::DATA_SOURCES_LIST_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::row(
                data_source_operation_id(access_control::DATA_SOURCES_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
        ])])
        .await;
    let own = seed_instance(
        &repository,
        workspace_id(),
        actor_user_id,
        DataSourceInstanceStatus::Ready,
    )
    .await;
    let peer = seed_instance(
        &repository,
        workspace_id(),
        peer_user_id,
        DataSourceInstanceStatus::Ready,
    )
    .await;
    let foreign = seed_instance(
        &repository,
        foreign_workspace_id,
        peer_user_id,
        DataSourceInstanceStatus::Ready,
    )
    .await;
    let service = DataSourceService::for_data_model_settings(
        repository.clone(),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    );

    let data_sources = service
        .list_data_sources(actor_user_id, workspace_id())
        .await
        .unwrap();
    let runtime_extension_ids = data_sources
        .into_iter()
        .filter_map(|data_source| match data_source.backend {
            crate::data_source::DataSourceBackendView::RuntimeExtension(view) => {
                Some(view.instance.id)
            }
            crate::data_source::DataSourceBackendView::Core { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(runtime_extension_ids, vec![own.id]);

    let peer_error = service
        .list_resources(actor_user_id, workspace_id(), peer.id)
        .await
        .unwrap_err();
    assert!(peer_error.to_string().contains("data_source_instance"));

    let cross_workspace_error = service
        .list_resources(actor_user_id, foreign_workspace_id, foreign.id)
        .await
        .unwrap_err();
    assert!(cross_workspace_error.to_string().contains("workspace_id"));
}

#[tokio::test]
async fn ac_007_data_source_simple_console_operations_do_not_fall_back_to_legacy_grants() {
    let actor = ActorContext::scoped_in_scope(user_id(), tenant_id(), workspace_id(), "member", []);
    let policy_repository = InMemoryDataSourceRepository::with_actor(actor);
    policy_repository
        .set_console_policies(vec![data_models_console_policy(vec![
            domain::ConsoleOperationPolicy::simple(
                data_source_operation_id(access_control::DATA_SOURCES_CREATE_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                data_source_operation_id(access_control::DATA_SOURCES_DISCOVER_OPERATION_ID),
                true,
            ),
        ])])
        .await;
    let service = DataSourceService::for_data_model_settings(
        policy_repository.clone(),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    )
    .with_node_artifact_context("test-node", env!("CARGO_MANIFEST_DIR"));

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "Policy Only HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "policy-only-secret" }),
        })
        .await
        .unwrap();
    assert_eq!(created.instance.created_by, user_id());
    assert_eq!(
        policy_repository
            .stored_secret_json(created.instance.id)
            .await,
        json!({ "client_secret": "policy-only-secret" })
    );

    let draft_error = service
        .discover_resources(DiscoverDataSourceResourcesCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap_err();
    assert!(draft_error.to_string().contains("data_source_instance"));

    let legacy_actor = ActorContext::scoped_in_scope(
        user_id(),
        tenant_id(),
        workspace_id(),
        "member",
        [access_control::SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION.to_string()],
    );
    let legacy_service = DataSourceService::for_data_model_settings(
        InMemoryDataSourceRepository::with_actor(legacy_actor),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    )
    .with_node_artifact_context("test-node", env!("CARGO_MANIFEST_DIR"));
    let legacy_error = legacy_service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "Legacy Feature HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({}),
        })
        .await
        .unwrap_err();
    assert!(legacy_error.to_string().contains("permission_denied"));
}
