use std::collections::VecDeque;

use super::*;
use crate::execution_engine::full_jitter_delay_ms;
use crate::execution_state::FlowDebugExecutionOutcome;
use time::OffsetDateTime;

struct SequencedTransportInvoker {
    outputs: Mutex<VecDeque<ProviderInvocationOutput>>,
    invocation_ids: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ProviderInvoker for SequencedTransportInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.invocation_ids
            .lock()
            .expect("invocation ids mutex poisoned")
            .push(
                input
                    .trace_context
                    .get("provider_invocation_id")
                    .expect("each attempt must have an invocation id")
                    .clone(),
            );
        self.outputs
            .lock()
            .expect("outputs mutex poisoned")
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("unexpected provider attempt"))
    }
}

#[async_trait]
impl CapabilityInvoker for SequencedTransportInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("transport retry fixture does not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for SequencedTransportInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("transport retry fixture does not execute code nodes")
    }
}

fn provider_error_output(
    kind: ProviderRuntimeErrorKind,
    after_first_token: bool,
) -> ProviderInvocationOutput {
    let error = ProviderRuntimeError::new(kind, "upstream connection reset");
    let mut events = Vec::new();
    if after_first_token {
        events.push(ProviderStreamEvent::TextDelta {
            delta: "partial".to_string(),
        });
    }
    events.push(ProviderStreamEvent::Error { error });
    ProviderInvocationOutput {
        events,
        result: ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
        first_token_at: after_first_token.then(OffsetDateTime::now_utc),
        time_to_first_token_ms: after_first_token.then_some(1),
    }
}

fn sequenced_invoker(
    outputs: impl IntoIterator<Item = ProviderInvocationOutput>,
) -> (SequencedTransportInvoker, Arc<Mutex<Vec<String>>>) {
    let invocation_ids = Arc::new(Mutex::new(Vec::new()));
    (
        SequencedTransportInvoker {
            outputs: Mutex::new(outputs.into_iter().collect()),
            invocation_ids: invocation_ids.clone(),
        },
        invocation_ids,
    )
}

fn llm_attempts(outcome: &FlowDebugExecutionOutcome) -> &Vec<Value> {
    outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist")
        .metrics_payload["attempts"]
        .as_array()
        .expect("attempt metrics should be present")
}

#[tokio::test]
async fn endpoint_unreachable_before_first_token_retries_without_node_opt_in() {
    let plan = base_plan();
    assert_eq!(plan.nodes["node-llm"].config.get("retry_enabled"), None);
    let (invoker, invocation_ids) = sequenced_invoker([
        provider_error_output(ProviderRuntimeErrorKind::EndpointUnreachable, false),
        final_provider_output("recovered".to_string()),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow should execute");
    let attempts = llm_attempts(&outcome);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0]["error_code"], json!("endpoint_unreachable"));
    assert_eq!(attempts[0]["failed_after_first_token"], json!(false));
    assert_eq!(attempts[1]["is_retry"], json!(true));
    assert_eq!(attempts[1]["retry_reason"], json!("endpoint_unreachable"));
    assert_eq!(attempts[1]["status"], json!("succeeded"));

    let ids = invocation_ids
        .lock()
        .expect("invocation ids mutex poisoned");
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn provider_transport_unavailable_before_first_token_retries_without_node_opt_in() {
    let plan = base_plan();
    let (invoker, invocation_ids) = sequenced_invoker([
        provider_error_output(
            ProviderRuntimeErrorKind::ProviderTransportUnavailable,
            false,
        ),
        final_provider_output("recovered".to_string()),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow should execute");
    let attempts = llm_attempts(&outcome);
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts[0]["error_code"],
        json!("provider_transport_unavailable")
    );
    assert_eq!(attempts[0]["failed_after_first_token"], json!(false));
    assert_eq!(attempts[1]["is_retry"], json!(true));
    assert_eq!(
        attempts[1]["retry_reason"],
        json!("provider_transport_unavailable")
    );
    assert_eq!(attempts[1]["status"], json!("succeeded"));

    let ids = invocation_ids
        .lock()
        .expect("invocation ids mutex poisoned");
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn automatic_transport_retry_budget_exhaustion_keeps_original_error() {
    let plan = base_plan();
    let (invoker, invocation_ids) = sequenced_invoker([
        provider_error_output(ProviderRuntimeErrorKind::EndpointUnreachable, false),
        provider_error_output(ProviderRuntimeErrorKind::EndpointUnreachable, false),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow failure should remain an execution outcome");
    let attempts = llm_attempts(&outcome);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[1]["error_code"], json!("endpoint_unreachable"));
    assert_eq!(
        invocation_ids
            .lock()
            .expect("invocation ids mutex poisoned")
            .len(),
        2
    );
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
}

#[tokio::test]
async fn automatic_transport_retry_rejects_post_token_and_non_endpoint_failures() {
    for output in [
        provider_error_output(ProviderRuntimeErrorKind::EndpointUnreachable, true),
        provider_error_output(ProviderRuntimeErrorKind::ProviderTransportUnavailable, true),
        provider_error_output(ProviderRuntimeErrorKind::ProviderInvalidResponse, false),
    ] {
        let plan = base_plan();
        let (invoker, invocation_ids) = sequenced_invoker([output]);
        let outcome =
            start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
                .await
                .expect("flow failure should remain an execution outcome");
        assert_eq!(llm_attempts(&outcome).len(), 1);
        assert_eq!(
            invocation_ids
                .lock()
                .expect("invocation ids mutex poisoned")
                .len(),
            1
        );
    }
}

#[test]
fn automatic_transport_retry_uses_bounded_full_jitter() {
    assert_eq!(full_jitter_delay_ms(0, 0), 0);
    assert_eq!(full_jitter_delay_ms(0, 200), 200);
    assert_eq!(full_jitter_delay_ms(0, 201), 0);
    assert_eq!(full_jitter_delay_ms(3, u64::MAX), u64::MAX % 1_001);
    assert!(full_jitter_delay_ms(usize::MAX, u64::MAX) <= 1_000);
}
