use super::*;
use crate::{errors::ControlPlaneError, ports::ModelProviderRepository};
use orchestration_runtime::compiled_plan::CompiledLlmRuntime;
use orchestration_runtime::execution_state::{
    ExecutionStopReason, FlowDebugExecutionOutcome, NodeExecutionTrace,
};
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensInput,
    ProviderCountTokensResult, ProviderFinishReason, ProviderInvocationCapability,
    ProviderInvocationInput, ProviderInvocationResult, ProviderMessage, ProviderMessageRole,
    ProviderModelDescriptor, ProviderRuntimeErrorKind, ProviderStreamEvent, ProviderToolCall,
    ProviderWireOperation,
};
use serde_json::Map;

fn compiled_llm_runtime(
    provider_instance_id: impl Into<String>,
    provider_code: &str,
) -> CompiledLlmRuntime {
    CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.into(),
        provider_instance_display_name: String::new(),
        provider_code: provider_code.to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    }
}

fn assert_control_plane_error(error: anyhow::Error, expected: ControlPlaneError) {
    assert_eq!(error.downcast_ref::<ControlPlaneError>(), Some(&expected));
}

#[derive(Clone)]
struct CapturingCountTokensRuntime {
    captured: Arc<Mutex<Vec<ProviderCountTokensInput>>>,
}

#[derive(Clone)]
struct CapturingCompactRuntime {
    captured: Arc<Mutex<Vec<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderRuntimePort for CapturingCompactRuntime {
    async fn ensure_loaded(&self, _installation: &domain::PluginInstallationRecord) -> Result<()> {
        Ok(())
    }

    async fn validate_provider(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }

    async fn list_models(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        Ok(Vec::new())
    }

    async fn compact(
        &self,
        _installation: &domain::PluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .push(input);
        Ok(ProviderCompactResult::ResponseItems {
            operation: ProviderWireOperation::Compact,
            profile: ProviderCompactProfile::ResponsesCompact,
            response_items: vec![json!({ "type": "message" })],
        })
    }

    async fn invoke_stream(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _input: ProviderInvocationInput,
    ) -> Result<crate::ports::ProviderRuntimeInvocationOutput> {
        panic!("Compact adapter must not invoke Generate")
    }
}

#[tokio::test]
async fn orchestration_runtime_compact_resolves_selected_runtime_and_provider_config() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: CapturingCompactRuntime {
            captured: captured.clone(),
        },
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
        provider_transport_payload: Some(
            crate::ports::ProviderTransportPayload::openai_responses(json!({
                "model": "gpt-5.4-mini",
                "input": "compact transport"
            }))
            .unwrap()
            .with_affinity(crate::ports::ProviderTransportAffinity::new(
                provider_instance_id.to_string(),
                "fixture_provider",
                "openai_compatible",
                "gpt-5.4-mini",
            )),
        ),
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let result = orchestration_runtime::execution_engine::ProviderInvoker::compact(
        &invoker,
        &runtime,
        ProviderInvocationInput {
            operation: ProviderWireOperation::Compact,
            profile: Some(ProviderCompactProfile::ResponsesCompact),
            provider_instance_id: provider_instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: "canonical compact prompt".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..ProviderInvocationInput::default()
        },
    )
    .await
    .expect("selected provider Compact capability should run");
    assert!(result.satisfies_profile(ProviderCompactProfile::ResponsesCompact));
    let captured = captured.lock().expect("capture mutex poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].operation, ProviderWireOperation::Compact);
    assert_eq!(
        captured[0].profile,
        Some(ProviderCompactProfile::ResponsesCompact)
    );
    assert_eq!(
        captured[0].provider_instance_id,
        provider_instance_id.to_string()
    );
    assert_eq!(captured[0].model, "gpt-5.4-mini");
    assert_eq!(captured[0].messages[0].content, "canonical compact prompt");
    assert!(captured[0].native_transport.is_some());
    assert!(!captured[0].provider_config.is_null());
}

#[async_trait]
impl ProviderRuntimePort for CapturingCountTokensRuntime {
    async fn ensure_loaded(&self, _installation: &domain::PluginInstallationRecord) -> Result<()> {
        Ok(())
    }

    async fn validate_provider(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }

    async fn list_models(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        Ok(Vec::new())
    }

    async fn count_tokens(
        &self,
        _installation: &domain::PluginInstallationRecord,
        input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .push(input);
        Ok(ProviderCountTokensResult {
            operation: ProviderWireOperation::CountTokens,
            input_tokens: 23,
        })
    }

    async fn invoke_stream(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _input: ProviderInvocationInput,
    ) -> Result<crate::ports::ProviderRuntimeInvocationOutput> {
        panic!("CountTokens adapter must not invoke Generate")
    }
}

#[tokio::test]
async fn orchestration_runtime_count_tokens_resolves_selected_runtime_and_provider_config() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: CapturingCountTokensRuntime {
            captured: captured.clone(),
        },
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
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = compiled_llm_runtime(provider_instance_id.to_string(), "fixture_provider");
    let result = orchestration_runtime::execution_engine::ProviderInvoker::count_tokens(
        &invoker,
        &runtime,
        ProviderCountTokensInput {
            provider_instance_id: provider_instance_id.to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: "canonical prompt".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..ProviderCountTokensInput::default()
        },
    )
    .await
    .expect("selected provider CountTokens capability should run");
    assert_eq!(result.input_tokens, 23);
    let captured = captured.lock().expect("capture mutex poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].provider_instance_id,
        provider_instance_id.to_string()
    );
    assert_eq!(captured[0].model, "gpt-5.4-mini");
    assert_eq!(captured[0].messages[0].content, "canonical prompt");
    assert!(!captured[0].provider_config.is_null());
}

/// #1366 AC-001 / AC-005: a cancelled partial result remains the only durable terminal when a
/// stale successful execution finishes later.
#[tokio::test]
async fn late_success_projects_cancelled_winner_without_success_terminal_or_answer_history() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let now = OffsetDateTime::now_utc();
    let running = OrchestrationRuntimeRepository::create_flow_run(
        &repository,
        &crate::ports::CreateFlowRunInput {
            actor_user_id: Uuid::nil(),
            application_id: Uuid::nil(),
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: "late-success".to_string(),
            flow_schema_version: "1".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "late success".to_string(),
            status: domain::FlowRunStatus::Running,
            input_payload: json!({}),
            started_at: now,
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        },
    )
    .await
    .expect("running flow should be created");
    let stale_partial = OrchestrationRuntimeRepository::update_flow_run(
        &repository,
        &crate::ports::UpdateFlowRunInput {
            flow_run_id: running.id,
            status: domain::FlowRunStatus::Running,
            output_payload: json!({ "answer": "partial" }),
            error_payload: None,
            finished_at: None,
        },
    )
    .await
    .expect("partial result should be durable before cancellation");
    let cancel_event = debug_stream_events::flow_cancelled(running.id);
    let cancelled = OrchestrationRuntimeRepository::commit_flow_run_terminal(
        &repository,
        &crate::ports::CommitFlowRunTerminalInput {
            flow_run_id: running.id,
            expected_status: domain::FlowRunStatus::Running,
            result: crate::ports::CommitFlowRunTerminalResult::Cancelled {
                output_payload: stale_partial.output_payload.clone(),
                error_payload: None,
            },
            flow_run_event_payload: json!({ "reason": "manual_stop" }),
            terminal_event_payload: cancel_event.payload,
            finished_at: now,
        },
    )
    .await
    .expect("cancellation commit should succeed");
    assert!(matches!(
        cancelled,
        crate::ports::CommitFlowRunTerminalReceipt::Winner(_)
            | crate::ports::CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(_)
    ));

    let late_success = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![NodeExecutionTrace {
            node_id: "node-answer".to_string(),
            node_type: "answer".to_string(),
            node_alias: "Answer".to_string(),
            input_payload: json!({}),
            output_payload: json!({ "answer": "late success must lose" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            provider_events: Vec::new(),
        }],
    };
    let projected = persist_flow_debug_outcome(
        &repository,
        PersistFlowDebugOutcomeInput {
            scope_id: Uuid::nil(),
            application_name: "fixture application",
            task_queue: None,
            application_id: running.application_id,
            flow_run: &stale_partial,
            compiled_plan: None,
            outcome: &late_success,
            prepared_node_runs: None,
            answer_presentation: None,
            trigger_event_type: "flow_run_execution_finished_late",
            trigger_event_payload: json!({}),
            base_started_at: now,
            waiting_node_resume: None,
        },
    )
    .await
    .expect("late completion should resolve the durable cancellation winner");

    assert_eq!(projected.flow_run.status, domain::FlowRunStatus::Cancelled);
    assert!(projected.stream_events.is_empty());
    assert_eq!(
        projected
            .terminal_event
            .as_ref()
            .map(|event| event.event_type.as_str()),
        Some("flow_cancelled")
    );
    let runtime_events =
        OrchestrationRuntimeRepository::list_runtime_events(&repository, running.id, 0)
            .await
            .expect("runtime replay should be readable");
    assert_eq!(
        runtime_events
            .iter()
            .filter(|event| event.event_type == "flow_cancelled")
            .count(),
        1
    );
    assert!(runtime_events
        .iter()
        .all(|event| event.event_type != "flow_finished" && event.event_type != "text_delta"));
    let detail = OrchestrationRuntimeRepository::get_application_run_detail(
        &repository,
        running.application_id,
        running.id,
    )
    .await
    .expect("run detail should load")
    .expect("run detail should exist");
    assert!(detail
        .events
        .iter()
        .all(|event| event.event_type != "flow_run_completed"));
}

#[tokio::test]
async fn orchestration_runtime_persists_visible_internal_llm_tool_route_events() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let now = OffsetDateTime::now_utc();
    let flow_run = OrchestrationRuntimeRepository::create_flow_run(
        &repository,
        &crate::ports::CreateFlowRunInput {
            actor_user_id: Uuid::nil(),
            application_id: Uuid::nil(),
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: "debug-session".to_string(),
            flow_schema_version: "1".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "debug flow".to_string(),
            status: domain::FlowRunStatus::Running,
            input_payload: json!({}),
            started_at: now,
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        },
    )
    .await
    .expect("flow run should be created");
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![NodeExecutionTrace {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            input_payload: json!({}),
            output_payload: json!({ "text": "done" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "visible_internal_llm_tool_events": [
                    {
                        "event_type": "visible_internal_llm_tool_started",
                        "main_node_id": "node-llm",
                        "target_node_id": "node-mounted-llm",
                        "tool_name": "image_llm",
                        "tool_call_id": "call_visible",
                        "arguments": { "task": "describe image" }
                    },
                    {
                        "event_type": "visible_internal_llm_tool_completed",
                        "main_node_id": "node-llm",
                        "target_node_id": "node-mounted-llm",
                        "tool_name": "image_llm",
                        "tool_call_id": "call_visible",
                        "provider_route": { "model": "gpt-5.4-mini" }
                    }
                ]
            }),
            provider_events: Vec::new(),
        }],
    };

    persist_flow_debug_outcome(
        &repository,
        PersistFlowDebugOutcomeInput {
            scope_id: Uuid::nil(),
            application_name: "fixture application",
            task_queue: None,
            application_id: flow_run.application_id,
            flow_run: &flow_run,
            compiled_plan: None,
            outcome: &outcome,
            prepared_node_runs: None,
            answer_presentation: None,
            trigger_event_type: "flow_run_started",
            trigger_event_payload: json!({}),
            base_started_at: now,
            waiting_node_resume: None,
        },
    )
    .await
    .expect("debug outcome should persist");

    let runtime_events =
        OrchestrationRuntimeRepository::list_runtime_events(&repository, flow_run.id, 0)
            .await
            .expect("runtime events should be listed");
    assert!(runtime_events.iter().any(|event| {
        event.event_type == "visible_internal_llm_tool_started"
            && event.node_run_id.is_some()
            && event.payload["node_id"] == json!("node-llm")
            && event.payload["tool_name"] == json!("image_llm")
            && event.payload["arguments"]["task"] == json!("describe image")
    }));
    assert!(runtime_events.iter().any(|event| {
        event.event_type == "visible_internal_llm_tool_completed"
            && event.payload["target_node_id"] == json!("node-mounted-llm")
            && event.payload["provider_route"]["model"] == json!("gpt-5.4-mini")
    }));
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_keeps_invalid_uuid_as_source_instance_id() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime("not-a-uuid", "fixture_provider"))
        .await
        .expect_err("invalid provider_instance_id should fail");

    assert_control_plane_error(error, ControlPlaneError::InvalidInput("source_instance_id"));
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_does_not_fallback_when_selected_instance_is_missing(
) {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (alpha_instance_id, _) = repository.seed_included_provider_instances();
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            Uuid::now_v7().to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("missing selected instance should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::NotFound("model_provider_instance"),
    );
    assert_ne!(alpha_instance_id, Uuid::nil());
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_does_not_fallback_when_selected_instance_is_not_ready(
) {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (_, backup_instance_id) = repository.seed_included_provider_instances();
    repository.set_instance_status(
        backup_instance_id,
        domain::ModelProviderInstanceStatus::Disabled,
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            backup_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("non-ready selected instance should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("provider_instance_not_ready"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_provider_code_mismatch() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "other_provider",
        ))
        .await
        .expect_err("provider_code mismatch should fail");

    assert_control_plane_error(error, ControlPlaneError::InvalidInput("provider_code"));
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_instance_not_in_main() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let provider_instance_id = repository.seed_provider_instance(
        "fixture_provider",
        "Not In Main",
        false,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["gpt-5.4-mini"],
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("instance excluded from main should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("provider_instance_not_in_main"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_unassigned_installation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), provider_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist")
            .installation_id;
    repository.remove_assignment_for_installation(Uuid::nil(), installation_id);
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("unassigned installation should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("plugin_assignment_required"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_disabled_installation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), provider_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist")
            .installation_id;
    repository.set_installation_state(
        installation_id,
        domain::PluginDesiredState::Disabled,
        domain::PluginAvailabilityStatus::Available,
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("disabled installation should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("plugin_installation_unavailable"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_unavailable_installation() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let installation_id =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), provider_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist")
            .installation_id;
    repository.set_installation_state(
        installation_id,
        domain::PluginDesiredState::ActiveRequested,
        domain::PluginAvailabilityStatus::ArtifactMissing,
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&compiled_llm_runtime(
            provider_instance_id.to_string(),
            "fixture_provider",
        ))
        .await
        .expect_err("unavailable installation should fail");

    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("plugin_installation_unavailable"),
    );
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_uses_selected_child_instance_without_provider_fallback(
) {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (_, backup_instance_id) = repository.seed_included_provider_instances();
    repository.set_instance_enabled_models(backup_instance_id, vec!["gpt-5.4-mini"]);
    let invoker = RuntimeProviderInvoker {
        repository: repository.clone(),
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
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let resolved = invoker
        .resolve_llm_instance(&orchestration_runtime::compiled_plan::CompiledLlmRuntime {
            provider_instance_id: backup_instance_id.to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        })
        .await
        .expect("selected child instance should resolve");

    let repository_instance =
        ModelProviderRepository::get_instance(&repository, Uuid::nil(), backup_instance_id)
            .await
            .expect("instance lookup should succeed")
            .expect("instance should exist");
    assert_eq!(resolved.id, repository_instance.id);
    assert_eq!(resolved.display_name, repository_instance.display_name);
}

#[tokio::test]
async fn orchestration_runtime_resolve_llm_instance_rejects_model_only_present_in_catalog_cache() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let selected_instance_id = repository.seed_provider_instance(
        "fixture_provider",
        "Cache Wider Than Enabled",
        true,
        domain::ModelProviderInstanceStatus::Ready,
        vec!["other-model"],
    );
    repository
        .set_instance_catalog_models(selected_instance_id, vec!["other-model", "gpt-5.4-mini"]);
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };

    let error = invoker
        .resolve_llm_instance(&orchestration_runtime::compiled_plan::CompiledLlmRuntime {
            provider_instance_id: selected_instance_id.to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        })
        .await
        .expect_err("model outside enabled_model_ids should fail");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidInput("model"))
    ));
}

#[tokio::test]
async fn orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
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
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    };
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "Describe image".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "text", "text": "Describe image"},
                {
                    "type": "image_url",
                    "image_url": {"url": "https://example.com/cat.png"}
                }
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker, &runtime, input,
    )
    .await
    .expect("non-multimodal model should receive textualized media context");

    let content = output.result.final_content.unwrap_or_default();
    assert!(content.contains("\"error_code\":\"message_media_unsupported\""));
    assert!(content.contains("\"url\":\"https://example.com/cat.png\""));
    assert!(!content.contains("content_blocks"));
}

#[tokio::test]
async fn orchestration_runtime_keeps_user_media_when_configured_model_supports_multimodal() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    repository.set_configured_model_supports_multimodal(provider_instance_id, "gpt-5.4-mini", true);
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: runtime_port,
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
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    };
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "Describe image".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type": "text", "text": "Describe image"},
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "aW1hZ2U="
                    }
                }
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(&invoker, &runtime, input)
        .await
        .expect("configured multimodal model should receive media content blocks");

    let captured = captured_inputs
        .lock()
        .expect("captured provider inputs should be readable");
    let content_blocks = captured[0].messages[0]
        .content_blocks
        .as_ref()
        .expect("media content blocks should be preserved for multimodal configured models");
    assert_eq!(content_blocks[1]["type"], json!("image"));
    assert_eq!(
        content_blocks[1]["source"]["media_type"],
        json!("image/png")
    );
    assert!(!captured[0].messages[0]
        .content
        .contains("message_media_unsupported"));
}

#[tokio::test]
async fn native_provider_transport_payload_restores_the_ephemeral_invocation_capability() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let (runtime_port, captured_inputs) =
        test_support::InMemoryProviderRuntime::with_invocation_capture();
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: runtime_port,
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
        provider_transport_payload: Some(
            crate::ports::ProviderTransportPayload::openai_responses(json!({
                "model": "gpt-5.4-mini",
                "input": "native transport"
            }))
            .unwrap()
            .with_affinity(crate::ports::ProviderTransportAffinity::new(
                provider_instance_id.to_string(),
                "fixture_provider",
                "openai_compatible",
                "gpt-5.4-mini",
            )),
        ),
        provider_transport_store: None,
        provider_continuation: None,
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

    orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(&invoker, &runtime, input)
        .await
        .expect("ephemeral native transport should reach the provider invocation");

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
        repository,
        runtime: runtime_port,
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
async fn orchestration_runtime_canonicalizes_live_provider_tool_call_names() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let (provider_instance_id, _) = repository.seed_included_provider_instances();
    let tool_call = ProviderToolCall {
        id: "call_bash".to_string(),
        name: "bash".to_string(),
        arguments: json!({ "command": "pwd" }),
        provider_metadata: json!({}),
    };
    let runtime_port = test_support::InMemoryProviderRuntime::with_provider_events_and_result(
        vec![
            ProviderStreamEvent::ToolCallDelta {
                call_id: "call_bash".to_string(),
                delta: json!({
                    "function": {
                        "name": "bash",
                        "arguments": ""
                    }
                }),
            },
            ProviderStreamEvent::ToolCallCommit {
                call: tool_call.clone(),
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::ToolCall,
            },
        ],
        ProviderInvocationResult {
            tool_calls: vec![tool_call],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
    );
    let (live_sender, mut live_receiver) = mpsc::channel(32);
    let invoker = RuntimeProviderInvoker {
        repository,
        runtime: runtime_port,
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: Some(live_sender),
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: Some("node-llm".to_string()),
        active_node_run_id: Some(Uuid::now_v7()),
        api_node_id: None,
        provider_install_root: None,
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
    };
    let runtime = orchestration_runtime::compiled_plan::CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    };
    let input = ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "run pwd".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        tools: vec![json!({
            "type": "function",
            "function": {
                "name": "Bash",
                "parameters": {
                    "type": "object"
                }
            }
        })],
        ..ProviderInvocationInput::default()
    };

    let output = orchestration_runtime::execution_engine::ProviderInvoker::invoke_llm(
        &invoker, &runtime, input,
    )
    .await
    .expect("provider invocation should succeed");

    assert_eq!(output.result.tool_calls[0].name, "Bash");
    let live_events = std::iter::from_fn(|| live_receiver.try_recv().ok()).collect::<Vec<_>>();
    assert!(live_events.iter().any(|event| {
        matches!(
            &event.event,
            ProviderStreamEvent::ToolCallDelta { delta, .. }
                if delta["function"]["name"] == json!("Bash")
        )
    }));
    assert!(live_events.iter().any(|event| {
        matches!(
            &event.event,
            ProviderStreamEvent::ToolCallCommit { call } if call.name == "Bash"
        )
    }));
}

#[test]
fn orchestration_runtime_textualizes_tool_result_media_for_text_models() {
    let mut input = ProviderInvocationInput {
        messages: vec![
            ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Describe image".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: String::new(),
                name: Some("Read".to_string()),
                tool_call_id: Some("call_read".to_string()),
                is_error: None,
                tool_calls: None,
                content_blocks: Some(json!([
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/png;base64,SHOULD_NOT_BE_VISIBLE"
                        }
                    }
                ])),
            },
        ],
        ..ProviderInvocationInput::default()
    };

    provider_invoker::textualize_media_content_blocks_for_text_model(&mut input);

    let tool_message = &input.messages[1];
    assert!(tool_message.content_blocks.is_none());
    assert!(tool_message
        .content
        .contains("\"error_code\":\"tool_result_media_unsupported\""));
    assert!(tool_message
        .content
        .contains("\"media_type\":\"image/png\""));
    assert!(!tool_message.content.contains("aW1hZ2U="));
    assert!(tool_message
        .content
        .contains("\"url\":\"data:image/png;base64,[redacted]\""));
    assert!(!tool_message.content.contains("SHOULD_NOT_BE_VISIBLE"));
}

#[test]
fn orchestration_runtime_textualizes_routed_media_as_retry_guidance_for_text_models() {
    let mut input = ProviderInvocationInput {
        messages: vec![
            ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Describe image".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: String::new(),
                name: Some("Read".to_string()),
                tool_call_id: Some("call_read".to_string()),
                is_error: None,
                tool_calls: None,
                content_blocks: Some(json!([
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }
                ])),
            },
        ],
        run_context: std::collections::BTreeMap::from([(
            "visible_internal_llm_media_tools".to_string(),
            json!([
                {
                    "name": "image_llm",
                    "media_kind": "image"
                }
            ]),
        )]),
        ..ProviderInvocationInput::default()
    };

    provider_invoker::textualize_media_content_blocks_for_text_model(&mut input);

    let tool_message = &input.messages[1];
    assert!(tool_message.content_blocks.is_none());
    assert!(tool_message
        .content
        .contains("\"event\":\"routed_media_content_available\""));
    assert!(tool_message.content.contains("\"name\":\"image_llm\""));
    assert!(tool_message
        .content
        .contains("Call the routed media tool again"));
    assert!(!tool_message
        .content
        .contains("tool_result_media_unsupported"));
    assert!(!tool_message.content.contains("message_media_unsupported"));
    assert!(!tool_message.content.contains("aW1hZ2U="));
}
