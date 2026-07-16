use super::*;
use crate::{errors::ControlPlaneError, ports::ModelProviderRepository};
use orchestration_runtime::compiled_plan::CompiledLlmRuntime;
use orchestration_runtime::execution_state::{
    ExecutionStopReason, FlowDebugExecutionOutcome, NodeExecutionTrace,
};
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationResult, ProviderMessage, ProviderMessageRole,
    ProviderStreamEvent, ProviderToolCall,
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
    let (live_sender, mut live_receiver) = mpsc::unbounded_channel();
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
