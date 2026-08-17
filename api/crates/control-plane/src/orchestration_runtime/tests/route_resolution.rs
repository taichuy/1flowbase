use super::*;

#[tokio::test]
async fn main_instance_routing_uses_current_registered_instances_instead_of_frozen_target() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (frozen_instance_id, current_instance_id) = repository.seed_included_provider_instances();
    repository.set_instance_included_in_main(frozen_instance_id, false);
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
        model_pricing_cache_store: None,
    };
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: frozen_instance_id.to_string(),
        provider_instance_display_name: "Frozen".to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: Some(orchestration_runtime::compiled_plan::CompiledLlmRouting {
            routing_mode: orchestration_runtime::compiled_plan::LlmRoutingMode::FixedModel,
            fixed_model_target: None,
            queue_template_id: None,
            queue_snapshot_id: None,
            queue_targets: Vec::new(),
            distribution_rule: Default::default(),
            distribution_key: None,
            context_policy: serde_json::json!({ "integration_context": "enabled" }),
            stream_policy: serde_json::json!({}),
        }),
    };

    let resolved =
        orchestration_runtime::execution_engine::ProviderInvoker::resolve_main_llm_routing(
            &invoker, &runtime,
        )
        .await
        .expect("main routing should resolve")
        .expect("ordinary LLM runtime should target main instance");

    assert_eq!(resolved.candidates.len(), 1);
    assert_eq!(
        resolved.candidates[0].runtime.provider_instance_id,
        current_instance_id.to_string()
    );
}

#[tokio::test]
async fn main_instance_routing_reads_current_distribution_rule_and_order() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (alpha_instance_id, backup_instance_id) = repository.seed_included_provider_instances();
    repository.set_main_model_routing_policy(
        "fixture_provider",
        "gpt-5.4-mini",
        domain::ModelProviderDistributionRule::RetryRoundRobin,
        vec![backup_instance_id, alpha_instance_id],
        Vec::new(),
    );
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
        model_pricing_cache_store: None,
    };
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: String::new(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: String::new(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    };

    let resolved =
        orchestration_runtime::execution_engine::ProviderInvoker::resolve_main_llm_routing(
            &invoker, &runtime,
        )
        .await
        .expect("main routing should resolve")
        .expect("logical runtime should target main instance");

    assert_eq!(
        resolved.distribution_rule,
        orchestration_runtime::compiled_plan::LlmDistributionRule::RetryRoundRobin
    );
    assert_eq!(
        resolved
            .candidates
            .iter()
            .map(|candidate| candidate.runtime.provider_instance_id.clone())
            .collect::<Vec<_>>(),
        vec![
            backup_instance_id.to_string(),
            alpha_instance_id.to_string()
        ]
    );
    let routing = resolved.candidates[0]
        .runtime
        .routing
        .as_ref()
        .expect("resolved runtime should carry auditable main routing facts");
    assert_eq!(
        routing.distribution_rule,
        orchestration_runtime::compiled_plan::LlmDistributionRule::RetryRoundRobin
    );
    assert_eq!(
        routing.fixed_model_target.as_ref().unwrap()["routing_owner"],
        serde_json::json!("main_instance")
    );
    assert_eq!(
        routing.fixed_model_target.as_ref().unwrap()["main_instance_revision"],
        serde_json::json!(1)
    );
}

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
        model_pricing_cache_store: None,
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
        model_pricing_cache_store: None,
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
