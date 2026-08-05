use super::*;

#[tokio::test]
async fn publisher_cutover_agent_flow_route_reads_receipt_marked_legacy_provider_package() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id = repository
        .get_instance(Uuid::nil(), provider_instance_id)
        .await
        .unwrap()
        .unwrap()
        .installation_id;
    repository
        .mark_provider_manifest_legacy_missing_publisher_namespace(installation_id)
        .await;
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");

    let resolved = orchestration_runtime::execution_engine::ProviderInvoker::resolve_llm_route(
        &invoker, &runtime,
    )
    .await
    .expect("AC-002 Agent Flow route should read the legacy installed provider package");

    assert!(resolved.runtime_capabilities.is_empty());
}

#[tokio::test]
async fn root_1534_resolved_provider_route_pins_the_installation_used_for_invocation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id = repository
        .get_instance(Uuid::nil(), provider_instance_id)
        .await
        .expect("provider lookup should succeed")
        .expect("provider instance should exist")
        .installation_id;
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let invoker = RuntimeProviderInvoker {
        repository: repository.clone(),
        runtime: runtime_port,
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let resolved = orchestration_runtime::execution_engine::ProviderInvoker::resolve_llm_route(
        &invoker, &runtime,
    )
    .await
    .expect("current Provider installation should resolve");

    repository.set_installation_state(
        installation_id,
        domain::PluginDesiredState::Disabled,
        domain::PluginAvailabilityStatus::Disabled,
    );

    orchestration_runtime::execution_engine::ProviderInvoker::invoke_resolved_llm(
        &invoker,
        &runtime,
        resolved,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("an already resolved attempt must use its pinned installation generation");
    assert_eq!(captured_inputs.lock().unwrap().len(), 1);

    assert!(
        orchestration_runtime::execution_engine::ProviderInvoker::resolve_llm_route(
            &invoker, &runtime,
        )
        .await
        .is_err(),
        "the next attempt must observe the disabled current installation"
    );
}
