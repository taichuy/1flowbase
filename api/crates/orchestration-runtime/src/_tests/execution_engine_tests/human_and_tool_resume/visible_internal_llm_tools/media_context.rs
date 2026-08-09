use super::*;

#[tokio::test]
async fn visible_internal_image_llm_tool_injects_workspace_path_media_blocks() {
    let media_dir = std::env::current_dir()
        .expect("test current dir should be available")
        .join("target")
        .join("visible-internal-media");
    tokio::fs::create_dir_all(&media_dir)
        .await
        .expect("test media dir should be created");
    let image_path = media_dir.join("sample.png");
    tokio::fs::write(&image_path, b"image")
        .await
        .expect("test image should be written");
    let relative_image_path = "target/visible-internal-media/sample.png";

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
                    },
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

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    let main_tool_names = captured[0]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        main_tool_names,
        vec![
            "Bash",
            "Read",
            "Bash_run_0",
            "Read_run_1",
            "inspect_visible_context"
        ]
    );
    let image_tool_schema = captured[0]
        .tools
        .iter()
        .find(|tool| tool["function"]["name"] == json!("inspect_visible_context"))
        .map(|tool| &tool["function"]["parameters"])
        .expect("visible internal media tool schema should be registered");
    assert_eq!(
        image_tool_schema["properties"]["media"]["items"]["properties"]["source"]["enum"][0],
        json!("workspace_path")
    );
    let mounted_input = &captured[1];
    assert!(
        mounted_input.tools.is_empty(),
        "mounted image LLM should not inherit outer client tools when media is present"
    );
    let resumed_main_tool_names = captured[2]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        resumed_main_tool_names,
        vec!["Bash", "Read", "Bash_run_0", "Read_run_1"]
    );
    let media_blocks = mounted_input.messages[0]
        .content_blocks
        .as_ref()
        .expect("mounted image LLM should receive media content blocks")
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
async fn visible_internal_image_llm_tool_ignores_repeated_media_call_when_external_tool_waits() {
    let media_dir = std::env::current_dir()
        .expect("test current dir should be available")
        .join("target")
        .join("visible-internal-media");
    tokio::fs::create_dir_all(&media_dir)
        .await
        .expect("test media dir should be created");
    let image_path = media_dir.join("mixed-repeat.png");
    tokio::fs::write(&image_path, b"image")
        .await
        .expect("test image should be written");
    let relative_image_path = "target/visible-internal-media/mixed-repeat.png";

    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "image_llm".to_string(),
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
        tool_call_response(vec![
            ProviderToolCall {
                id: "call_bash".to_string(),
                name: "Bash".to_string(),
                arguments: json!({ "command": "rg Navigation web/app/src" }),
                provider_metadata: json!({}),
            },
            ProviderToolCall {
                id: "call_visible_again".to_string(),
                name: "image_llm".to_string(),
                arguments: json!({
                    "task": "再描述一次图片中的导航栏",
                    "media": [
                        {
                            "kind": "image",
                            "source": "workspace_path",
                            "path": relative_image_path
                        }
                    ]
                }),
                provider_metadata: json!({}),
            },
        ]),
    ]);
    let mut plan = visible_internal_llm_tool_plan();
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["tool_name"] = json!("image_llm");
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

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": format!("看{} 然后继续查代码", relative_image_path),
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

    match outcome.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-llm");
            assert_eq!(pending.callback_kind, "llm_tool_calls");
            let pending_tool_names = pending.request_payload["tool_calls"]
                .as_array()
                .expect("pending request should include tool calls")
                .iter()
                .filter_map(|tool_call| tool_call["name"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(pending_tool_names, vec!["Bash"]);
        }
        other => panic!("expected main llm external tool callback wait, got {other:?}"),
    }

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    let resumed_main_tool_names = captured[2]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(resumed_main_tool_names, vec!["Bash", "Bash_run_0"]);
}

#[tokio::test]
async fn visible_internal_image_llm_tool_preserves_visible_media_arguments() {
    let media_dir = std::env::current_dir()
        .expect("test current dir should be available")
        .join("target")
        .join("visible-internal-media");
    tokio::fs::create_dir_all(&media_dir)
        .await
        .expect("test media dir should be created");
    let image_path = media_dir.join("sanitize.png");
    tokio::fs::write(&image_path, b"image")
        .await
        .expect("test image should be written");
    let relative_image_path = "target/visible-internal-media/sanitize.png";

    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("main-before ".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "image_llm".to_string(),
                arguments: json!({
                    "task": "看一下这幅图内容是什么",
                    "media": [
                        {
                            "kind": "image",
                            "source": "workspace_path",
                            "path": image_path.to_string_lossy(),
                            "media_type": "image/png",
                            "custom_note": "keep-me"
                        },
                        {
                            "kind": "image",
                            "source": "url",
                            "url": "https://example.test/image.png"
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
    main_llm.config["visible_internal_llm_tools"][0]["tool_name"] = json!("image_llm");
    main_llm.config["visible_internal_llm_tools"][0]["connector_id"] = json!("image_llm");
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

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": format!("看{} 看一下这幅图内容是什么", relative_image_path),
                "history": []
            }
        }),
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
        .expect("main debug payload should include route events");
    assert_eq!(
        route_events[0]["arguments"]["media"],
        json!([
            {
                "kind": "image",
                "source": "workspace_path",
                "path": image_path.to_string_lossy(),
                "media_type": "image/png",
                "custom_note": "keep-me"
            },
            {
                "kind": "image",
                "source": "url",
                "url": "https://example.test/image.png"
            }
        ])
    );
    let persisted_main_payload = serde_json::to_string(&json!([
        main_trace.output_payload,
        main_trace.debug_payload
    ]))
    .expect("trace payload should serialize");
    assert!(persisted_main_payload.contains("keep-me"));
    assert!(persisted_main_payload.contains("media_type"));
    assert!(persisted_main_payload.contains(image_path.to_string_lossy().as_ref()));

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);
    assert!(captured[1].tools.is_empty());
    let media_blocks = captured[1].messages[0]
        .content_blocks
        .as_ref()
        .expect("mounted image LLM should receive media content blocks")
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
async fn visible_internal_image_llm_tool_schema_does_not_synthesize_media_contract() {
    let (invoker, captured_inputs) =
        sequential_tool_invoker(vec![final_llm_response("main-after")]);
    let mut plan = visible_internal_llm_tool_plan();
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["tool_name"] = json!("image_llm");
    main_llm.config["visible_internal_llm_tools"][0]["connector_id"] = json!("image_llm");
    main_llm.config["visible_internal_llm_tools"][0]["input_schema"] = json!({
        "type": "object",
        "properties": {
            "task": { "type": "string" }
        },
        "required": ["task"]
    });

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "看uploads/image_aionui_1781014667000.png 看一下这幅图内容是什么",
                "history": []
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
    assert_eq!(captured.len(), 1);
    let image_tool_schema = captured[0]
        .tools
        .iter()
        .find(|tool| tool["function"]["name"] == json!("image_llm"))
        .map(|tool| &tool["function"]["parameters"])
        .expect("image_llm schema should be registered");
    assert_eq!(
        image_tool_schema,
        &json!({
            "type": "object",
            "properties": {
                "task": { "type": "string" }
            },
            "required": ["task"]
        })
    );
}
