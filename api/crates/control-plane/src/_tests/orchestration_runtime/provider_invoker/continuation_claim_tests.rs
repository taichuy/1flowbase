use super::*;
use async_trait::async_trait;
use plugin_framework::provider_contract::ProviderMessage;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Default)]
struct Issue1743ContinuationStore {
    continuation: Mutex<
        std::collections::HashMap<
            crate::ports::ProviderContinuationSlotId,
            crate::ports::ProviderContinuation,
        >,
    >,
}

#[async_trait]
impl crate::ports::ProviderTransportStore for Issue1743ContinuationStore {
    async fn put(
        &self,
        _slot_id: crate::ports::ProviderTransportSlotId,
        _payload: crate::ports::ProviderTransportPayload,
    ) -> anyhow::Result<()> {
        unreachable!("continuation fixture does not stage native payload slots")
    }

    async fn get(
        &self,
        _slot_id: crate::ports::ProviderTransportSlotId,
    ) -> anyhow::Result<Option<crate::ports::ProviderTransportPayload>> {
        unreachable!("continuation fixture does not read native payload slots")
    }

    async fn delete(
        &self,
        _slot_id: crate::ports::ProviderTransportSlotId,
    ) -> anyhow::Result<bool> {
        unreachable!("continuation fixture does not delete native payload slots")
    }

    async fn put_protocol_context(
        &self,
        _slot_id: crate::ports::ProviderProtocolContextSlotId,
        _value: crate::ports::ProviderProtocolContextValue,
    ) -> anyhow::Result<()> {
        unreachable!("continuation fixture does not stage protocol contexts")
    }

    async fn get_protocol_context(
        &self,
        _slot_id: crate::ports::ProviderProtocolContextSlotId,
    ) -> anyhow::Result<Option<crate::ports::ProviderProtocolContextValue>> {
        unreachable!("continuation fixture does not read protocol contexts")
    }

    async fn delete_flow_run_protocol_contexts(&self, _flow_run_id: Uuid) -> anyhow::Result<usize> {
        unreachable!("continuation fixture does not delete protocol contexts")
    }

    async fn put_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
        continuation: crate::ports::ProviderContinuation,
    ) -> anyhow::Result<()> {
        self.continuation.lock().await.insert(slot_id, continuation);
        Ok(())
    }

    async fn get_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<crate::ports::ProviderContinuation>> {
        Ok(self.continuation.lock().await.get(&slot_id).cloned())
    }

    async fn consume_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
    ) -> anyhow::Result<crate::ports::ProviderContinuation> {
        self.continuation
            .lock()
            .await
            .remove(&slot_id)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))
    }

    async fn claim_continuation(
        &self,
        flow_run_id: Uuid,
        resume_claim_id: Uuid,
    ) -> anyhow::Result<crate::ports::ProviderContinuation> {
        let current = crate::ports::ProviderContinuationSlotId::for_flow_run(flow_run_id);
        let claimed = crate::ports::ProviderContinuationSlotId::for_resume_claim(
            flow_run_id,
            resume_claim_id,
        );
        let mut continuations = self.continuation.lock().await;
        if let Some(continuation) = continuations.get(&claimed) {
            return Ok(continuation.clone());
        }
        let continuation = continuations
            .remove(&current)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))?;
        continuations.insert(claimed, continuation.clone());
        Ok(continuation)
    }

    async fn delete_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        Ok(self.continuation.lock().await.remove(&slot_id).is_some())
    }
}

fn issue_1743_runtime(
    provider_instance_id: Uuid,
) -> orchestration_runtime::compiled_plan::CompiledLlmRuntime {
    orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    }
}

fn issue_1743_affinity(provider_instance_id: Uuid) -> crate::ports::ProviderTransportAffinity {
    crate::ports::ProviderTransportAffinity::new(
        provider_instance_id.to_string(),
        "fixture_provider",
        "openai_compatible",
        "gpt-5.4-mini",
    )
}

fn issue_1743_invoker(
    flow_run_id: Uuid,
    store: Arc<Issue1743ContinuationStore>,
    continuation: Option<crate::ports::ProviderContinuation>,
) -> RuntimeProviderInvoker<
    test_support::InMemoryOrchestrationRuntimeRepository,
    test_support::InMemoryProviderRuntime,
> {
    RuntimeProviderInvoker {
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository: test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]),
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: Some(flow_run_id),
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: None,
        provider_install_root: None,
        provider_session_scope: None,
        flow_execution_context: None,
        answer_presentation: None,
        transport_connection_scope_override: None,
        observation_context: None,
        provider_transport_payload: None,
        provider_transport_store: Some(store),
        provider_continuation: continuation,
        model_pricing_cache_store: None,
    }
}

#[tokio::test]
async fn issue_1743_atomic_claim_owned_copy_survives_probe_and_actual_rebuild() {
    let provider_instance_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let slot = crate::ports::ProviderContinuationSlotId::for_flow_run(flow_run_id);
    let continuation = crate::ports::ProviderContinuation::new(
        "resp-one-shot",
        issue_1743_affinity(provider_instance_id),
    )
    .unwrap();
    let store = Arc::new(Issue1743ContinuationStore::default());
    crate::ports::ProviderTransportStore::put_continuation(
        store.as_ref(),
        slot,
        continuation.clone(),
    )
    .await
    .unwrap();
    let claimed = crate::ports::ProviderTransportStore::consume_continuation(store.as_ref(), slot)
        .await
        .expect("execution segment must atomically own the continuation");
    let invoker = issue_1743_invoker(flow_run_id, store.clone(), Some(claimed));
    let runtime = issue_1743_runtime(provider_instance_id);
    let mut input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Tool,
            content: "tool delta".to_string(),
            name: None,
            tool_call_id: Some("call-1".to_string()),
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    };

    let mut routing_probe = input.clone();
    invoker
        .apply_provider_transport(&runtime, &mut routing_probe)
        .unwrap();
    invoker
        .apply_provider_transport(&runtime, &mut input)
        .unwrap();

    assert_eq!(input.previous_response_id.as_deref(), Some("resp-one-shot"));
    assert_eq!(
        routing_probe.previous_response_id.as_deref(),
        Some("resp-one-shot")
    );
    assert_eq!(input.messages.len(), 1);
    assert!(input.required_capabilities.contains(
        &plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported
    ));
    assert!(
        crate::ports::ProviderTransportStore::get_continuation(store.as_ref(), slot)
            .await
            .unwrap()
            .is_none()
    );

    assert!(
        crate::ports::ProviderTransportStore::consume_continuation(store.as_ref(), slot)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn issue_1743_provider_continuation_is_staged_before_late_billing_classification() {
    let provider_instance_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let store = Arc::new(Issue1743ContinuationStore::default());
    let invoker = issue_1743_invoker(
        flow_run_id,
        store.clone(),
        Some(
            crate::ports::ProviderContinuation::new(
                "resp-staged",
                issue_1743_affinity(provider_instance_id),
            )
            .unwrap(),
        ),
    );

    invoker
        .stage_provider_continuation(&issue_1743_runtime(provider_instance_id), Some("resp-next"))
        .await
        .expect("provider continuation must be staged before billing classification");

    assert_eq!(
        crate::ports::ProviderTransportStore::get_continuation(
            store.as_ref(),
            crate::ports::ProviderContinuationSlotId::for_flow_run(flow_run_id),
        )
        .await
        .unwrap()
        .unwrap()
        .response_id(),
        "resp-next"
    );
}

#[tokio::test]
async fn issue_1743_provider_response_id_is_staged_without_existing_transport_state() {
    let provider_instance_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let store = Arc::new(Issue1743ContinuationStore::default());
    let invoker = issue_1743_invoker(flow_run_id, store.clone(), None);

    invoker
        .stage_provider_continuation(&issue_1743_runtime(provider_instance_id), Some("resp-mcp"))
        .await
        .expect("provider response id must be staged for a later MCP approval turn");

    assert_eq!(
        crate::ports::ProviderTransportStore::get_continuation(
            store.as_ref(),
            crate::ports::ProviderContinuationSlotId::for_flow_run(flow_run_id),
        )
        .await
        .unwrap()
        .unwrap()
        .response_id(),
        "resp-mcp"
    );
}

#[test]
fn issue_1743_continuation_route_requires_affinity_and_native_capability() {
    let provider_instance_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let continuation = crate::ports::ProviderContinuation::new(
        "resp-capability",
        issue_1743_affinity(provider_instance_id),
    )
    .unwrap();
    let invoker = issue_1743_invoker(
        flow_run_id,
        Arc::new(Issue1743ContinuationStore::default()),
        Some(continuation),
    );
    let runtime = issue_1743_runtime(provider_instance_id);
    let capability = plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported
        .manifest_capability_name()
        .to_string();

    let error = invoker
        .ensure_continuation_route(&runtime, &std::collections::BTreeSet::new())
        .expect_err("continuation route without native capability must fail closed");
    assert!(matches!(
        error.downcast_ref::<plugin_framework::PluginFrameworkError>(),
        Some(plugin_framework::PluginFrameworkError::RuntimeContract { error })
            if error.kind == ProviderRuntimeErrorKind::SemanticCapabilityUnsupported
    ));
    invoker
        .ensure_continuation_route(&runtime, &std::collections::BTreeSet::from([capability]))
        .expect("matching affinity with native capability is legal");
}

#[tokio::test]
async fn warmup_evidence_crosses_flow_then_extends_through_three_tool_rounds() {
    use crate::_tests::validate_full_retry_input;
    use crate::application_public_api::compat::openai::history::completed_history;
    use crate::ports::{
        ProviderContinuationSlotId, ProviderTransportPayload, ProviderTransportStore,
    };
    let provider_id = Uuid::now_v7();
    let warm_flow = Uuid::now_v7();
    let store = Arc::new(Issue1743ContinuationStore::default());
    let warm_invoker = issue_1743_invoker(warm_flow, store.clone(), None);
    let first_input = json!([{"role":"user","content":"perform three tools"}]);
    let warm = completed_history(&json!({"generate":false,"input":first_input}), None, &[])
        .unwrap()
        .unwrap();
    warm_invoker
        .stage_provider_continuation_with_history(
            &issue_1743_runtime(provider_id),
            Some("resp_warm"),
            Some(warm),
            None,
        )
        .await
        .unwrap();
    let continuation = store
        .consume_continuation(ProviderContinuationSlotId::for_flow_run(warm_flow))
        .await
        .unwrap();
    let bound = ProviderTransportPayload::openai_responses(
        json!({"previous_response_id":"resp_public","input":[]}),
    )
    .unwrap()
    .bind_openai_continuation(continuation)
    .unwrap();
    let mut invoker = issue_1743_invoker(Uuid::now_v7(), store.clone(), None)
        .with_provider_transport_payload(Some(bound));
    let mut full = first_input.as_array().unwrap().clone();
    for round in 0..3 {
        let call_id = format!("call_{round}");
        let call = json!({"type":"function_call","call_id":call_id,"name":"exec","arguments":"{}"});
        let transport = invoker.provider_transport_payload.as_ref().unwrap();
        let history = completed_history(
            transport.wire_body(),
            invoker.native_history.as_ref(),
            std::slice::from_ref(&call),
        )
        .unwrap()
        .unwrap();
        full.push(call);
        let result = json!({"type":"function_call_output","call_id":call_id,"output":"once"});
        full.push(result.clone());
        validate_full_retry_input(&json!(full), &history, &[call_id]).unwrap();
        invoker
            .stage_provider_continuation_with_history(
                &issue_1743_runtime(provider_id),
                Some("resp_next"),
                Some(history),
                None,
            )
            .await
            .unwrap();
        let continuation = store
            .consume_continuation(ProviderContinuationSlotId::for_flow_run(
                invoker.flow_run_id.unwrap(),
            ))
            .await
            .unwrap();
        invoker = invoker.with_provider_transport_payload(Some(
            ProviderTransportPayload::openai_responses(
                json!({"previous_response_id":"resp_public","input":[result]}),
            )
            .unwrap()
            .bind_openai_continuation(continuation)
            .unwrap(),
        ));
    }
}

#[test]
fn sparse_completed_output_indexes_keep_a_provable_native_history() {
    use crate::application_public_api::compat::openai::history::{
        completed_history, prove_full_context_input,
    };

    let user = json!({"role":"user","content":"run a long task"});
    let first_call =
        json!({"type":"function_call","call_id":"call_first","name":"exec","arguments":"{}"});
    let previous = completed_history(
        &json!({"input":[user]}),
        None,
        std::slice::from_ref(&first_call),
    )
    .unwrap()
    .unwrap();
    let first_result = json!({"type":"function_call_output","call_id":"call_first","output":"ok"});
    let reasoning =
        json!({"type":"reasoning","id":"rs_next","summary":[],"encrypted_content":"opaque"});
    let next_call =
        json!({"type":"function_call","call_id":"call_next","name":"exec","arguments":"{}"});
    let mut slots = std::collections::BTreeMap::from([
        (0, Some(reasoning.clone())),
        (2, Some(next_call.clone())),
    ]);
    let ordered = completed_native_output_items(&slots).expect("sparse completed items are valid");
    assert_eq!(ordered, [reasoning.clone(), next_call.clone()]);

    let history = completed_history(
        &json!({"previous_response_id":"resp_first","input":[first_result]}),
        Some(&previous),
        &ordered,
    )
    .unwrap()
    .expect("the completed response must retain history evidence");
    let full_context = json!([
        user,
        first_call,
        first_result,
        reasoning,
        next_call,
        {"type":"function_call_output","call_id":"call_next","output":"done"},
        {"role":"user","content":"continue with refreshed tools"}
    ]);
    prove_full_context_input(&full_context, &history, &["call_next".into()])
        .expect("a later configuration refresh can prove the predecessor history");

    slots.insert(1, None);
    assert!(
        completed_native_output_items(&slots).is_none(),
        "an announced but unfinished item cannot establish history"
    );
}

#[test]
fn exact_native_replay_is_authorized_only_by_the_owned_matching_snapshot() {
    use orchestration_runtime::execution_engine::ProviderInvoker;
    let runtime = issue_1743_runtime(Uuid::now_v7());
    let base = issue_1743_invoker(
        Uuid::now_v7(),
        Arc::new(Issue1743ContinuationStore::default()),
        None,
    );
    let sealed = crate::ports::ProviderTransportPayload::openai_responses(json!({
        "input":[{"type":"reasoning","encrypted_content":"opaque"},
            {"role":"user","content":"question"}]
    }))
    .unwrap();
    let owner = base.with_provider_transport_payload(Some(sealed));
    let mut input = ProviderInvocationInput::default();
    owner
        .apply_provider_transport(&runtime, &mut input)
        .unwrap();
    assert!(owner.permits_selected_native_exact_replay(&input));
    for (source, body, expected) in [
        (
            "openai_responses",
            json!({"conversation":"conv_external"}),
            false,
        ),
        (
            "openai_responses",
            json!({"conversation":{"id":"conv_external"}}),
            false,
        ),
        ("openai_responses", json!({"conversation":"null"}), false),
        (
            "openai_responses",
            json!({"previous_response_id":"resp_external"}),
            false,
        ),
        (
            "openai_responses",
            json!({"previous_response_id":"null"}),
            false,
        ),
        (
            "openai_responses",
            json!({"previous_response_id":null,"conversation":null}),
            true,
        ),
        (
            "openai_responses",
            json!({"future_extension":{"supported":"opaque"}}),
            true,
        ),
        // Foreign envelopes are ignored by the actual Responses plugin restore boundary.
        (
            "openai_chat",
            json!({"previous_response_id":"resp_foreign","conversation":"conv_foreign"}),
            true,
        ),
    ] {
        let mut with_envelope = input.clone();
        with_envelope.client_protocol_envelope = Some(
            serde_json::from_value(json!({
                "source_protocol":source, "body":body,
                "headers":{"conversation":["legal-header-extension"]},
                "query":{"conversation":["legal-query-extension"]}
            }))
            .unwrap(),
        );
        assert_eq!(
            owner.permits_selected_native_exact_replay(&with_envelope),
            expected,
            "source={source}; body={body}"
        );
    }
    assert!(
        !base.permits_selected_native_exact_replay(&input),
        "matching claims without ownership are insufficient"
    );
    let mut missing = input.clone();
    missing.native_transport = None;
    assert!(
        !owner.permits_selected_native_exact_replay(&missing),
        "capability-only is insufficient"
    );
    let mut cursor = input.clone();
    cursor.previous_response_id = Some("resp_canonical".into());
    assert!(!owner.permits_selected_native_exact_replay(&cursor));
    for mutation in 0..4 {
        let mut changed = input.clone();
        let native = changed.native_transport.as_mut().unwrap();
        match mutation {
            0 => native.protocol = "other".into(),
            1 => native.digest = "forged".into(),
            2 => native.size_bytes += 1,
            _ => native.wire_body["input"] = json!("tampered with unchanged digest claim"),
        }
        assert!(!owner.permits_selected_native_exact_replay(&changed));
    }
    for body in [
        json!({"input":"question", "previous_response_id":"resp_wire"}),
        json!({"input":"question", "conversation":"conv_wire"}),
        json!({"input":[{"type":"item_reference","id":"item_wire"}]}),
    ] {
        let owner = base.with_provider_transport_payload(Some(
            crate::ports::ProviderTransportPayload::openai_responses(body).unwrap(),
        ));
        let mut input = ProviderInvocationInput::default();
        owner
            .apply_provider_transport(&runtime, &mut input)
            .unwrap();
        assert!(!owner.permits_selected_native_exact_replay(&input));
    }
}

fn exact_replay_config_input(
    runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
) -> ProviderInvocationInput {
    ProviderInvocationInput {
        provider_code: runtime.provider_code.clone(),
        protocol: runtime.protocol.clone(),
        model: runtime.model.clone(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "same logical request".into(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn selected_route_reuses_effective_configuration_while_new_route_reads_fresh_source() {
    use orchestration_runtime::execution_engine::ProviderInvoker;
    let mut invoker = issue_1743_invoker(
        Uuid::now_v7(),
        Arc::new(Issue1743ContinuationStore::default()),
        None,
    );
    // Exercise the actual host send/config path without creating a workflow or billing session.
    invoker.flow_run_id = None;
    invoker.api_node_id = Some("local:test".into());
    invoker.provider_install_root = Some(std::env::temp_dir());
    let (provider_runtime, captured) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    invoker.runtime = provider_runtime;
    let instance = invoker.repository.seed_provider_instance(
        "fixture_provider",
        "exact replay",
        true,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["gpt-5.4-mini"],
    );
    let runtime = issue_1743_runtime(instance);
    invoker.repository.replace_instance_runtime_configuration(
        instance,
        json!({"base_url":"https://original.example/v1"}),
        json!({"api_key":"original-secret"}),
    );
    let route = invoker.resolve_llm_route(&runtime).await.unwrap();
    invoker
        .invoke_resolved_llm(&runtime, route.clone(), exact_replay_config_input(&runtime))
        .await
        .unwrap();
    invoker.repository.replace_instance_runtime_configuration(
        instance,
        json!({"base_url":"https://replacement.example/v1"}),
        json!({"api_key":"replacement-secret"}),
    );
    invoker
        .invoke_resolved_llm(&runtime, route.clone(), exact_replay_config_input(&runtime))
        .await
        .unwrap();
    let configured_route = invoker.resolve_llm_route(&runtime).await.unwrap();
    invoker
        .invoke_resolved_llm(
            &runtime,
            configured_route,
            exact_replay_config_input(&runtime),
        )
        .await
        .unwrap();
    let calls = captured.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[0].provider_config, calls[1].provider_config,
        "automatic retry must keep the effective account/endpoint/credential snapshot"
    );
    assert_eq!(calls[1].provider_config["api_key"], "original-secret");
    assert_eq!(
        calls[1].provider_config["base_url"],
        "https://original.example/v1"
    );
    assert_eq!(calls[2].provider_config["api_key"], "replacement-secret");
    assert_eq!(
        calls[2].provider_config["base_url"],
        "https://replacement.example/v1"
    );
    assert_eq!(
        route
            .invocation_pin::<RuntimeProviderInvocationPin>()
            .unwrap()
            .effective_runtime_config
            .get()
            .unwrap(),
        &calls[0].provider_config
    );
}

#[tokio::test]
async fn failed_configuration_maintenance_is_visible_and_does_not_seal_a_snapshot() {
    use orchestration_runtime::execution_engine::ProviderInvoker;
    let mut invoker = issue_1743_invoker(
        Uuid::now_v7(),
        Arc::new(Issue1743ContinuationStore::default()),
        None,
    );
    invoker.flow_run_id = None;
    invoker.api_node_id = Some("local:test".into());
    invoker.provider_install_root = Some(std::env::temp_dir());
    let (provider_runtime, captured) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    invoker.runtime = provider_runtime;
    let instance = invoker.repository.seed_provider_instance(
        "fixture_provider",
        "exact replay",
        true,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["gpt-5.4-mini"],
    );
    let runtime = issue_1743_runtime(instance);
    invoker.repository.replace_instance_runtime_configuration(
        instance,
        json!({"base_url":"https://original.example/v1"}),
        json!({}),
    );
    let route = invoker.resolve_llm_route(&runtime).await.unwrap();
    assert!(invoker
        .invoke_resolved_llm(&runtime, route.clone(), exact_replay_config_input(&runtime))
        .await
        .is_err());
    assert!(
        captured.lock().unwrap().is_empty(),
        "maintenance failure cannot reach the provider"
    );
    assert!(
        route
            .invocation_pin::<RuntimeProviderInvocationPin>()
            .unwrap()
            .effective_runtime_config
            .get()
            .is_none(),
        "failed initialization cannot become a successful config snapshot"
    );
    invoker.repository.replace_instance_runtime_configuration(
        instance,
        json!({"base_url":"https://original.example/v1"}),
        json!({"api_key":"repaired-secret"}),
    );
    invoker
        .invoke_resolved_llm(&runtime, route, exact_replay_config_input(&runtime))
        .await
        .unwrap();
    let calls = captured.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].provider_config["api_key"], "repaired-secret");
}
