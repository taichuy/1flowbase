use super::visible_internal_llm_tool_fixtures::*;
use super::*;

#[tokio::test]
async fn visible_internal_llm_tool_emits_structured_route_events_in_main_debug_payload() {
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({ "query": "image?" }),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("mounted-visible "),
        final_llm_response("main-after"),
    ]);

    let outcome = start_flow_debug_run(
        &visible_internal_llm_tool_plan(),
        &json!({ "node-start": { "query": "describe the picture", "history": [] } }),
        &invoker,
    )
    .await
    .unwrap();

    let main_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("main llm trace should exist");
    let route_events = main_trace.debug_payload["visible_internal_llm_tool_events"]
        .as_array()
        .expect("main debug payload should include visible internal route events");
    assert_eq!(
        route_events[0]["event_type"],
        json!("visible_internal_llm_tool_started")
    );
    assert_eq!(route_events[0]["main_node_id"], json!("node-llm"));
    assert_eq!(route_events[0]["target_node_id"], json!("node-mounted-llm"));
    assert_eq!(
        route_events[0]["tool_name"],
        json!("inspect_visible_context")
    );
    assert_eq!(route_events[0]["tool_call_id"], json!("call_visible"));
    assert!(route_events.iter().any(|event| {
        event["event_type"] == json!("visible_internal_llm_tool_completed")
            && event["target_node_id"] == json!("node-mounted-llm")
            && event["provider_route"]["model"] == json!("gpt-5.4-mini")
            && event["content"] == json!("mounted-visible ")
    }));
}

#[tokio::test]
async fn visible_internal_llm_tool_recoverable_branch_model_error_returns_hidden_tool_result() {
    let captured_inputs = Arc::new(Mutex::new(Vec::new()));
    let invoker = MountedModelUnsupportedInvoker {
        captured_inputs: captured_inputs.clone(),
    };

    let outcome = start_flow_debug_run(
        &visible_internal_llm_tool_plan(),
        &json!({
            "node-start": {
                "query": "describe the picture",
                "history": [
                    { "role": "user", "content": "describe the picture" }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "recoverable branch model errors should return a hidden tool result, got {:?}",
        outcome.stop_reason
    );
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    let hidden_tool_result = captured[2]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main recall should include hidden branch error as tool result");
    assert!(hidden_tool_result
        .content
        .contains("model_multimodal_unsupported"));
}

#[tokio::test]
async fn visible_internal_llm_tool_resume_recoverable_branch_model_error_returns_hidden_tool_result(
) {
    let (waiting_invoker, _waiting_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({ "query": "image?" }),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        tool_call_response(vec![ProviderToolCall {
            id: "call_bash".to_string(),
            name: "Bash".to_string(),
            arguments: json!({ "command": "file tmp/frontstage-layout-preview.png" }),
            provider_metadata: json!({}),
        }]),
    ]);
    let plan = visible_internal_llm_tool_plan();

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "describe the picture",
                "history": [],
                "tools": [
                    {
                        "name": "Bash",
                        "description": "Run a shell command",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" }
                            },
                            "required": ["command"]
                        }
                    }
                ]
            }
        }),
        &waiting_invoker,
    )
    .await
    .unwrap();
    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("mounted llm tool wait should have checkpoint");

    let captured_inputs = Arc::new(Mutex::new(Vec::new()));
    let resume_invoker = ResumeMountedModelUnsupportedInvoker {
        captured_inputs: captured_inputs.clone(),
    };
    let resumed = resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-mounted-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_bash",
                    "content": "tmp/frontstage-layout-preview.png: PNG image data"
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(resumed.stop_reason, ExecutionStopReason::Completed),
        "recoverable branch model errors after callback should return a hidden tool result, got {:?}",
        resumed.stop_reason
    );
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 2);
    let hidden_tool_result = captured[1]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main recall should include hidden branch error as tool result");
    assert!(hidden_tool_result
        .content
        .contains("model_multimodal_unsupported"));
}

struct MountedModelUnsupportedInvoker {
    captured_inputs: Arc<Mutex<Vec<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderInvoker for MountedModelUnsupportedInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        let mut captured = self
            .captured_inputs
            .lock()
            .expect("captured inputs mutex poisoned");
        let call_index = captured.len();
        captured.push(input);
        drop(captured);

        match call_index {
            0 => Ok(ProviderInvocationOutput {
                events: vec![ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::ToolCall,
                }],
                result: ProviderInvocationResult {
                    final_content: Some("main-before ".to_string()),
                    tool_calls: vec![ProviderToolCall {
                        id: "call_visible".to_string(),
                        name: "inspect_visible_context".to_string(),
                        arguments: json!({ "query": "image?" }),
                        provider_metadata: json!({}),
                    }],
                    finish_reason: Some(ProviderFinishReason::ToolCall),
                    ..ProviderInvocationResult::default()
                },
                first_token_at: None,
                time_to_first_token_ms: None,
            }),
            1 => Err(anyhow::anyhow!("conflict: model_multimodal_unsupported")),
            _ => Ok(ProviderInvocationOutput {
                events: vec![ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                }],
                result: final_llm_response("main-after"),
                first_token_at: None,
                time_to_first_token_ms: None,
            }),
        }
    }
}

#[async_trait]
impl CapabilityInvoker for MountedModelUnsupportedInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("visible internal model unsupported test does not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for MountedModelUnsupportedInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("visible internal model unsupported test does not execute code nodes")
    }
}

struct ResumeMountedModelUnsupportedInvoker {
    captured_inputs: Arc<Mutex<Vec<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderInvoker for ResumeMountedModelUnsupportedInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        let mut captured = self
            .captured_inputs
            .lock()
            .expect("captured inputs mutex poisoned");
        let call_index = captured.len();
        captured.push(input);
        drop(captured);

        match call_index {
            0 => Err(anyhow::anyhow!("conflict: model_multimodal_unsupported")),
            _ => Ok(ProviderInvocationOutput {
                events: vec![ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                }],
                result: final_llm_response("main-after"),
                first_token_at: None,
                time_to_first_token_ms: None,
            }),
        }
    }
}

#[async_trait]
impl CapabilityInvoker for ResumeMountedModelUnsupportedInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!(
            "visible internal model unsupported resume test does not execute capability nodes"
        )
    }
}

#[async_trait]
impl CodeInvoker for ResumeMountedModelUnsupportedInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("visible internal model unsupported resume test does not execute code nodes")
    }
}

#[tokio::test]
async fn visible_internal_llm_tool_callback_resume_keeps_completed_hidden_tool_results() {
    let mut plan = visible_internal_llm_tool_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("main llm node should exist")
        .config["visible_internal_llm_tools"]
        .as_array_mut()
        .expect("visible internal tools should be configured")
        .push(json!({
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_secondary_context",
            "connector_id": "inspect_secondary_context",
            "description": "Inspect secondary context with the mounted LLM",
            "target_node_id": "node-mounted-llm",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }
        }));

    let (waiting_invoker, _waiting_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![
                ProviderToolCall {
                    id: "call_visible".to_string(),
                    name: "inspect_visible_context".to_string(),
                    arguments: json!({ "query": "first image?" }),
                    provider_metadata: json!({}),
                },
                ProviderToolCall {
                    id: "call_secondary".to_string(),
                    name: "inspect_secondary_context".to_string(),
                    arguments: json!({ "query": "second image?" }),
                    provider_metadata: json!({}),
                },
            ],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("first-mounted "),
        tool_call_response(vec![ProviderToolCall {
            id: "call_bash".to_string(),
            name: "Bash".to_string(),
            arguments: json!({ "command": "file tmp/second-image.png" }),
            provider_metadata: json!({}),
        }]),
    ]);

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "describe the pictures",
                "history": [],
                "tools": [
                    {
                        "name": "Bash",
                        "description": "Run a shell command",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" }
                            },
                            "required": ["command"]
                        }
                    }
                ]
            }
        }),
        &waiting_invoker,
    )
    .await
    .unwrap();

    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("mounted llm tool wait should have checkpoint");
    match waiting.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-mounted-llm");
            assert_eq!(
                pending.request_payload["host_tool_continuation_owner"],
                "agentflow"
            );
            assert_eq!(
                checkpoint.variable_pool["node-mounted-llm"]["__llm_tool_callback"]
                    ["host_tool_continuation_owner"],
                "agentflow"
            );
            assert_eq!(
                pending.request_payload["tool_calls"][0]["id"],
                json!("call_bash")
            );
        }
        other => panic!("expected mounted llm external tool callback wait, got {other:?}"),
    }

    let (resume_invoker, resumed_inputs) = sequential_tool_invoker(vec![
        final_llm_response("second-mounted-after-tool "),
        final_llm_response("main-after"),
    ]);
    let resumed = resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-mounted-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_bash",
                    "content": "tmp/second-image.png: PNG image data"
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(resumed.stop_reason, ExecutionStopReason::Completed),
        "expected completed run, got {:?}",
        resumed.stop_reason
    );
    assert_eq!(
        resumed.variable_pool["node-answer"]["answer"],
        json!("main-before first-mounted second-mounted-after-tool main-after")
    );

    let captured_resumed = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured_resumed.len(), 2);
    let main_recall_messages = &captured_resumed[1].messages;
    let first_hidden_tool_result = main_recall_messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main llm recall should include first hidden tool result");
    assert_eq!(first_hidden_tool_result.content, "first-mounted ");
    let second_hidden_tool_result = main_recall_messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_secondary")
        })
        .expect("main llm recall should include second hidden tool result");
    assert_eq!(
        second_hidden_tool_result.content,
        "second-mounted-after-tool "
    );
}

#[tokio::test]
async fn visible_internal_llm_tool_failure_fails_main_llm_run() {
    let failing_internal_result = ProviderInvocationResult {
        final_content: Some("partial mounted output".to_string()),
        finish_reason: Some(ProviderFinishReason::Error),
        ..ProviderInvocationResult::default()
    };
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some(String::new()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({}),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        failing_internal_result,
    ]);

    let outcome = start_flow_debug_run(
        &visible_internal_llm_tool_plan(),
        &json!({ "node-start": { "query": "describe the picture" } }),
        &invoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("visible_internal_llm_tool_failed")
            );
            assert_eq!(
                failure.error_payload["target_node_id"],
                json!("node-mounted-llm")
            );
        }
        other => panic!("expected failed visible internal llm tool run, got {other:?}"),
    }
}

#[tokio::test]
async fn external_tool_calls_still_wait_for_client_when_internal_tools_are_configured() {
    let (invoker, captured_inputs) =
        sequential_tool_invoker(vec![tool_call_response(vec![ProviderToolCall {
            id: "call_external".to_string(),
            name: "lookup_weather".to_string(),
            arguments: json!({ "city": "Shanghai" }),
            provider_metadata: json!({}),
        }])]);

    let outcome = start_flow_debug_run(
        &visible_internal_llm_tool_plan(),
        &json!({
            "node-start": {
                "query": "weather?",
                "history": [
                    {
                        "role": "user",
                        "content": "上一轮看过 uploads/image_aionui_1781014667000.png"
                    }
                ],
                "tools": [
                    {
                        "name": "lookup_weather",
                        "description": "Lookup weather",
                        "input_schema": { "type": "object" }
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.callback_kind, "llm_tool_calls");
            assert_eq!(
                pending.request_payload["tool_calls"][0]["name"],
                json!("lookup_weather")
            );
        }
        other => panic!("expected external llm tool callback wait, got {other:?}"),
    }

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    let tool_names = captured[0]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"inspect_visible_context"));
    assert!(tool_names.contains(&"lookup_weather"));
}
