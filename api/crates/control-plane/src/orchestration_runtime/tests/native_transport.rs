use super::*;

#[tokio::test]
async fn native_provider_transport_payload_restores_the_ephemeral_invocation_capability() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let invoker = RuntimeProviderInvoker {
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository,
        runtime: runtime_port,
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: Some(Uuid::nil()),
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        transport_connection_scope_override: None,
        provider_transport_payload: Some(
            crate::ports::ProviderTransportPayload::openai_responses(json!({
                "model": "gpt-5.4-mini",
                "input": "native transport"
            }))
            .unwrap(),
        ),
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "native transport".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    };

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker, &runtime, input,
    )
    .await
    .expect("ephemeral native transport should reach the provider invocation");

    // AC-003: billing and timing wrappers must not discard the native round binding.
    let mut metadata = &output.result.provider_metadata;
    while let Some(upstream) = metadata.get("_1flowbase_upstream_provider_metadata") {
        metadata = upstream;
    }
    assert_eq!(
        metadata["native_response"]["response_id"],
        format!("resp_{}", Uuid::nil())
    );
    assert!(metadata["native_response"]["configuration_digest"].is_string());

    let captured = captured_inputs
        .lock()
        .expect("captured provider inputs should be readable");
    assert!(captured[0].native_transport.is_some());
    assert!(captured[0]
        .required_capabilities
        .contains(&ProviderInvocationCapability::ResponsesNativePassthrough));
}

#[tokio::test]
async fn native_provider_transport_affinity_rejects_a_different_selected_llm_before_invocation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let affinity = crate::ports::ProviderTransportAffinity::new(
        Uuid::now_v7().to_string(),
        "other_provider",
        "openai_compatible",
        "gpt-5.4-mini",
    );
    let invoker = RuntimeProviderInvoker {
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository,
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
        transport_connection_scope_override: None,
        provider_transport_payload: Some(
            crate::ports::ProviderTransportPayload::openai_responses(json!({
                "model": "gpt-5.4-mini",
                "input": "opaque continuation"
            }))
            .unwrap()
            .with_affinity(affinity),
        ),
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "continue".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    };

    let error = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker, &runtime, input,
    )
    .await
    .expect_err("a different selected Provider must not receive opaque continuation state");
    let runtime_error = error
        .downcast_ref::<plugin_framework::PluginFrameworkError>()
        .and_then(|error| match error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => Some(error),
            _ => None,
        })
        .expect("affinity mismatch should remain a typed runtime error");
    assert_eq!(
        runtime_error.kind,
        ProviderRuntimeErrorKind::ProviderAffinityMismatch
    );
    assert!(captured_inputs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn issue_1743_bound_native_continuation_sends_only_sealed_delta_wire() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let affinity = crate::ports::ProviderTransportAffinity::new(
        provider_instance_id.to_string(),
        "fixture_provider",
        "openai_compatible",
        "gpt-5.4-mini",
    );
    let continuation = crate::ports::ProviderContinuation::new("resp-bound", affinity).unwrap();
    let payload = crate::ports::ProviderTransportPayload::openai_responses(json!({
        "model": "gpt-5.4-mini",
        "input": [{"type": "function_call_output", "call_id": "call-1", "output": "ok"}]
    }))
    .unwrap()
    .bind_openai_continuation(continuation)
    .unwrap();
    let invoker = RuntimeProviderInvoker {
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository,
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        transport_connection_scope_override: None,
        provider_transport_payload: Some(payload),
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let pipelined =
        orchestration_runtime::execution_engine::ProviderInvoker::pipeline_provider_input(
            &invoker,
            provider_user_input(provider_instance_id),
        )
        .await
        .unwrap();
    assert!(pipelined
        .input
        .required_capabilities
        .contains(&ProviderInvocationCapability::NativeContinuationSupported));
    let mut input = pipelined.input;

    invoker
        .apply_provider_transport(&runtime, &mut input)
        .unwrap();

    assert!(input.messages.is_empty());
    assert!(input.system.is_empty());
    assert!(input.tools.is_empty());
    assert!(input
        .required_capabilities
        .contains(&ProviderInvocationCapability::NativeContinuationSupported));
    let native = input
        .native_transport
        .expect("sealed native delta must be attached");
    assert_eq!(
        native.wire_body["previous_response_id"],
        json!("resp-bound")
    );
    assert_eq!(
        native.wire_body["input"][0]["type"],
        json!("function_call_output")
    );
}

#[tokio::test]
async fn qf6_initial_scope_keeps_canonical_context_valid_until_actual_invoker_boundary() {
    use std::collections::BTreeMap;

    use crate::application_public_api::native::NativeRunRequest;
    use plugin_framework::provider_contract::{
        validate_protocol_context_envelope, ProtocolContextEnvelope,
    };

    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let mut request: NativeRunRequest =
        serde_json::from_value(json!({"query":"scope boundary"})).unwrap();
    let canonical = ProtocolContextEnvelope {
        source_protocol: "openai_responses".into(),
        headers: BTreeMap::from([("session-id".into(), vec!["logical-thread".into()])]),
        ..Default::default()
    };
    request.client_protocol_envelope = Some(canonical.clone());
    request
        .metadata
        .set_transport_connection_scope(Some("host-initial-socket".into()));
    validate_protocol_context_envelope(request.client_protocol_envelope.as_ref().unwrap())
        .expect("host execution metadata must not contaminate canonical protocol validation");
    let serialized = serde_json::to_value(&request).unwrap();
    assert!(!serialized.to_string().contains("host-initial-socket"));
    let command = StartPublishedFlowRunCommand {
        application_id: Uuid::nil(),
        flow_run_id: Uuid::nil(),
        provider_transport_slot: None,
        transport_connection_scope: request.metadata.take_transport_connection_scope(),
    };
    assert!(request.metadata.take_transport_connection_scope().is_none());
    let invoker = RuntimeProviderInvoker {
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository,
        runtime: runtime_port,
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".into(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: Some(command.flow_run_id),
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".into()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        transport_connection_scope_override: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    }
    .with_transport_connection_scope_override(command.transport_connection_scope);
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let mut input = provider_user_input(provider_instance_id);
    input.client_protocol_envelope = request.client_protocol_envelope;
    orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(&invoker, &runtime, input)
        .await
        .expect("initial execution scope must reach the real Provider invoker");
    let captured = captured_inputs.lock().unwrap();
    assert_eq!(captured.len(), 1);
    let mut late_context = captured[0].client_protocol_envelope.clone().unwrap();
    assert_eq!(
        late_context
            .headers
            .remove(HOST_TRANSPORT_CONNECTION_SCOPE_HEADER),
        Some(vec!["host-initial-socket".into()])
    );
    assert_eq!(late_context, canonical);
    validate_protocol_context_envelope(&late_context).unwrap();
    // Controlled negative reproduces QA5's actual canonical contract violation.
    late_context.headers.insert(
        HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
        vec!["old-ingress-injection".into()],
    );
    assert!(validate_protocol_context_envelope(&late_context).is_err());
}
