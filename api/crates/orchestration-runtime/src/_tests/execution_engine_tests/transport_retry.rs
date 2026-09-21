use std::collections::VecDeque;

use super::*;
use crate::execution_engine::full_jitter_delay_ms;
use crate::execution_state::FlowDebugExecutionOutcome;
use extension_contracts::error::ExtensionContractError;
use extension_contracts::provider_contract::{CommitLevel, RecoveryDisposition};
use time::OffsetDateTime;

struct SequencedTransportInvoker {
    attempts: Mutex<VecDeque<ScriptedAttempt>>,
    invocation_ids: Arc<Mutex<Vec<String>>>,
    emit_typed_recovery_receipts: bool,
}

/// One scripted provider attempt. `Error` exercises the real `invoke_llm -> Err`
/// path, where the typed recovery receipt must travel inside the error's
/// `provider_details` exactly as the real provider worker emits it.
enum ScriptedAttempt {
    Output(ProviderInvocationOutput),
    Error {
        kind: ProviderRuntimeErrorKind,
        disposition: RecoveryDisposition,
        commit_level: CommitLevel,
        /// `None` models a failure observed before any socket existed.
        socket_incarnation: Option<u64>,
    },
    /// An error with no `provider_details` at all: the pre-fix
    /// `missing_typed_receipt` shape.
    ErrorWithoutDetails {
        kind: ProviderRuntimeErrorKind,
    },
    /// An error whose `provider_details` carries a corrupt receipt: the receipt
    /// is present but untrustworthy and must never authorize a replay.
    ErrorWithRawDetails {
        kind: ProviderRuntimeErrorKind,
        details: Value,
    },
}

/// Build the exact `provider_details` object a real worker attaches to its
/// failed invocation, using the canonical contract type so the wire key and the
/// receipt shape cannot drift from the parser under test.
fn error_receipt_details(
    input: &ProviderInvocationInput,
    disposition: RecoveryDisposition,
    commit_level: CommitLevel,
    socket_incarnation: Option<u64>,
) -> anyhow::Result<Value> {
    let directive: extension_contracts::provider_contract::ProviderRecoveryDirective =
        serde_json::from_value(
            input.run_context
                [extension_contracts::provider_contract::PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY]
                .clone(),
        )?;
    let receipt = extension_contracts::provider_contract::ProviderRecoveryReceipt {
        attempt: 0,
        transport: extension_contracts::provider_contract::RecoveryTransport::AiNativeWebSocket,
        transport_epoch: directive.transport_epoch,
        socket_incarnation: socket_incarnation
            .map(extension_contracts::provider_contract::SocketIncarnation::new)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        commit_level,
        disposition,
        reason: extension_contracts::provider_contract::RecoveryReason::TransportDisconnected,
    };
    Ok(raw_receipt_details(serde_json::to_value(receipt)?))
}

fn raw_receipt_details(receipt: Value) -> Value {
    let mut details = serde_json::Map::new();
    details.insert(
        extension_contracts::provider_contract::PROVIDER_RECOVERY_RECEIPT_METADATA_KEY.to_string(),
        receipt,
    );
    Value::Object(details)
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
        let attempt = self
            .attempts
            .lock()
            .expect("attempts mutex poisoned")
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("unexpected provider attempt"))?;
        let mut output = match attempt {
            ScriptedAttempt::Output(output) => output,
            ScriptedAttempt::Error {
                kind,
                disposition,
                commit_level,
                socket_incarnation,
            } => {
                return Err(anyhow::Error::new(
                    ExtensionContractError::RuntimeContract {
                        error: Box::new(ProviderRuntimeError {
                            kind,
                            message: "upstream connection reset".to_string(),
                            provider_summary: None,
                            provider_details: Some(error_receipt_details(
                                &input,
                                disposition,
                                commit_level,
                                socket_incarnation,
                            )?),
                        }),
                    },
                ))
            }
            ScriptedAttempt::ErrorWithoutDetails { kind } => {
                return Err(anyhow::Error::new(
                    ExtensionContractError::RuntimeContract {
                        error: Box::new(ProviderRuntimeError {
                            kind,
                            message: "upstream connection reset".to_string(),
                            provider_summary: None,
                            provider_details: None,
                        }),
                    },
                ))
            }
            ScriptedAttempt::ErrorWithRawDetails { kind, mut details } => {
                if details["1flowbase_provider_recovery"]["transport_epoch"] == "$current" {
                    details["1flowbase_provider_recovery"]["transport_epoch"] =
                        input.run_context["provider_recovery"]["transport_epoch"].clone();
                }
                return Err(anyhow::Error::new(
                    ExtensionContractError::RuntimeContract {
                        error: Box::new(ProviderRuntimeError {
                            kind,
                            message: "upstream connection reset".to_string(),
                            provider_summary: None,
                            provider_details: Some(details),
                        }),
                    },
                ));
            }
        };
        let retryable_transport_failure = output.events.iter().any(|event| {
            matches!(
                event,
                ProviderStreamEvent::Error { error }
                    if matches!(
                        error.kind,
                        ProviderRuntimeErrorKind::EndpointUnreachable
                            | ProviderRuntimeErrorKind::ProviderTransportUnavailable
                    )
            )
        });
        if self.emit_typed_recovery_receipts && retryable_transport_failure {
            assert!(
                output.result.provider_metadata.is_object(),
                "typed recovery receipt fixture requires object provider metadata"
            );
            let directive: extension_contracts::provider_contract::ProviderRecoveryDirective =
                serde_json::from_value(
                    input.run_context[extension_contracts::provider_contract::PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY]
                        .clone(),
                )?;
            output.result.set_recovery_receipt(
                extension_contracts::provider_contract::ProviderRecoveryReceipt {
                    attempt: 1,
                    transport:
                        extension_contracts::provider_contract::RecoveryTransport::AiNativeWebSocket,
                    transport_epoch: directive.transport_epoch,
                    socket_incarnation: Some(
                        extension_contracts::provider_contract::SocketIncarnation::new(1)
                            .unwrap(),
                    ),
                    commit_level: extension_contracts::provider_contract::CommitLevel::LifecycleOnly,
                    disposition:
                        extension_contracts::provider_contract::RecoveryDisposition::LogicalInvocationRetry,
                    reason:
                        extension_contracts::provider_contract::RecoveryReason::TransportDisconnected,
                },
            ).map_err(anyhow::Error::msg)?;
        }
        Ok(output)
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
            provider_metadata: json!({}),
            ..ProviderInvocationResult::default()
        },
        first_token_at: after_first_token.then(OffsetDateTime::now_utc),
        time_to_first_token_ms: after_first_token.then_some(1),
    }
}

fn sequenced_invoker(
    outputs: impl IntoIterator<Item = ProviderInvocationOutput>,
) -> (SequencedTransportInvoker, Arc<Mutex<Vec<String>>>) {
    scripted_invoker(outputs.into_iter().map(ScriptedAttempt::Output))
}

fn scripted_invoker(
    attempts: impl IntoIterator<Item = ScriptedAttempt>,
) -> (SequencedTransportInvoker, Arc<Mutex<Vec<String>>>) {
    let invocation_ids = Arc::new(Mutex::new(Vec::new()));
    (
        SequencedTransportInvoker {
            attempts: Mutex::new(attempts.into_iter().collect()),
            invocation_ids: invocation_ids.clone(),
            emit_typed_recovery_receipts: true,
        },
        invocation_ids,
    )
}

fn sequenced_invoker_without_recovery_receipts(
    outputs: impl IntoIterator<Item = ProviderInvocationOutput>,
) -> (SequencedTransportInvoker, Arc<Mutex<Vec<String>>>) {
    let (mut invoker, invocation_ids) = sequenced_invoker(outputs);
    invoker.emit_typed_recovery_receipts = false;
    (invoker, invocation_ids)
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
    assert_eq!(
        attempts[0]["ai_native_recovery"]["provider_inner_attempt"],
        json!(1)
    );
    assert_eq!(attempts[0]["ai_native_recovery"]["outer_attempt"], json!(0));
    assert_eq!(
        attempts[0]["ai_native_recovery"]["decision"],
        json!("retry")
    );
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
async fn endpoint_failure_without_typed_logical_retry_receipt_is_not_replayed() {
    let plan = base_plan();
    let (invoker, invocation_ids) = sequenced_invoker_without_recovery_receipts([
        provider_error_output(ProviderRuntimeErrorKind::EndpointUnreachable, false),
        final_provider_output("must not execute".to_string()),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow failure should remain an execution outcome");
    let attempts = llm_attempts(&outcome);
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0]["ai_native_recovery"]["decision"],
        json!("missing_typed_receipt")
    );
    assert_eq!(
        invocation_ids
            .lock()
            .expect("invocation ids mutex poisoned")
            .len(),
        1
    );
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

// The cases below pin the host-side `invoke_llm -> Err` path. Before this change
// that path always passed `None` to the AI Native decision, so the typed receipt
// the provider attached to its error could never authorize a bounded recovery -
// the incident's `missing_typed_receipt` symptom.

fn err_attempt(
    disposition: RecoveryDisposition,
    commit_level: CommitLevel,
    socket_incarnation: Option<u64>,
) -> ScriptedAttempt {
    ScriptedAttempt::Error {
        kind: ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        disposition,
        commit_level,
        socket_incarnation,
    }
}

#[tokio::test]
async fn err_path_typed_logical_retry_receipt_authorizes_one_bounded_retry() {
    let plan = base_plan();
    let (invoker, invocation_ids) = scripted_invoker([
        err_attempt(
            RecoveryDisposition::LogicalInvocationRetry,
            CommitLevel::LifecycleOnly,
            Some(6),
        ),
        ScriptedAttempt::Output(final_provider_output("recovered via err path".to_string())),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow should execute");
    let attempts = llm_attempts(&outcome);
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts[0]["ai_native_recovery"]["decision"],
        json!("retry")
    );
    assert_eq!(attempts[0]["ai_native_recovery"]["outer_attempt"], json!(0));
    assert_eq!(
        attempts[0]["ai_native_recovery"]["provider_inner_receipt"]["socket_incarnation"],
        json!(6)
    );
    assert_eq!(
        attempts[0]["error_code"],
        json!("provider_transport_unavailable")
    );
    assert_eq!(attempts[1]["is_retry"], json!(true));
    assert_eq!(attempts[1]["status"], json!("succeeded"));

    let ids = invocation_ids
        .lock()
        .expect("invocation ids mutex poisoned");
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn err_path_corrupt_receipt_is_invalid_not_retryable() {
    let plan = base_plan();
    let (invoker, invocation_ids) = scripted_invoker([
        ScriptedAttempt::ErrorWithRawDetails {
            kind: ProviderRuntimeErrorKind::ProviderTransportUnavailable,
            // A recoverable disposition that never names its socket and omits
            // the epoch: present, but not trustworthy.
            details: raw_receipt_details(json!({
                "attempt": 0,
                "transport": "ai_native_websocket",
                "commit_level": "lifecycle_only",
                "disposition": "same_epoch_reconnect",
                "reason": "transport_disconnected",
            })),
        },
        ScriptedAttempt::Output(final_provider_output("must not execute".to_string())),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow failure should remain an execution outcome");
    assert_eq!(llm_attempts(&outcome).len(), 1);
    assert_eq!(
        llm_attempts(&outcome)[0]["ai_native_recovery"]["decision"],
        json!("invalid_receipt")
    );
    assert_eq!(
        invocation_ids
            .lock()
            .expect("invocation ids mutex poisoned")
            .len(),
        1
    );
}

#[tokio::test]
async fn err_path_terminal_socketless_receipt_stops_without_replay() {
    let plan = base_plan();
    let (invoker, invocation_ids) = scripted_invoker([
        err_attempt(
            RecoveryDisposition::TerminalInterruption,
            CommitLevel::Terminal,
            None,
        ),
        ScriptedAttempt::Output(final_provider_output("must not execute".to_string())),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow failure should remain an execution outcome");
    let attempts = llm_attempts(&outcome);
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0]["ai_native_recovery"]["decision"],
        json!("semantic_terminal")
    );
    assert_eq!(
        attempts[0]["ai_native_recovery"]["original_terminal_preserved"],
        json!(true)
    );
    assert_eq!(
        attempts[0]["error_code"],
        json!("provider_transport_unavailable")
    );
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
    assert_eq!(
        invocation_ids
            .lock()
            .expect("invocation ids mutex poisoned")
            .len(),
        1
    );
}

#[tokio::test]
async fn err_path_without_any_receipt_reports_missing_typed_receipt() {
    let plan = base_plan();
    let (invoker, invocation_ids) = scripted_invoker([
        ScriptedAttempt::ErrorWithoutDetails {
            kind: ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        },
        ScriptedAttempt::Output(final_provider_output("must not execute".to_string())),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start":{"query":"hello"}}), &invoker)
        .await
        .expect("flow failure should remain an execution outcome");
    assert_eq!(llm_attempts(&outcome).len(), 1);
    assert_eq!(
        llm_attempts(&outcome)[0]["ai_native_recovery"]["decision"],
        json!("missing_typed_receipt")
    );
    assert_eq!(
        invocation_ids
            .lock()
            .expect("invocation ids mutex poisoned")
            .len(),
        1
    );
}

#[tokio::test]
async fn exhausted_error_preserves_first_last_and_final_decision_in_engine_outcome() {
    let (invoker, ids) = scripted_invoker([ScriptedAttempt::ErrorWithRawDetails {
        kind: ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        details: json!({
            "1flowbase_provider_recovery": {"attempt":1,"transport":"ai_native_web_socket","transport_epoch":"$current","socket_incarnation":4,"commit_level":"terminal","disposition":"terminal_interruption","reason":"budget_exhausted"},
            "1flowbase_provider_recovery_diagnostics": {
                "first_failure":{"kind":"websocket_close","close_code":1011,"reason_category":"proxy_failed","socket_incarnation":3,"attempt":0,"consumed_attempts":1},
                "last_failure":{"kind":"websocket_close","close_code":1008,"reason_category":"continuation_unavailable","socket_incarnation":4,"owner_socket_incarnation":3,"attempt":1,"consumed_attempts":2},
                "association_valid":false,"attempts":[],"secret":"SECRET_CANARY"
            }
        }),
    }]);
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({"node-start":{"query":"hello"}}),
        &invoker,
    )
    .await
    .unwrap();
    assert_eq!(
        ids.lock().unwrap().len(),
        1,
        "terminal receipt must not cause an outer provider call"
    );
    let attempts = llm_attempts(&outcome);
    let metric = &attempts[0];
    assert_eq!(
        metric["ai_native_recovery"]["decision"],
        "semantic_terminal"
    );
    assert_eq!(
        metric["ai_native_recovery"]["provider_attempts_consumed"],
        2
    );
    assert_eq!(
        metric["1flowbase_provider_recovery_diagnostics"]["first_failure"]["close_code"],
        1011
    );
    let ExecutionStopReason::Failed(failure) = outcome.stop_reason else {
        panic!("expected provider failure")
    };
    assert_eq!(
        failure.error_payload["1flowbase_provider_recovery_diagnostics"]["last_failure"]
            ["close_code"],
        1008
    );
    assert_eq!(
        failure.error_payload["ai_native_recovery"]["decision"],
        "semantic_terminal"
    );
    assert!(!failure.error_payload.to_string().contains("SECRET_CANARY"));
}
