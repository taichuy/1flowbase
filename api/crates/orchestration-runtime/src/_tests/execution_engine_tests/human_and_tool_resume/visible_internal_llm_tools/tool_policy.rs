use super::*;

#[tokio::test]
async fn visible_internal_llm_tool_default_external_tool_policy_blocks_run_context_tools() {
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

    start_flow_debug_run(
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

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    assert!(
        captured[1].tools.is_empty(),
        "mounted LLM without external_tool_policy must default to forbidden and receive no run-context tools, got {:?}",
        captured[1].tools
    );
    let resumed_main_tool_names = captured[2]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(resumed_main_tool_names.contains(&"Bash"));
}

#[tokio::test]
async fn visible_internal_llm_tool_inherited_policy_keeps_run_context_tools_with_media() {
    let media_dir = std::env::current_dir()
        .expect("test current dir should be available")
        .join("target")
        .join("visible-internal-media");
    tokio::fs::create_dir_all(&media_dir)
        .await
        .expect("test media dir should be created");
    let image_path = media_dir.join("inherited-policy.png");
    tokio::fs::write(&image_path, b"image")
        .await
        .expect("test image should be written");
    let relative_image_path = "target/visible-internal-media/inherited-policy.png";

    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({
                    "task": "看一下这幅图内容是什么",
                    "media": [
                        {
                            "kind": "image",
                            "source": "workspace_path",
                            "path": relative_image_path
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
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["external_tool_policy"] = json!("inherited");
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

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": format!("看{} 看一下这幅图内容是什么", relative_image_path),
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

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    let mounted_input = &captured[1];
    let mounted_tool_names = mounted_input
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert!(
        mounted_tool_names.contains(&"Bash"),
        "mounted LLM with external_tool_policy inherited must receive run-context tools even with media, got {mounted_tool_names:?}"
    );
    let media_blocks = mounted_input.messages[0]
        .content_blocks
        .as_ref()
        .expect("mounted LLM should still receive media content blocks")
        .as_array()
        .expect("content blocks should be an array");
    assert!(media_blocks.iter().any(|block| {
        block["type"] == json!("image_url")
            && block["image_url"]["url"]
                .as_str()
                .is_some_and(|url| url.starts_with("data:image/png;base64,"))
    }));
}

#[tokio::test]
async fn visible_internal_llm_tool_mixed_round_runs_internal_inline_and_waits_for_external() {
    let (waiting_invoker, waiting_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![
                ProviderToolCall {
                    id: "call_visible".to_string(),
                    name: "inspect_visible_context".to_string(),
                    arguments: json!({ "query": "image?" }),
                    provider_metadata: json!({}),
                },
                ProviderToolCall {
                    id: "call_bash".to_string(),
                    name: "Bash".to_string(),
                    arguments: json!({ "command": "ls uploads" }),
                    provider_metadata: json!({}),
                },
            ],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("mounted-visible "),
    ]);
    let plan = visible_internal_llm_tool_plan();

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "describe the picture then inspect the repo",
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
            assert_eq!(pending.node_id, "node-llm");
            assert_eq!(pending.callback_kind, "llm_tool_calls");
            let pending_tool_names = pending.request_payload["tool_calls"]
                .as_array()
                .expect("mixed round waiting payload should carry tool calls")
                .iter()
                .filter_map(|tool_call| tool_call["name"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                pending_tool_names,
                vec!["Bash"],
                "internal tool call must not leak into the client-facing waiting payload"
            );
        }
        other => panic!("expected main llm external tool callback wait, got {other:?}"),
    }

    let captured_waiting = waiting_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(
        captured_waiting.len(),
        2,
        "mixed round should execute the mounted branch inline before waiting"
    );

    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("mixed round should checkpoint while waiting for the client tool");

    let (resume_invoker, resumed_inputs) =
        sequential_tool_invoker(vec![final_llm_response("main-after")]);
    let resumed = resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_bash",
                    "content": "image-1.png"
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
        json!("main-before mounted-visible main-after")
    );

    let captured_resumed = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured_resumed.len(), 1);
    let recall_messages = &captured_resumed[0].messages;
    let internal_result = recall_messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        })
        .expect("main llm recall should include the hidden internal tool result");
    assert_eq!(internal_result.content, "mounted-visible ");
    let external_result = recall_messages
        .iter()
        .find(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_bash")
        })
        .expect("main llm recall should include the client tool result");
    assert_eq!(external_result.content, "image-1.png");
}
