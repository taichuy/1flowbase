use super::visible_internal_llm_tool_fixtures::*;
use super::*;

struct CrossProviderVisibleToolInvoker {
    inner: SequentialLlmToolCallInvoker,
}

#[async_trait]
impl ProviderInvoker for CrossProviderVisibleToolInvoker {
    async fn resolve_llm_route(
        &self,
        runtime: &CompiledLlmRuntime,
    ) -> Result<ResolvedProviderRoute> {
        let capabilities = if runtime.provider_code == "deepseek" {
            [
                "message_blocks.reasoning_history.v1".to_string(),
                "message_blocks.redacted_reasoning_history.v1".to_string(),
            ]
            .into_iter()
            .collect()
        } else {
            Default::default()
        };
        Ok(ResolvedProviderRoute::new(capabilities, ()))
    }

    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.inner.invoke_llm(runtime, input).await
    }
}

#[async_trait]
impl CapabilityInvoker for CrossProviderVisibleToolInvoker {
    async fn invoke_capability_node(
        &self,
        runtime: &CompiledPluginRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        self.inner
            .invoke_capability_node(runtime, config_payload, input_payload)
            .await
    }
}

#[async_trait]
impl CodeInvoker for CrossProviderVisibleToolInvoker {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        self.inner
            .invoke_code_node(runtime, config_payload, input_payload)
            .await
    }
}

fn configure_image_llm_tool(plan: &mut CompiledPlan) {
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["tool_name"] = json!("image_llm");
    main_llm.config["visible_internal_llm_tools"][0]["connector_id"] = json!("image_llm");
    main_llm.config["visible_internal_llm_tools"][0]["preconditions"] = json!([
        {
            "kind": "media_content_available",
            "argument_path": ["media"],
            "media_kind": "image"
        }
    ]);
    main_llm.config["visible_internal_llm_tools"][0]["input_schema"] = json!({
        "type": "object",
        "properties": {
            "task": { "type": "string" },
            "media": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["image"] },
                        "source": { "type": "string", "enum": ["workspace_path"] },
                        "path": { "type": "string" }
                    },
                    "required": ["kind", "source", "path"]
                }
            }
        },
        "required": ["task"]
    });
    let mounted_llm = plan
        .nodes
        .get_mut("node-mounted-llm")
        .expect("mounted llm node should exist");
    mounted_llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
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
}

#[tokio::test]
async fn missing_workspace_image_path_waits_for_client_read_without_invoking_image_llm() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![tool_call_response(vec![
        ProviderToolCall {
            id: "call_image".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        },
        ProviderToolCall {
            id: "call_read".to_string(),
            name: "Read".to_string(),
            arguments: json!({ "file_path": "E:\\code\\project\\uploads\\windows-only.png" }),
            provider_metadata: json!({}),
        },
    ])]);
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 找一下这幅图相关代码",
                "history": [],
                "tools": [
                    {
                        "name": "Read",
                        "description": "Read a file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" }
                            },
                            "required": ["file_path"]
                        }
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
            assert_eq!(pending.node_id, "node-llm");
            let pending_tool_names = pending.request_payload["tool_calls"]
                .as_array()
                .expect("pending request should include external tool calls")
                .iter()
                .filter_map(|tool_call| tool_call["name"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(pending_tool_names, vec!["Read"]);
        }
        other => panic!("expected external Read callback wait, got {other:?}"),
    }

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(
        captured.len(),
        1,
        "missing server-side workspace media must not invoke the mounted image LLM before client Read returns media"
    );
    let image_tool = captured[0]
        .tools
        .iter()
        .find(|tool| tool["function"]["name"] == json!("image_llm"))
        .expect("main model should receive the routed image tool");
    assert!(
        image_tool["function"].get("preconditions").is_none()
            && image_tool["function"]["parameters"]
                .get("preconditions")
                .is_none(),
        "runtime preconditions must stay out of provider function parameters"
    );

    let main_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("main llm trace should exist");
    let route_events = main_trace.debug_payload["visible_internal_llm_tool_events"]
        .as_array()
        .expect("main debug payload should include route events");
    assert!(route_events.iter().any(|event| {
        event["event_type"] == json!("visible_internal_llm_tool_failed")
            && event["error_payload"]["details"]["error_code"]
                == json!("visible_internal_llm_tool_media_unavailable")
            && event["error_payload"]["details"]["precondition"]["kind"]
                == json!("media_content_available")
            && event["error_payload"]["details"]["precondition"]["argument_path"]
                == json!(["media"])
    }));
}

#[tokio::test]
async fn empty_saved_media_precondition_still_waits_for_client_read() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![tool_call_response(vec![
        ProviderToolCall {
            id: "call_image".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        },
        ProviderToolCall {
            id: "call_read".to_string(),
            name: "Read".to_string(),
            arguments: json!({ "file_path": "E:\\code\\project\\uploads\\windows-only.png" }),
            provider_metadata: json!({}),
        },
    ])]);
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);
    plan.nodes
        .get_mut("node-llm")
        .expect("main llm node should exist")
        .config["visible_internal_llm_tools"][0]["preconditions"] = json!([{}]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 找一下这幅图相关代码",
                "history": [],
                "tools": [
                    {
                        "name": "Read",
                        "description": "Read a file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" }
                            },
                            "required": ["file_path"]
                        }
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
            assert_eq!(pending.node_id, "node-llm");
            let pending_tool_names = pending.request_payload["tool_calls"]
                .as_array()
                .expect("pending request should include external tool calls")
                .iter()
                .filter_map(|tool_call| tool_call["name"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(pending_tool_names, vec!["Read"]);
        }
        other => panic!("expected external Read callback wait, got {other:?}"),
    }

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(
        captured.len(),
        1,
        "empty saved precondition rows must still inherit the media input schema guard"
    );
}

#[tokio::test]
async fn missing_workspace_image_path_returns_short_guidance_without_error_flag() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_image".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        }]),
        final_llm_response("main-after-guidance"),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 描述这张图片",
                "history": []
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected main llm to continue after media guidance, got {:?}",
        outcome.stop_reason
    );
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(
        captured.len(),
        2,
        "missing server-side workspace media must not invoke the mounted image LLM before media content is available"
    );
    let tool_result = captured[1]
        .messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_image")
        })
        .expect("main recall should receive media guidance as tool result");
    assert_eq!(tool_result.is_error, None);
    assert!(
        tool_result
            .content
            .contains("read the file with a client file tool first"),
        "expected actionable media guidance, got {:?}",
        tool_result.content
    );
    assert!(!tool_result.content.contains("\"details\""));
    assert!(!tool_result
        .content
        .contains("visible internal LLM tool branch node failed"));

    let main_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("main llm trace should exist");
    let route_events = main_trace.debug_payload["visible_internal_llm_tool_events"]
        .as_array()
        .expect("main debug payload should include route events");
    assert!(route_events.iter().any(|event| {
        event["event_type"] == json!("visible_internal_llm_tool_failed")
            && event["error_payload"]["details"]["error_code"]
                == json!("visible_internal_llm_tool_media_unavailable")
    }));
}

#[tokio::test]
async fn client_read_image_callback_reenables_image_llm_retry_after_media_guidance() {
    let (waiting_invoker, waiting_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_image".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        }]),
        tool_call_response(vec![ProviderToolCall {
            id: "call_read".to_string(),
            name: "Read".to_string(),
            arguments: json!({ "file_path": "E:\\code\\project\\uploads\\windows-only.png" }),
            provider_metadata: json!({}),
        }]),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 找一下这幅图相关代码",
                "history": [],
                "tools": [
                    {
                        "name": "Read",
                        "description": "Read a file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" }
                            },
                            "required": ["file_path"]
                        }
                    }
                ]
            }
        }),
        &waiting_invoker,
    )
    .await
    .unwrap();

    let checkpoint = match waiting.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-llm");
            let pending_tool_names = pending.request_payload["tool_calls"]
                .as_array()
                .expect("pending request should include external tool calls")
                .iter()
                .filter_map(|tool_call| tool_call["name"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(pending_tool_names, vec!["Read"]);
            waiting
                .checkpoint_snapshot
                .clone()
                .expect("external Read wait should checkpoint")
        }
        other => panic!("expected external Read callback wait, got {other:?}"),
    };

    let waiting_captured = waiting_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(waiting_captured.len(), 2);
    let before_read_tool_names = waiting_captured[1]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        before_read_tool_names,
        vec!["Read", "Read_run_0"],
        "image_llm should stay hidden until the client Read callback supplies media"
    );

    let (resume_invoker, resumed_inputs) =
        sequential_tool_invoker(vec![final_llm_response("main-after-read")]);
    resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_read",
                    "name": "Read",
                    "content": [
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
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    let resumed_captured = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(resumed_captured.len(), 1);
    let after_read_tool_names = resumed_captured[0]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(
        after_read_tool_names.contains(&"image_llm"),
        "image_llm must be available after the client Read callback returns image content blocks, got {after_read_tool_names:?}"
    );
    assert_eq!(
        resumed_captured[0].run_context["visible_internal_llm_media_tools"][0]["name"],
        json!("image_llm"),
        "provider invocation should mark visible internal media tools so text-model media fallback can guide the main model to retry the routed tool"
    );
}

#[tokio::test]
async fn client_read_image_callback_feeds_retry_media_blocks_to_agent_mounted_llm() {
    let (waiting_invoker, _waiting_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_image".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        }]),
        tool_call_response(vec![ProviderToolCall {
            id: "call_read".to_string(),
            name: "Read".to_string(),
            arguments: json!({ "file_path": "E:\\code\\project\\uploads\\windows-only.png" }),
            provider_metadata: json!({}),
        }]),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["tool_mode"] = json!("agent");
    main_llm.config["visible_internal_llm_tools"][0]["external_tool_policy"] = json!("forbidden");

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 找一下这幅图相关代码",
                "history": [],
                "tools": [
                    {
                        "name": "Read",
                        "description": "Read a file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" }
                            },
                            "required": ["file_path"]
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
        .expect("external Read wait should checkpoint");

    let (resume_invoker, resumed_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_image_retry".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "根据 Read 返回的图片内容描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        }]),
        final_llm_response("mounted-visible "),
        final_llm_response("main-after-image"),
    ]);
    resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_read",
                    "name": "Read",
                    "content": [
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
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    let resumed_captured = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(resumed_captured.len(), 3);
    let retry_tool_names = resumed_captured[0]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(
        retry_tool_names.contains(&"image_llm"),
        "image_llm must be available for retry after client Read supplies image content blocks, got {retry_tool_names:?}"
    );

    let mounted_input = &resumed_captured[1];
    assert!(
        mounted_input.tools.is_empty(),
        "agent image_llm with external_tool_policy forbidden must not inherit Read/Bash tools, got {:?}",
        mounted_input.tools
    );
    let mounted_user_message = mounted_input
        .messages
        .iter()
        .rev()
        .find(|message| message.role == ProviderMessageRole::User)
        .expect("mounted LLM should have a user prompt message");
    let media_blocks = mounted_user_message
        .content_blocks
        .as_ref()
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        media_blocks.iter().any(|block| {
            block["type"] == json!("image")
                && block["source"]["media_type"] == json!("image/png")
                && block["source"]["data"] == json!("aW1hZ2U=")
        }),
        "mounted image LLM user prompt should receive inherited Read image content blocks, got {:?}",
        mounted_input.messages
    );
}

#[tokio::test]
async fn missing_workspace_image_path_reuses_inherited_image_content_blocks() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_image".to_string(),
                name: "image_llm".to_string(),
                arguments: json!({
                    "task": "描述这张图片",
                    "media": [
                        {
                            "kind": "image",
                            "source": "workspace_path",
                            "path": "uploads/windows-only.png"
                        }
                    ]
                }),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("mounted-visible "),
        final_llm_response("main-after"),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 找一下这幅图相关代码",
                "history": [
                    {
                        "role": "user",
                        "content": "uploads\\windows-only.png 找一下这幅图相关代码"
                    },
                    {
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [
                            {
                                "id": "call_read",
                                "name": "Read",
                                "arguments": {
                                    "file_path": "E:\\code\\project\\uploads\\windows-only.png"
                                }
                            }
                        ]
                    },
                    {
                        "role": "tool",
                        "tool_call_id": "call_read",
                        "name": "Read",
                        "content": "",
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

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    let mounted_input = &captured[1];
    let media_blocks = mounted_input
        .messages
        .iter()
        .filter_map(|message| message.content_blocks.as_ref())
        .flat_map(|content_blocks| content_blocks.as_array().into_iter().flatten())
        .collect::<Vec<_>>();
    assert!(
        media_blocks.iter().any(|block| {
            block["type"] == json!("image")
                && block["source"]["media_type"] == json!("image/png")
                && block["source"]["data"] == json!("aW1hZ2U=")
        }),
        "mounted image LLM should receive inherited image content blocks when workspace_path is not readable by the server, got {:?}",
        mounted_input.messages
    );
}

#[tokio::test]
async fn ac_001_cross_provider_image_llm_does_not_inherit_parent_reasoning_semantics() {
    let (inner, captured_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_image".to_string(),
            name: "image_llm".to_string(),
            arguments: json!({
                "task": "描述这张图片",
                "media": [
                    {
                        "kind": "image",
                        "source": "workspace_path",
                        "path": "uploads/windows-only.png"
                    }
                ]
            }),
            provider_metadata: json!({}),
        }]),
        final_llm_response("mounted-visible"),
        final_llm_response("main-after-image"),
    ]);
    let invoker = CrossProviderVisibleToolInvoker { inner };
    let mut plan = visible_internal_llm_tool_plan();
    configure_image_llm_tool(&mut plan);
    let main_runtime = plan
        .nodes
        .get_mut("node-llm")
        .and_then(|node| node.llm_runtime.as_mut())
        .expect("main llm runtime should exist");
    main_runtime.provider_code = "deepseek".to_string();
    main_runtime.protocol = "openai_compatible".to_string();
    main_runtime.model = "deepseek-v4-pro".to_string();
    let mounted_runtime = plan
        .nodes
        .get_mut("node-mounted-llm")
        .and_then(|node| node.llm_runtime.as_mut())
        .expect("mounted llm runtime should exist");
    mounted_runtime.provider_code = "openai".to_string();
    mounted_runtime.protocol = "openai_responses".to_string();
    mounted_runtime.model = "gpt-5.6-luna".to_string();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "uploads\\windows-only.png 找一下这幅图相关代码",
                "history": [
                    {
                        "role": "user",
                        "content": "uploads\\windows-only.png 找一下这幅图相关代码"
                    },
                    {
                        "role": "assistant",
                        "content": "我先看图。",
                        "content_blocks": [
                            { "type": "reasoning", "text": "parent private reasoning" },
                            { "type": "reasoning_redacted", "data": "parent opaque reasoning" },
                            { "type": "text", "text": "我先看图。" }
                        ]
                    },
                    {
                        "role": "tool",
                        "tool_call_id": "call_read",
                        "name": "Read",
                        "content": "",
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
    .expect("cross-provider image_llm flow should execute");

    assert!(
        outcome.variable_pool["node-answer"]["answer"]
            .as_str()
            .is_some_and(|answer| answer.ends_with("main-after-image")),
        "main provider must resume after the image provider result, got {:?}",
        outcome.variable_pool["node-answer"]["answer"]
    );
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3, "mounted image provider must be invoked");
    let main_input = &captured[0];
    assert!(main_input.required_capabilities.contains(
        &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksReasoningHistoryV1
    ));
    assert!(main_input.required_capabilities.contains(
        &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1
    ));
    let mounted_input = &captured[1];
    assert_eq!(mounted_input.provider_code, "openai");
    assert!(
        !mounted_input.required_capabilities.contains(
            &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksReasoningHistoryV1
        )
    );
    assert!(
        !mounted_input.required_capabilities.contains(
            &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1
        )
    );
    let mounted_block_types = mounted_input
        .messages
        .iter()
        .filter_map(|message| message.content_blocks.as_ref())
        .flat_map(|blocks| blocks.as_array().into_iter().flatten())
        .filter_map(|block| block.get("type").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(mounted_block_types.contains(&"text"));
    assert!(mounted_block_types.contains(&"image"));
    assert!(!mounted_block_types.contains(&"reasoning"));
    assert!(!mounted_block_types.contains(&"reasoning_redacted"));
}
