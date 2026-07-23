use super::*;

fn plan_with_llm_behind_if_else() -> CompiledPlan {
    let mut plan = base_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-if".to_string(),
        "node-llm".to_string(),
        "node-human".to_string(),
        "node-answer".to_string(),
    ];
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-if".to_string(),
            source: "node-start".to_string(),
            target: "node-if".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-if-llm".to_string(),
            source: "node-if".to_string(),
            target: "node-llm".to_string(),
            source_handle: Some("if".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-human".to_string(),
            source: "node-llm".to_string(),
            target: "node-human".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-human-answer".to_string(),
            source: "node-human".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];
    plan.nodes
        .get_mut("node-start")
        .expect("start node should exist")
        .downstream_node_ids = vec!["node-if".to_string()];
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.dependency_node_ids = vec!["node-if".to_string()];
    plan.nodes.insert(
        "node-if".to_string(),
        CompiledNode {
            node_id: "node-if".to_string(),
            node_type: "if_else".to_string(),
            alias: "If / Else".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-llm".to_string()],
            bindings: BTreeMap::from([(
                "branches".to_string(),
                CompiledBinding {
                    kind: "if_else_branches".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                    raw_value: json!({
                        "branches": [
                            {
                                "id": "if",
                                "kind": "if",
                                "title": "If",
                                "sourceHandle": "if",
                                "condition": {
                                    "operator": "and",
                                    "conditions": [{
                                        "kind": "rule",
                                        "left": ["node-start", "query"],
                                        "comparator": "exists"
                                    }]
                                }
                            },
                            {
                                "id": "else",
                                "kind": "else",
                                "title": "Else",
                                "sourceHandle": "else"
                            }
                        ]
                    }),
                },
            )]),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    plan
}

#[tokio::test]
async fn native_prompt_context_reaches_llm_across_intermediate_nodes_once() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan_with_llm_behind_if_else(),
        &json!({
            "__native_model_prompt_context": {
                "system": [
                    { "type": "text", "text": "Use the external system." }
                ],
                "messages": [
                    { "role": "user", "content": "Earlier question" },
                    { "role": "assistant", "content": "Earlier answer" }
                ]
            },
            "node-start": {
                "query": "Current question",
                "system": [
                    { "type": "text", "text": "Use the external system." }
                ],
                "history": [
                    { "role": "user", "content": "Earlier question" },
                    { "role": "assistant", "content": "Earlier answer" }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(
        input.system_text().as_deref(),
        Some("Use the external system.")
    );
    assert_eq!(input.messages.len(), 3);
    assert_eq!(input.messages[0].role, ProviderMessageRole::User);
    assert_eq!(input.messages[0].content, "Earlier question");
    assert_eq!(input.messages[1].role, ProviderMessageRole::Assistant);
    assert_eq!(input.messages[1].content, "Earlier answer");
    assert_eq!(input.messages[2].role, ProviderMessageRole::User);
    assert_eq!(input.messages[2].content, "Current question");
}

#[tokio::test]
async fn external_tool_callback_recall_keeps_native_prompt_across_intermediate_nodes() {
    const ORIGINAL_REQUIREMENT: &str = "Keep the original UI design requirement.";
    let plan = plan_with_llm_behind_if_else();
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_lookup".to_string(),
            name: "lookup".to_string(),
            arguments: json!({ "query": "UI design" }),
            provider_metadata: json!({}),
        }]),
        final_llm_response("callback complete"),
    ]);

    let waiting = start_flow_debug_run(
        &plan,
        &json!({
            "__native_model_prompt_context": {
                "system": [
                    { "type": "text", "text": "Preserve the AI Native prompt." }
                ],
                "messages": [
                    { "role": "user", "content": ORIGINAL_REQUIREMENT },
                    { "role": "assistant", "content": "I will use a tool." }
                ]
            },
            "node-start": {
                "query": "Current question",
                "history": []
            }
        }),
        &invoker,
    )
    .await
    .unwrap();
    let checkpoint = waiting
        .checkpoint_snapshot
        .expect("tool callback should persist a checkpoint");

    resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [{
                "tool_call_id": "call_lookup",
                "content": "lookup result"
            }]
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 2);
    for input in &captured {
        assert_eq!(
            input
                .messages
                .iter()
                .filter(|message| message.content == ORIGINAL_REQUIREMENT)
                .count(),
            1,
            "every external callback round must keep the original AI Native user turn exactly once: {:?}",
            input.messages
        );
        assert_eq!(
            input.system_text().as_deref(),
            Some("Preserve the AI Native prompt.")
        );
    }
}

#[tokio::test]
async fn compatible_start_context_reaches_llm_across_intermediate_nodes() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan_with_llm_behind_if_else(),
        &json!({
            "node-start": {
                "query": "Current question",
                "system": [
                    { "type": "text", "text": "Use the compatible system." }
                ],
                "history": [
                    { "role": "user", "content": "Earlier question" },
                    { "role": "assistant", "content": "Earlier answer" }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(
        input.system_text().as_deref(),
        Some("Use the compatible system.")
    );
    assert_eq!(input.messages.len(), 3);
    assert_eq!(input.messages[0].content, "Earlier question");
    assert_eq!(input.messages[1].content, "Earlier answer");
    assert_eq!(input.messages[2].content, "Current question");
}

#[tokio::test]
async fn malformed_native_prompt_context_fails_before_provider_invocation() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "should not run".to_string(),
    };

    let error = start_flow_debug_run(
        &base_plan(),
        &json!({
            "__native_model_prompt_context": "invalid",
            "node-start": { "query": "hello" }
        }),
        &invoker,
    )
    .await
    .expect_err("malformed Native context must fail closed");

    assert!(
        error.to_string().contains("__native_model_prompt_context"),
        "unexpected error: {error}"
    );
    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
}

#[tokio::test]
async fn ac_003_prompt_binding_appends_text_block_without_stringifying_system_blocks() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![
                vec!["node-start".to_string(), "system".to_string()],
                vec!["node-start".to_string(), "query".to_string()],
            ],
            raw_value: json!([
                {
                    "id": "system-1",
                    "role": "system",
                    "content": {
                        "kind": "templated_text",
                        "value": "{{node-start.system}}，语言偏好中文"
                    }
                },
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "{{node-start.query}}"
                    }
                }
            ]),
        },
    )]);
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "__native_model_request_context": {
                "end_user_reference": "claude-code-user-123"
            },
            "node-start": {
                "query": "hello",
                "system": [
                    {
                        "type": "text",
                        "text": "Use Claude Code project instructions.",
                        "cache_control": { "type": "ephemeral" }
                    },
                    {
                        "type": "text",
                        "text": "Preserve repository safety rules."
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    let payload = serde_json::to_value(input).unwrap();

    assert_eq!(
        payload["system"],
        json!([
            {
                "type": "text",
                "text": "Use Claude Code project instructions.",
                "cache_control": { "type": "ephemeral" }
            },
            {
                "type": "text",
                "text": "Preserve repository safety rules."
            },
            {
                "type": "text",
                "text": "，语言偏好中文"
            }
        ])
    );
    assert_eq!(
        payload["request_context"]["end_user_reference"],
        json!("claude-code-user-123")
    );
}

#[tokio::test]
async fn llm_runtime_exposes_effective_system_and_promotes_legacy_history_system() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["context_policy"] = json!({
        "integration_context": "enabled"
    });
    llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
            raw_value: json!([
                {
                    "id": "system-1",
                    "role": "system",
                    "content": {
                        "kind": "templated_text",
                        "value": "Use the node policy."
                    }
                },
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "Question: {{ node-start.query }}"
                    }
                }
            ]),
        },
    )]);
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "hello",
                "system": "Use the run policy.",
                "history": [
                    { "role": "system", "content": "Use the legacy history policy." },
                    { "role": "user", "content": "Earlier question" }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(
        input.system_text().as_deref(),
        Some("Use the run policy.\n\nUse the legacy history policy.\n\nUse the node policy.")
    );
    assert_eq!(input.messages.len(), 2);
    assert_eq!(input.messages[0].role, ProviderMessageRole::User);
    assert_eq!(input.messages[0].content, "Earlier question");
    assert_eq!(input.messages[1].role, ProviderMessageRole::User);
    assert_eq!(input.messages[1].content, "Question: hello");

    let trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert_eq!(
        trace.debug_payload["llm_context"]["effective_system"],
        json!([
            { "type": "text", "text": "Use the run policy." },
            { "type": "text", "text": "Use the legacy history policy." },
            { "type": "text", "text": "Use the node policy." }
        ])
    );
    assert_eq!(
        trace.debug_payload["llm_context"]["provider_messages"],
        json!([
            { "role": "user", "content": "Earlier question" },
            { "role": "user", "content": "Question: hello" }
        ])
    );
    assert_eq!(
        trace.debug_payload["llm_context"]["compatibility_promotions"],
        json!([
            {
                "source": "node-start.history",
                "source_kind": "history",
                "message_index": 0,
                "target": "effective_system"
            }
        ])
    );
}

#[tokio::test]
async fn llm_runtime_injects_selected_context_messages() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["context_policy"] = json!({
        "integration_context": "enabled",
        "context_selector": ["node-start", "history"]
    });
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "hello",
                "history": [
                    { "role": "user", "content": "Earlier question" },
                    { "role": "assistant", "content": "Earlier answer" }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::WaitingHuman(_)
    ));
    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(input.messages.len(), 3);
    assert_eq!(input.messages[0].content, "Earlier question");
    assert_eq!(input.messages[1].content, "Earlier answer");
    assert_eq!(input.messages[2].content, "hello");
}

#[tokio::test]
async fn llm_runtime_fails_when_selected_context_value_is_not_messages() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["context_policy"] = json!({
        "integration_context": "enabled",
        "context_selector": ["node-start", "history"]
    });

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "hello",
                "history": [{ "role": "user" }]
            }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("llm_context_selector_error")
            );
        }
        other => panic!("expected llm context selector failure, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_runtime_context_policy_can_disable_run_level_system_context() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["context_policy"] = json!({
        "integration_context": "disabled"
    });
    llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
            raw_value: json!([
                {
                    "id": "system-1",
                    "role": "system",
                    "content": {
                        "kind": "templated_text",
                        "value": "Use only the local node policy."
                    }
                },
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "{{ node-start.query }}"
                    }
                }
            ]),
        },
    )]);
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "hello",
                "system": "Ignored run-level policy.",
                "history": [
                    { "role": "user", "content": "Ignored earlier question" }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(
        input.system_text().as_deref(),
        Some("Use only the local node policy.")
    );
    assert_eq!(input.messages.len(), 1);
    assert_eq!(input.messages[0].content, "hello");
}

#[tokio::test]
async fn llm_runtime_forwards_compatible_tools_and_tool_history_to_provider() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "final answer".to_string(),
    };

    start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": {
                "query": "Final question",
                "history": [
                    {
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [
                            {
                                "id": "call_123",
                                "type": "function",
                                "function": {
                                    "name": "lookup_order",
                                    "arguments": "{\"order_id\":\"A-1\"}"
                                }
                            }
                        ]
                    },
                    {
                        "role": "tool",
                        "tool_call_id": "call_123",
                        "content": "{\"status\":\"shipped\"}"
                    }
                ],
                "tools": [
                    {
                        "name": "lookup_order",
                        "description": "Lookup an order",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "order_id": { "type": "string" }
                            }
                        },
                        "source": "openai_compatible"
                    }
                ],
                "tool_choice": "auto"
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(captured.tools[0]["function"]["name"], json!("lookup_order"));
    assert_eq!(captured.model_parameters["tool_choice"], json!("auto"));
    assert_eq!(
        captured.tools[0]["function"]["parameters"]["properties"]["order_id"]["type"],
        json!("string")
    );

    let messages = serde_json::to_value(&captured.messages).expect("messages serialize");
    assert_eq!(messages[0]["role"], json!("assistant"));
    assert_eq!(messages[0]["tool_calls"][0]["id"], json!("call_123"));
    assert_eq!(messages[1]["role"], json!("tool"));
    assert_eq!(messages[1]["tool_call_id"], json!("call_123"));
    assert_eq!(messages[2]["role"], json!("user"));
    assert_eq!(messages[2]["content"], json!("Final question"));
}

#[tokio::test]
async fn downstream_llm_inherits_run_level_tools_from_start_input() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        final_llm_response("first answer"),
        final_llm_response("second answer"),
    ]);

    let outcome = start_flow_debug_run(
        &multi_llm_answer_plan(),
        &json!({
            "node-start": {
                "query": "List files",
                "tools": [
                    {
                        "name": "list_directory",
                        "description": "List a directory",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            }
                        }
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));

    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 2);
    assert_eq!(
        captured[0].tools[0]["function"]["name"],
        json!("list_directory")
    );
    assert_eq!(
        captured[1].tools[0]["function"]["name"],
        json!("list_directory")
    );
    assert_eq!(
        captured[1].tools[0]["function"]["parameters"]["properties"]["path"]["type"],
        json!("string")
    );
}
