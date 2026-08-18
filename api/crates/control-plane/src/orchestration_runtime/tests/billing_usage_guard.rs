use super::*;
use plugin_framework::provider_contract::{ProviderRuntimeError, ProviderUsage};
use plugin_framework::PluginFrameworkError;

fn billing_flow_execution_context(flow_run_id: Uuid) -> Arc<RuntimeFlowExecutionContext> {
    Arc::new(RuntimeFlowExecutionContext {
        active_node: Mutex::new(None),
        data_model: RuntimeDataModelExecutionContext {
            actor: domain::ActorContext::root(Uuid::now_v7(), Uuid::nil(), "root"),
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
    let flow_run_id = Uuid::now_v7();
    RuntimeProviderInvoker {
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
        flow_execution_context: Some(billing_flow_execution_context(flow_run_id)),
        answer_presentation: None,
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
    let releases = repository_probe.model_billing_credit_releases();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].1, "provider_usage_unavailable");
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 0);
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
    let releases = repository_probe.model_billing_credit_releases();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].1, "provider_usage_unavailable");
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 0);
}

// AC-003 + AC-004: billable output without provider usage stays fail-closed, and
// the conflict carries structured evidence instead of a bare static string.
#[tokio::test]
async fn billing_no_usage_with_billable_output_fails_closed_with_evidence() {
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

    let error = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect_err("billable output without usage must fail closed");

    let contract_error = error
        .downcast_ref::<PluginFrameworkError>()
        .and_then(|error| match error {
            PluginFrameworkError::RuntimeContract { error } => Some(error.as_ref()),
            _ => None,
        })
        .expect("usage conflict must carry a structured provider runtime error");
    assert_eq!(contract_error.message, "provider_usage_unavailable");
    let details = contract_error
        .provider_details
        .as_ref()
        .expect("usage conflict must carry provider evidence details");
    assert_eq!(details["finish_reason"], json!("stop"));
    assert_eq!(details["billable_output"], json!(true));
    let releases = repository_probe.model_billing_credit_releases();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].1, "provider_usage_unavailable");
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 0);
}

// AC-004 positive control: reported usage keeps the settlement path and never
// releases the reservation.
#[tokio::test]
async fn billing_with_usage_attempts_settlement_without_release() {
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
    let invoker = billing_invoker(repository, runtime);
    let runtime = compiled_llm_runtime(provider_instance_id, "fixture_provider");

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker,
        &runtime,
        provider_user_input(provider_instance_id),
    )
    .await
    .expect("usage-bearing invocation must succeed");

    assert_eq!(
        output.result.final_content.as_deref(),
        Some("computed answer")
    );
    assert_eq!(repository_probe.model_billing_reserved_session_count(), 1);
    assert_eq!(repository_probe.model_billing_finalize_attempt_count(), 1);
    assert!(repository_probe.model_billing_credit_releases().is_empty());
}
