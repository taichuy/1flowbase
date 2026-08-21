use super::*;
use async_trait::async_trait;
use plugin_framework::provider_contract::ProviderMessage;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Default)]
struct Issue1743ContinuationStore {
    continuation: Mutex<
        Option<(
            crate::ports::ProviderContinuationSlotId,
            crate::ports::ProviderContinuation,
        )>,
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
        *self.continuation.lock().await = Some((slot_id, continuation));
        Ok(())
    }

    async fn get_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<crate::ports::ProviderContinuation>> {
        Ok(self
            .continuation
            .lock()
            .await
            .as_ref()
            .filter(|(stored_slot, _)| *stored_slot == slot_id)
            .map(|(_, continuation)| continuation.clone()))
    }

    async fn consume_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
    ) -> anyhow::Result<crate::ports::ProviderContinuation> {
        let mut stored = self.continuation.lock().await;
        let matches = stored
            .as_ref()
            .is_some_and(|(stored_slot, _)| *stored_slot == slot_id);
        anyhow::ensure!(matches, "ephemeral_continuation_missing");
        Ok(stored.take().expect("matching continuation must exist").1)
    }

    async fn delete_continuation(
        &self,
        slot_id: crate::ports::ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        let mut stored = self.continuation.lock().await;
        if stored
            .as_ref()
            .is_some_and(|(stored_slot, _)| *stored_slot == slot_id)
        {
            stored.take();
            return Ok(true);
        }
        Ok(false)
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
        flow_execution_context: None,
        answer_presentation: None,
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
