use super::*;

#[tokio::test]
async fn visible_internal_llm_tool_outputs_visible_text_and_recalls_main_llm() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
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
    let plan = visible_internal_llm_tool_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "describe the picture",
                "history": [
                    {
                        "role": "user",
                        "content": "describe the picture",
                        "content_blocks": [
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": "aW1hZ2U="
                                }
                            }
                        ]
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed run, got {:?}",
        outcome.stop_reason
    );
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("main-before mounted-visible main-after")
    );
    assert!(outcome
        .node_traces
        .iter()
        .all(|trace| trace.node_id != "node-mounted-llm"));

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    assert_eq!(
        captured[0].tools[0]["function"]["name"],
        json!("inspect_visible_context")
    );
    assert_eq!(
        captured[1].messages[0]
            .content_blocks
            .as_ref()
            .expect("mounted llm should receive media content blocks")[0]["type"],
        json!("image")
    );
    assert!(captured[2].messages.iter().any(|message| {
        message
            .tool_calls
            .as_ref()
            .and_then(|tool_calls| tool_calls.get(0))
            .and_then(|tool_call| tool_call.get("id"))
            == Some(&json!("call_visible"))
    }));
    let tool_result = captured[2]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main llm recall should include the internal tool result");
    assert_eq!(tool_result.content, "mounted-visible ");
}

#[tokio::test]
async fn visible_internal_llm_tool_executes_composed_connector_branch() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
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
    let plan = visible_internal_llm_tool_chain_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "describe the picture", "history": [] } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed run, got {:?}",
        outcome.stop_reason
    );
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("main-before mounted-visible main-after")
    );
    assert!(
        outcome
            .node_traces
            .iter()
            .all(|trace| trace.node_id != "node-tool-transform"
                && trace.node_id != "node-mounted-llm")
    );

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    assert_eq!(
        captured[1].messages[0].content,
        "Inspect transformed image?"
    );
    let tool_result = captured[2]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main llm recall should include the internal tool result");
    assert_eq!(tool_result.content, "mounted-visible ");
}

#[tokio::test]
async fn visible_internal_llm_tool_branch_commits_conversation_updates_on_sync_completion() {
    let (invoker, _) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({ "query": "update context" }),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("mounted-visible "),
        final_llm_response("main-after"),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    plan.topological_order.insert(
        plan.topological_order
            .iter()
            .position(|node_id| node_id == "node-tool-result")
            .expect("tool result should be ordered"),
        "node-conversation-update".to_string(),
    );
    plan.nodes
        .get_mut("node-mounted-llm")
        .expect("mounted LLM should exist")
        .downstream_node_ids = vec!["node-conversation-update".to_string()];
    plan.nodes
        .get_mut("node-tool-result")
        .expect("tool result should exist")
        .dependency_node_ids = vec!["node-conversation-update".to_string()];
    plan.nodes.insert(
        "node-conversation-update".to_string(),
        CompiledNode {
            node_id: "node-conversation-update".to_string(),
            node_type: "variable_assigner".to_string(),
            alias: "Update Conversation".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-mounted-llm".to_string()],
            downstream_node_ids: vec!["node-tool-result".to_string()],
            bindings: BTreeMap::from([(
                "operations".to_string(),
                CompiledBinding {
                    kind: "state_write".to_string(),
                    selector_paths: Vec::new(),
                    raw_value: json!([{
                        "path": ["conversation", "branch_status"],
                        "operator": "set",
                        "value": {
                            "kind": "constant",
                            "value": "updated-by-internal-tool"
                        }
                    }]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "branch_status".to_string(),
                title: "conversation.branch_status".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    plan.edges
        .iter_mut()
        .find(|edge| edge.edge_id == "edge-mounted-tool-result")
        .expect("mounted LLM edge should exist")
        .target = "node-conversation-update".to_string();
    plan.edges.push(CompiledEdge {
        edge_id: "edge-conversation-update-tool-result".to_string(),
        source: "node-conversation-update".to_string(),
        target: "node-tool-result".to_string(),
        source_handle: None,
        target_handle: None,
    });

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "conversation": { "branch_status": "before" },
            "node-start": { "query": "update context", "history": [] }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["conversation"]["branch_status"],
        json!("updated-by-internal-tool")
    );
}

#[tokio::test]
async fn visible_internal_llm_tool_returns_tool_result_node_content() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
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
    let plan = visible_internal_llm_tool_plan_with_result();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "describe the picture", "history": [] } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed run, got {:?}",
        outcome.stop_reason
    );
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("main-before tool-result: mounted-visible main-after")
    );

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    let tool_result = captured[2]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main llm recall should include the explicit tool result");
    assert_eq!(tool_result.content, "tool-result: mounted-visible ");
}

#[tokio::test]
async fn visible_internal_llm_tool_branch_llm_can_wait_for_external_tool_callback() {
    let (waiting_invoker, waiting_inputs) = sequential_tool_invoker(vec![
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
    let mut plan = visible_internal_llm_tool_plan();
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["external_tool_policy"] = json!("inherited");
    main_llm.config["visible_internal_llm_tools"][0]["target_node_id"] =
        json!("node-conversation-update");
    main_llm
        .downstream_node_ids
        .retain(|node_id| node_id != "node-mounted-llm");
    main_llm
        .downstream_node_ids
        .push("node-conversation-update".to_string());
    plan.topological_order.insert(
        plan.topological_order
            .iter()
            .position(|node_id| node_id == "node-mounted-llm")
            .expect("mounted LLM should be ordered"),
        "node-conversation-update".to_string(),
    );
    plan.nodes
        .get_mut("node-mounted-llm")
        .expect("mounted LLM should exist")
        .dependency_node_ids = vec!["node-conversation-update".to_string()];
    plan.nodes.insert(
        "node-conversation-update".to_string(),
        CompiledNode {
            node_id: "node-conversation-update".to_string(),
            node_type: "variable_assigner".to_string(),
            alias: "Update Conversation".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-mounted-llm".to_string()],
            bindings: BTreeMap::from([(
                "operations".to_string(),
                CompiledBinding {
                    kind: "state_write".to_string(),
                    selector_paths: Vec::new(),
                    raw_value: json!([{
                        "path": ["conversation", "branch_status"],
                        "operator": "set",
                        "value": {
                            "kind": "constant",
                            "value": "updated-before-callback"
                        }
                    }]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "branch_status".to_string(),
                title: "conversation.branch_status".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    let mounted_edge = plan
        .edges
        .iter_mut()
        .find(|edge| edge.edge_id == "edge-llm-visible-tool-mounted")
        .expect("mounted tool edge should exist");
    mounted_edge.target = "node-conversation-update".to_string();
    plan.edges.push(CompiledEdge {
        edge_id: "edge-conversation-update-mounted".to_string(),
        source: "node-conversation-update".to_string(),
        target: "node-mounted-llm".to_string(),
        source_handle: None,
        target_handle: None,
    });

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "conversation": { "branch_status": "before" },
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

    match waiting.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-mounted-llm");
            assert_eq!(pending.callback_kind, "llm_tool_calls");
            assert_eq!(
                pending.request_payload["tool_calls"][0]["name"],
                json!("Bash")
            );
        }
        other => panic!("expected mounted llm external tool callback wait, got {other:?}"),
    }

    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("mounted llm tool wait should have checkpoint");
    assert_eq!(
        checkpoint.variable_pool["conversation"]["branch_status"],
        json!("updated-before-callback")
    );
    let main_wait_trace = waiting
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("main llm waiting trace should exist");
    let route_events = main_wait_trace.debug_payload["visible_internal_llm_tool_events"]
        .as_array()
        .expect("main waiting trace should include visible internal route events");
    assert!(route_events.iter().any(|event| {
        event["event_type"] == json!("visible_internal_llm_tool_waiting_callback")
            && event["waiting_node_id"] == json!("node-mounted-llm")
            && event["request_payload"]["tool_calls"][0]["name"] == json!("Bash")
    }));
    let captured_waiting = waiting_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured_waiting.len(), 2);
    let mounted_tool_names = captured_waiting[1]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(mounted_tool_names.contains(&"Bash"));

    let (resume_invoker, resumed_inputs) = sequential_tool_invoker(vec![
        final_llm_response("mounted-after-tool "),
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
        "expected completed run, got {:?}",
        resumed.stop_reason
    );
    assert_eq!(
        resumed.variable_pool["node-answer"]["answer"],
        json!("main-before mounted-after-tool main-after")
    );
    assert_eq!(
        resumed.variable_pool["conversation"]["branch_status"],
        json!("updated-before-callback")
    );

    let captured_resumed = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured_resumed.len(), 2);
    assert_eq!(
        captured_resumed[0].messages.last().unwrap().role,
        ProviderMessageRole::Tool
    );
    assert_eq!(
        captured_resumed[0]
            .messages
            .last()
            .unwrap()
            .tool_call_id
            .as_deref(),
        Some("call_bash")
    );
    let main_internal_tool_result = captured_resumed[1]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main llm recall should include mounted llm output as hidden tool result");
    assert_eq!(main_internal_tool_result.content, "mounted-after-tool ");
}

#[tokio::test]
async fn fusion_visible_internal_llm_tool_blocks_external_callback_wait() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
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
            arguments: json!({ "command": "pwd" }),
            provider_metadata: json!({}),
        }]),
        final_llm_response("main-after"),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["tool_mode"] = json!("fusion");
    main_llm.config["visible_internal_llm_tools"][0]["external_tool_policy"] = json!("forbidden");
    main_llm.config["visible_internal_llm_tools"][0]["external_callback_policy"] =
        json!("forbidden");
    main_llm.config["visible_internal_llm_tools"][0]["execution_mode"] =
        json!("bounded_parallel_panel");

    let outcome = start_flow_debug_run(
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
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "fusion mode should return an internal tool error to the main LLM instead of waiting, got {:?}",
        outcome.stop_reason
    );
    assert!(outcome.checkpoint_snapshot.is_none());
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("main-before main-after")
    );

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    assert!(
        captured[1].tools.is_empty(),
        "fusion mounted LLM must not inherit run-context tools, got {:?}",
        captured[1].tools
    );
    let main_tool_result = captured[2]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main LLM recall should include the fusion callback rejection");
    assert!(main_tool_result
        .content
        .contains("external callback is forbidden"));
}

#[tokio::test]
async fn fusion_visible_internal_llm_tool_executes_panel_llms_in_bounded_parallel() {
    let invoker = FusionPanelTimingInvoker::default();
    let max_panel_inflight = invoker.max_panel_inflight.clone();
    let plan = fusion_panel_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "compare panel answers",
                "history": []
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed fusion panel run, got {:?}",
        outcome.stop_reason
    );
    assert_eq!(
        max_panel_inflight.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "fusion panel LLMs should overlap within the bounded parallel executor"
    );
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("main-before judge-result main-after")
    );
    let main_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("main llm trace should exist");
    let route_events = main_trace.debug_payload["visible_internal_llm_tool_events"]
        .as_array()
        .expect("main debug payload should include route events");
    for node_id in ["node-panel-a", "node-panel-b"] {
        let panel_event = route_events
            .iter()
            .find(|event| {
                event["event_type"] == json!("visible_internal_llm_tool_completed")
                    && event["node_id"] == json!(node_id)
            })
            .expect("panel LLM completed event should exist");
        assert_eq!(panel_event["node_type"], json!("llm"));
        assert!(
            panel_event["input_payload"]["prompt_messages"].is_array(),
            "panel LLM event should preserve node-run-like input payload"
        );
        assert!(
            panel_event["debug_payload"].is_object(),
            "panel LLM event should preserve node-run-like debug payload"
        );
        assert!(
            panel_event["output_payload"]["text"].is_string(),
            "panel LLM event should preserve node-run-like output payload"
        );
    }
    let judge_event = route_events
        .iter()
        .find(|event| {
            event["event_type"] == json!("visible_internal_llm_tool_completed")
                && event["node_id"] == json!("node-judge")
        })
        .expect("fusion summary LLM completed event should exist");
    assert_eq!(judge_event["node_type"], json!("llm"));
    assert!(
        judge_event["input_payload"]["prompt_messages"].is_array(),
        "fusion summary LLM event should preserve node-run-like input payload"
    );
    assert_eq!(
        judge_event["output_payload"]["text"],
        json!("judge-result ")
    );
    assert_eq!(
        judge_event["metrics_payload"]["usage"]["total_tokens"],
        json!(24)
    );

    let captured = invoker
        .captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 5);
    let main_recall = captured
        .last()
        .expect("main recall should be the final provider call");
    let tool_result = main_recall
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main LLM recall should include the fusion judge result");
    assert_eq!(tool_result.content, "judge-result ");
}

#[tokio::test]
async fn fusion_visible_internal_llm_tool_executes_direct_panel_roots_in_bounded_parallel() {
    let invoker = FusionPanelTimingInvoker::default();
    let max_panel_inflight = invoker.max_panel_inflight.clone();
    let plan = direct_fusion_panel_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "compare panel answers",
                "history": []
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed direct fusion panel run, got {:?}",
        outcome.stop_reason
    );
    assert_eq!(
        max_panel_inflight.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "direct fusion panel roots should overlap within the bounded parallel executor"
    );
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("main-before judge-result main-after")
    );
}

#[tokio::test]
async fn visible_internal_llm_tool_branch_canonicalizes_inherited_tool_name_case() {
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
            name: "bash".to_string(),
            arguments: json!({ "command": "pwd" }),
            provider_metadata: json!({}),
        }]),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("main llm node should exist")
        .config["visible_internal_llm_tools"][0]["external_tool_policy"] = json!("inherited");

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

    match waiting.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-mounted-llm");
            assert_eq!(
                pending.request_payload["tool_calls"][0]["name"],
                json!("Bash")
            );
        }
        other => panic!("expected mounted llm external tool callback wait, got {other:?}"),
    }
    let main_wait_trace = waiting
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("main llm waiting trace should exist");
    assert_eq!(
        main_wait_trace.debug_payload["visible_internal_llm_tool_events"][1]["request_payload"]
            ["tool_calls"][0]["name"],
        json!("Bash")
    );
}

#[tokio::test]
async fn visible_internal_llm_tool_branch_inherits_run_context_query_when_argument_is_only_task() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({ "task": "describe the image" }),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("mounted-visible "),
        final_llm_response("main-after"),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    let mounted_llm = plan
        .nodes
        .get_mut("node-mounted-llm")
        .expect("mounted llm node should exist");
    mounted_llm.config = json!({
        "model_provider": {
            "provider_code": "fixture_provider",
            "model_id": "gpt-5.4-mini"
        },
        "context_policy": {
            "integration_context": "enabled"
        }
    });
    mounted_llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![vec![
                "visible_internal_llm_tool".to_string(),
                "arguments".to_string(),
                "task".to_string(),
            ]],
            raw_value: json!([
                {
                    "id": "mounted-user",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "{{ visible_internal_llm_tool.arguments.task }}"
                    }
                }
            ]),
        },
    )]);

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "调用 image_llm 看看 tmp/frontstage-layout-preview.png 内容是什么",
                "history": [],
                "files": [
                    {
                        "path": "tmp/frontstage-layout-preview.png",
                        "media_type": "image/png"
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    let mounted_messages = &captured[1].messages;
    assert!(
        mounted_messages.iter().any(|message| message
            .content
            .contains("tmp/frontstage-layout-preview.png")),
        "mounted LLM should inherit original run query/files context, got {mounted_messages:?}"
    );
}
