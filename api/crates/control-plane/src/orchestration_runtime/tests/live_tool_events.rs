use super::*;
use plugin_framework::provider_contract::ProviderOutputItemPhase;

#[test]
fn provider_tool_done_requires_commit_while_message_done_stays_live() {
    let node_run_id = Uuid::now_v7();
    let tool = ProviderStreamEvent::OutputItem {
        phase: ProviderOutputItemPhase::Done,
        output_index: 2,
        item: json!({
            "type": "function_call",
            "call_id": "call_bash",
            "name": "Bash",
            "arguments": "{}"
        }),
    };
    let message = ProviderStreamEvent::OutputItem {
        phase: ProviderOutputItemPhase::Done,
        output_index: 1,
        item: json!({ "type": "message", "content": [] }),
    };

    let delivery = super::super::provider_invoker::committed_tool_delivery_candidate(
        "node-llm",
        node_run_id,
        &tool,
    )
    .unwrap()
    .expect("tool done must enter the commit buffer");
    assert_eq!(delivery.event_type, "provider_output_item_done");
    assert!(
        super::super::provider_invoker::committed_tool_delivery_candidate(
            "node-llm",
            node_run_id,
            &message,
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn provider_tool_done_without_stable_call_id_is_rejected() {
    let event = ProviderStreamEvent::OutputItem {
        phase: ProviderOutputItemPhase::Done,
        output_index: 0,
        item: json!({ "type": "function_call", "name": "Bash" }),
    };
    assert!(
        super::super::provider_invoker::committed_tool_delivery_candidate(
            "node-llm",
            Uuid::now_v7(),
            &event
        )
        .is_err()
    );
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
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        repository,
        runtime: runtime_port,
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: Some(live_sender),
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: Some("node-llm".to_string()),
        active_node_run_id: Some(Uuid::now_v7()),
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        transport_connection_scope_override: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
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
