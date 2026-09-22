use super::*;
use plugin_framework::provider_contract::{
    ProviderOutputProtocolFailure, ProviderRuntimeError, ProviderUsage,
};
use plugin_framework::PluginFrameworkError;

fn billing_flow_execution_context(
    flow_run_id: Uuid,
    is_root: bool,
    require_provider_usage_for_billing: bool,
) -> Arc<RuntimeFlowExecutionContext> {
    let user_id = Uuid::now_v7();
    let actor = if is_root {
        domain::ActorContext::root(user_id, Uuid::nil(), "root")
    } else {
        domain::ActorContext::scoped(user_id, Uuid::nil(), "member", Vec::<String>::new())
    };
    Arc::new(RuntimeFlowExecutionContext {
        user_account: Some("billing-user".to_string()),
        require_provider_usage_for_billing,
        active_node: Mutex::new(None),
        tool_delivery_events: Mutex::new(Vec::new()),
        data_model: RuntimeDataModelExecutionContext {
            actor,
            application_id: Uuid::nil(),
            draft_id: Uuid::nil(),
            flow_run_id,
            runtime_engine: Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        },
    })
}

fn billing_invoker(
    repository: test_support::InMemoryOrchestrationRuntimeRepository,
    runtime: test_support::InMemoryProviderRuntime,
) -> RuntimeProviderInvoker<
    test_support::InMemoryOrchestrationRuntimeRepository,
    test_support::InMemoryProviderRuntime,
> {
    repository.allow_model_billing_finalize();
    billing_invoker_with_policy(repository, runtime, false, false)
}

fn billing_invoker_with_policy(
    repository: test_support::InMemoryOrchestrationRuntimeRepository,
    runtime: test_support::InMemoryProviderRuntime,
    is_root: bool,
    require_provider_usage_for_billing: bool,
) -> RuntimeProviderInvoker<
    test_support::InMemoryOrchestrationRuntimeRepository,
    test_support::InMemoryProviderRuntime,
> {
    let flow_run_id = Uuid::now_v7();
    RuntimeProviderInvoker {
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository,
        runtime,
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: Some(flow_run_id),
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: Some(billing_flow_execution_context(
            flow_run_id,
            is_root,
            require_provider_usage_for_billing,
        )),
        answer_presentation: None,
        transport_connection_scope_override: None,
        observation_context: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    }
}

fn upstream_error_event() -> ProviderStreamEvent {
    ProviderStreamEvent::Error {
        error: ProviderRuntimeError {
            kind: ProviderRuntimeErrorKind::ProviderUpstreamError,
            message: "429 Too Many Requests: relay rejected the request".to_string(),
            provider_summary: None,
            provider_details: Some(json!({ "status_code": 429 })),
        },
    }
}

fn partial_upstream_error_output() -> crate::ports::ProviderRuntimeInvocationOutput {
    crate::ports::ProviderRuntimeInvocationOutput {
        events: vec![
            ProviderStreamEvent::TextDelta {
                delta: "partial answer".to_string(),
            },
            upstream_error_event(),
        ],
        result: ProviderInvocationResult {
            final_content: Some("partial answer".to_string()),
            ..ProviderInvocationResult::default()
        },
    }
}

fn event_only_finish_error_output() -> crate::ports::ProviderRuntimeInvocationOutput {
    crate::ports::ProviderRuntimeInvocationOutput {
        events: vec![
            ProviderStreamEvent::TextDelta {
                delta: "partial answer".to_string(),
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            },
        ],
        result: ProviderInvocationResult {
            final_content: Some("partial answer".to_string()),
            ..ProviderInvocationResult::default()
        },
    }
}

fn result_only_finish_error_output() -> crate::ports::ProviderRuntimeInvocationOutput {
    crate::ports::ProviderRuntimeInvocationOutput {
        events: vec![ProviderStreamEvent::TextDelta {
            delta: "partial answer".to_string(),
        }],
        result: ProviderInvocationResult {
            final_content: Some("partial answer".to_string()),
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
    }
}

fn output_protocol_failure_output() -> crate::ports::ProviderRuntimeInvocationOutput {
    crate::ports::ProviderRuntimeInvocationOutput {
        events: vec![ProviderStreamEvent::OutputProtocolFailure {
            failure: ProviderOutputProtocolFailure {
                protocol: "fixture".to_string(),
                error_code: "invalid_output".to_string(),
                message: "provider emitted invalid output".to_string(),
                retry_feedback: "retry with a valid output".to_string(),
                provider_details: json!({"fixture": true}),
            },
        }],
        result: ProviderInvocationResult::default(),
    }
}

// AC-001 + AC-004: billing armed, transport-Ok stream carrying an upstream error
// event and no usage must reach the caller for truthful classification instead of
// being replaced by an opaque provider_usage_unavailable conflict.
#[tokio::test]
async fn billing_no_usage_with_upstream_error_event_returns_output_for_classification() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: vec![upstream_error_event()],
            result: ProviderInvocationResult {
                finish_reason: Some(ProviderFinishReason::Error),
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect(
        "stream without usage must surface the upstream error evidence, not a billing conflict",
    );

    let stream_error = output.events.iter().find_map(|event| match event {
        ProviderStreamEvent::Error { error } => Some(error),
        _ => None,
    });
    let stream_error = stream_error.expect("upstream error event must be preserved");
    assert_eq!(
        stream_error.kind,
        ProviderRuntimeErrorKind::ProviderUpstreamError
    );
    assert!(stream_error.message.contains("429 Too Many Requests"));
    assert_eq!(
        stream_error.provider_details,
        Some(json!({ "status_code": 429 }))
    );
    assert_eq!(repository_probe.model_billing_reserved_session_count(), 1);
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

// AC-001: partial content must not make a later upstream failure billable.
// The original error remains available to executor classification and retry
// policy after the missing usage is recorded as zero.
#[tokio::test]
async fn billing_no_usage_after_partial_content_preserves_upstream_failure() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        partial_upstream_error_output(),
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("mixed stream must reach executor classification");

    assert!(output.events.iter().any(|event| {
        matches!(event, ProviderStreamEvent::Error { error } if error.message.contains("429"))
    }));
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

#[tokio::test]
async fn billing_no_usage_with_event_only_finish_error_returns_output_for_classification() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        event_only_finish_error_output(),
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("event-only finish error must reach executor classification");

    assert!(output.events.iter().any(|event| {
        matches!(event, ProviderStreamEvent::Finish { reason } if *reason == ProviderFinishReason::Error)
    }));
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

#[tokio::test]
async fn billing_no_usage_with_result_only_finish_error_returns_output_for_classification() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        result_only_finish_error_output(),
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("result-only finish error must reach executor classification");

    assert_eq!(
        output.result.finish_reason,
        Some(ProviderFinishReason::Error)
    );
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

#[tokio::test]
async fn billing_no_usage_with_output_protocol_failure_returns_output_for_classification() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        output_protocol_failure_output(),
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("protocol failure must reach executor classification");

    assert!(output
        .events
        .iter()
        .any(|event| { matches!(event, ProviderStreamEvent::OutputProtocolFailure { .. }) }));
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

// AC-002 + AC-004: billing armed, empty stream (no events, no output, no usage)
// must reach the caller so the executor classifies it (empty_response / invalid
// finish reason) instead of a masked billing conflict.
#[tokio::test]
async fn billing_no_usage_empty_stream_returns_output_for_classification() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult::default(),
        },
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("empty stream without usage must reach the executor for truthful classification");

    assert!(output.events.is_empty());
    assert_eq!(output.result.final_content, None);
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

// Missing provider usage defaults to a searchable zero-value reconciliation record without
// blocking a completed provider invocation.
#[tokio::test]
async fn billing_no_usage_with_billable_output_records_zero_and_succeeds_by_default() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult {
                final_content: Some("computed answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("missing usage must fail open by default");
    assert_eq!(output.result.usage.input_tokens, Some(0));
    assert_eq!(output.result.usage.output_tokens, Some(0));
    let details = &output.result.provider_metadata["_1flowbase_upstream_provider_metadata"]
        ["_1flowbase_billing"];
    assert_eq!(details["billing_status"], "reconciliation_failed");
    assert_eq!(details["billing_error_code"], "provider_usage_unavailable");
    assert_eq!(details["total_cost"], "0");
    assert!(repository_probe.model_billing_credit_releases().is_empty());
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
    let usage = repository_probe.model_billing_usage_ledger(invoker.flow_run_id.unwrap());
    assert_eq!(usage.len(), 1);
    assert_eq!(
        usage[0].usage_status,
        domain::UsageLedgerStatus::UnavailableError
    );
    assert_eq!(usage[0].total_tokens, Some(0));
    let costs = repository_probe.model_billing_cost_ledger(invoker.flow_run_id.unwrap());
    assert_eq!(costs.len(), 1);
    assert_eq!(costs[0].normalized_cost.as_deref(), Some("0"));
    assert_eq!(costs[0].cost_status, "reconciliation_failed_zero");
}

#[tokio::test]
async fn billing_no_usage_with_billable_output_fails_for_chargeable_user_in_strict_mode() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult {
                final_content: Some("computed answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker_with_policy(repository, runtime, false, true);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let error = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect_err("strict mode must reject missing usage for a chargeable user");
    assert!(error.to_string().contains("provider_usage_unavailable"));
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 0);
    assert_eq!(
        repository_probe.model_billing_credit_releases()[0].1,
        "provider_usage_unavailable"
    );
}

#[tokio::test]
async fn billing_exempt_user_missing_usage_succeeds_even_in_strict_mode() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    repository.allow_model_billing_finalize();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult {
                final_content: Some("computed answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker_with_policy(repository, runtime, true, true);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("billing-exempt user must bypass strict missing-usage failure");
    assert_eq!(output.result.usage.total_tokens, Some(0));
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
}

// AC4: reported usage reaches settlement; a failed settlement releases the reservation
// and retains the fee/usage evidence without returning a successful provider outcome.
#[tokio::test]
async fn billing_finalize_failure_releases_and_preserves_failure_evidence() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.enable_model_billing();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult {
                final_content: Some("computed answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                usage: ProviderUsage {
                    input_tokens: Some(120),
                    output_tokens: Some(30),
                    ..ProviderUsage::default()
                },
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let repository_probe = repository.clone();
    let invoker = billing_invoker_with_policy(repository, runtime, false, false);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let error = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect_err("failed settlement must not return a free success");
    let Some(PluginFrameworkError::RuntimeContract { error }) =
        error.downcast_ref::<PluginFrameworkError>()
    else {
        panic!("expected typed fee failure")
    };
    let details = error
        .provider_details
        .as_ref()
        .expect("fee failure evidence");
    assert_eq!(
        details["_1flowbase_billing"]["billing_status"],
        "reconciliation_failed"
    );
    assert_eq!(
        details["_1flowbase_billing"]["billing_error_code"],
        "billing_finalize_failed"
    );
    assert_eq!(details["usage"]["input_tokens"], 120);
    assert_eq!(details["_1flowbase_user_account"], "billing-user");
    assert_eq!(repository_probe.model_billing_reserved_session_count(), 1);
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
    assert_eq!(repository_probe.model_billing_credit_releases().len(), 1);
}

// AC5: request-log attribution comes from the execution snapshot even when fees
// are disabled; it must not depend on a later users-table lookup.
#[tokio::test]
async fn provider_outcome_carries_account_snapshot_without_billing() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let runtime = test_support::InMemoryProviderRuntime::with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult {
                final_content: Some("answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");
    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("non-billed provider invocation should succeed");
    let mut metadata = &output.result.provider_metadata;
    loop {
        if let Some(account) = metadata.get("_1flowbase_user_account") {
            assert_eq!(account.as_str(), Some("billing-user"));
            break;
        }
        metadata = metadata
            .get("_1flowbase_upstream_provider_metadata")
            .expect("host outcome must carry the frozen user account");
    }
}
