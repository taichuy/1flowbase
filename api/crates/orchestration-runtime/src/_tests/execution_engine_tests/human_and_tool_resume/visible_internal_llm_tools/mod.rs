use super::visible_internal_llm_tool_fixtures::*;
use super::*;

#[derive(Default)]
struct FusionPanelTimingInvoker {
    captured_inputs: Arc<Mutex<Vec<ProviderInvocationInput>>>,
    current_panel_inflight: Arc<std::sync::atomic::AtomicUsize>,
    max_panel_inflight: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl ProviderInvoker for FusionPanelTimingInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.captured_inputs
            .lock()
            .expect("captured inputs mutex poisoned")
            .push(input.clone());

        let prompt_text = input
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let has_internal_tool = input
            .tools
            .iter()
            .any(|tool| tool["function"]["name"] == json!("inspect_visible_context"));
        let has_internal_tool_result = input.messages.iter().any(|message| {
            message.role == ProviderMessageRole::Tool
                && message.tool_call_id.as_deref() == Some("call_visible")
        });

        if has_internal_tool && !has_internal_tool_result {
            return Ok(provider_output(ProviderInvocationResult {
                final_content: Some("main-before ".to_string()),
                tool_calls: vec![ProviderToolCall {
                    id: "call_visible".to_string(),
                    name: "inspect_visible_context".to_string(),
                    arguments: json!({ "query": "compare panel answers" }),
                    provider_metadata: json!({}),
                }],
                finish_reason: Some(ProviderFinishReason::ToolCall),
                ..ProviderInvocationResult::default()
            }));
        }

        if prompt_text.contains("Panel A") || prompt_text.contains("Panel B") {
            let current = self
                .current_panel_inflight
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            self.max_panel_inflight
                .fetch_max(current, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            self.current_panel_inflight
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            let content = if prompt_text.contains("Panel A") {
                "panel-a "
            } else {
                "panel-b "
            };
            return Ok(provider_output(final_llm_response(content)));
        }

        if prompt_text.contains("Judge") {
            return Ok(provider_output(final_llm_response("judge-result ")));
        }

        Ok(provider_output(final_llm_response("main-after")))
    }
}

#[async_trait]
impl CapabilityInvoker for FusionPanelTimingInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("fusion panel timing test does not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for FusionPanelTimingInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("fusion panel timing test does not execute code nodes")
    }
}

fn fusion_panel_plan() -> CompiledPlan {
    let mut plan = llm_answer_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-panel-seed".to_string(),
        "node-panel-a".to_string(),
        "node-panel-b".to_string(),
        "node-judge".to_string(),
        "node-tool-result".to_string(),
        "node-answer".to_string(),
    ];
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-llm".to_string(),
            source: "node-start".to_string(),
            target: "node-llm".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-fusion-tool".to_string(),
            source: "node-llm".to_string(),
            target: "node-panel-seed".to_string(),
            source_handle: Some("visible_internal_llm_tool:inspect_visible_context".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-seed-panel-a".to_string(),
            source: "node-panel-seed".to_string(),
            target: "node-panel-a".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-seed-panel-b".to_string(),
            source: "node-panel-seed".to_string(),
            target: "node-panel-b".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-panel-a-judge".to_string(),
            source: "node-panel-a".to_string(),
            target: "node-judge".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-panel-b-judge".to_string(),
            source: "node-panel-b".to_string(),
            target: "node-judge".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-judge-tool-result".to_string(),
            source: "node-judge".to_string(),
            target: "node-tool-result".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];

    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.downstream_node_ids = vec!["node-answer".to_string(), "node-panel-seed".to_string()];
    main_llm.config["visible_internal_llm_tools_enabled"] = json!(true);
    main_llm.config["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "tool_mode": "fusion",
            "internal_llm_node_policy": "allowed",
            "external_tool_policy": "forbidden",
            "external_callback_policy": "forbidden",
            "execution_mode": "bounded_parallel_panel",
            "description": "Compare panel model answers",
            "target_node_id": "node-panel-seed",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }
        }
    ]);

    plan.nodes.insert(
        "node-panel-seed".to_string(),
        CompiledNode {
            node_id: "node-panel-seed".to_string(),
            node_type: "template_transform".to_string(),
            alias: "Panel Seed".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-panel-a".to_string(), "node-panel-b".to_string()],
            bindings: BTreeMap::from([(
                "template".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "templated_text".to_string(),
                    selector_paths: vec![vec![
                        "visible_internal_llm_tool".to_string(),
                        "arguments".to_string(),
                        "query".to_string(),
                    ]],
                    raw_value: json!("{{ visible_internal_llm_tool.arguments.query }}"),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "Panel Seed".to_string(),
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
    plan.nodes.insert(
        "node-panel-a".to_string(),
        fusion_panel_llm_node("node-panel-a", "Panel A"),
    );
    plan.nodes.insert(
        "node-panel-b".to_string(),
        fusion_panel_llm_node("node-panel-b", "Panel B"),
    );
    plan.nodes.insert(
        "node-judge".to_string(),
        CompiledNode {
            node_id: "node-judge".to_string(),
            node_type: "llm".to_string(),
            alias: "Judge LLM".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-panel-a".to_string(), "node-panel-b".to_string()],
            downstream_node_ids: vec!["node-tool-result".to_string()],
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "prompt_messages".to_string(),
                    selector_paths: vec![
                        vec!["node-panel-a".to_string(), "text".to_string()],
                        vec!["node-panel-b".to_string(), "text".to_string()],
                    ],
                    raw_value: json!([
                        {
                            "id": "judge-user",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "Judge {{ node-panel-a.text }} / {{ node-panel-b.text }}"
                            }
                        }
                    ]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "Judge Output".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                }
            }),
            plugin_runtime: None,
            llm_runtime: Some(CompiledLlmRuntime {
                provider_instance_id: "provider-ready".to_string(),
                provider_instance_display_name: String::new(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "gpt-5.4-mini".to_string(),
                routing: None,
            }),
            code_runtime: None,
        },
    );
    plan.nodes.insert(
        "node-tool-result".to_string(),
        CompiledNode {
            node_id: "node-tool-result".to_string(),
            node_type: "tool_result".to_string(),
            alias: "Tool Result".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-judge".to_string()],
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::from([(
                "result_template".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "templated_text".to_string(),
                    selector_paths: vec![vec!["node-judge".to_string(), "text".to_string()]],
                    raw_value: json!("{{ node-judge.text }}"),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "result".to_string(),
                title: "Tool Result".to_string(),
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
    plan
}

fn direct_fusion_panel_plan() -> CompiledPlan {
    let mut plan = fusion_panel_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-panel-a".to_string(),
        "node-panel-b".to_string(),
        "node-judge".to_string(),
        "node-tool-result".to_string(),
        "node-answer".to_string(),
    ];
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-llm".to_string(),
            source: "node-start".to_string(),
            target: "node-llm".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-panel-a".to_string(),
            source: "node-llm".to_string(),
            target: "node-panel-a".to_string(),
            source_handle: Some("visible_internal_llm_tool:inspect_visible_context".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-panel-b".to_string(),
            source: "node-llm".to_string(),
            target: "node-panel-b".to_string(),
            source_handle: Some("visible_internal_llm_tool:inspect_visible_context".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-panel-a-judge".to_string(),
            source: "node-panel-a".to_string(),
            target: "node-judge".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-panel-b-judge".to_string(),
            source: "node-panel-b".to_string(),
            target: "node-judge".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-judge-tool-result".to_string(),
            source: "node-judge".to_string(),
            target: "node-tool-result".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];
    plan.nodes.remove("node-panel-seed");
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.downstream_node_ids = vec![
        "node-answer".to_string(),
        "node-panel-a".to_string(),
        "node-panel-b".to_string(),
    ];
    main_llm.config["visible_internal_llm_tools"][0]["target_node_id"] = json!("node-panel-a");
    main_llm.config["visible_internal_llm_tools"][0]["target_node_ids"] =
        json!(["node-panel-a", "node-panel-b"]);
    for node_id in ["node-panel-a", "node-panel-b"] {
        let panel_node = plan
            .nodes
            .get_mut(node_id)
            .expect("panel node should exist");
        panel_node.dependency_node_ids = vec!["node-llm".to_string()];
    }
    plan
}

fn fusion_panel_llm_node(node_id: &str, prompt_prefix: &str) -> CompiledNode {
    CompiledNode {
        node_id: node_id.to_string(),
        node_type: "llm".to_string(),
        alias: prompt_prefix.to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-panel-seed".to_string()],
        downstream_node_ids: vec!["node-judge".to_string()],
        bindings: BTreeMap::from([(
            "prompt_messages".to_string(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "prompt_messages".to_string(),
                selector_paths: vec![vec![
                    "visible_internal_llm_tool".to_string(),
                    "arguments".to_string(),
                    "query".to_string(),
                ]],
                raw_value: json!([
                    {
                        "id": format!("{node_id}-user"),
                        "role": "user",
                        "content": {
                            "kind": "templated_text",
                            "value": format!("{prompt_prefix}: {{{{ visible_internal_llm_tool.arguments.query }}}}")
                        }
                    }
                ]),
            },
        )]),
        outputs: vec![CompiledOutput {
            key: "text".to_string(),
            title: "Panel Output".to_string(),
            value_type: "string".to_string(),
            selector: Vec::new(),
            json_schema: None,
        }],
        config: json!({
            "model_provider": {
                "provider_code": "fixture_provider",
                "model_id": "gpt-5.4-mini"
            }
        }),
        plugin_runtime: None,
        llm_runtime: Some(CompiledLlmRuntime {
            provider_instance_id: "provider-ready".to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        }),
        code_runtime: None,
    }
}

mod media_context;
mod text_and_branch_execution;
mod tool_policy;
