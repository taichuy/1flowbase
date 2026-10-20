use super::*;

pub(super) fn base_plan() -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-llm".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "query".to_string(),
                title: "用户输入".to_string(),
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
    nodes.insert(
        "node-llm".to_string(),
        CompiledNode {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            alias: "LLM".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-human".to_string()],
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "prompt_messages".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                    raw_value: json!([
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
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "模型输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "provider_instance_id": "provider-ready",
                "model": "gpt-5.4-mini"
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
    nodes.insert(
        "node-human".to_string(),
        CompiledNode {
            node_id: "node-human".to_string(),
            node_type: "human_input".to_string(),
            alias: "Human Input".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-answer".to_string()],
            bindings: BTreeMap::from([(
                "prompt".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "templated_text".to_string(),
                    selector_paths: vec![vec!["node-llm".to_string(), "text".to_string()]],
                    raw_value: json!("请审核：{{ node-llm.text }}"),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "input".to_string(),
                title: "人工输入".to_string(),
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
    nodes.insert(
        "node-answer".to_string(),
        CompiledNode {
            node_id: "node-answer".to_string(),
            node_type: "answer".to_string(),
            alias: "Answer".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-human".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::from([(
                "answer_template".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "selector".to_string(),
                    selector_paths: vec![vec!["node-human".to_string(), "input".to_string()]],
                    raw_value: json!(["node-human", "input"]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "answer".to_string(),
                title: "对话输出".to_string(),
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

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec![
            "node-start".to_string(),
            "node-llm".to_string(),
            "node-human".to_string(),
            "node-answer".to_string(),
        ],
        edges: Vec::new(),
        nodes,
        compile_issues: Vec::new(),
    }
}

pub(super) fn llm_answer_plan() -> CompiledPlan {
    let mut plan = base_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-answer".to_string(),
    ];
    plan.nodes.remove("node-human");
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.downstream_node_ids = vec!["node-answer".to_string()];
    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.dependency_node_ids = vec!["node-llm".to_string()];
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "selector".to_string(),
            selector_paths: vec![vec!["node-llm".to_string(), "text".to_string()]],
            raw_value: json!(["node-llm", "text"]),
        },
    )]);
    plan
}

pub(super) fn multi_llm_answer_plan() -> CompiledPlan {
    let mut plan = llm_answer_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-llm-2".to_string(),
        "node-answer".to_string(),
    ];

    let first_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("first llm node should exist");
    first_llm.downstream_node_ids = vec!["node-llm-2".to_string()];

    plan.nodes.insert(
        "node-llm-2".to_string(),
        CompiledNode {
            node_id: "node-llm-2".to_string(),
            node_type: "llm".to_string(),
            alias: "LLM2".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-answer".to_string()],
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "prompt_messages".to_string(),
                    selector_paths: vec![vec!["node-llm".to_string(), "text".to_string()]],
                    raw_value: json!([
                        {
                            "id": "user-2",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "Continue from {{ node-llm.text }}"
                            }
                        }
                    ]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "模型输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "provider_instance_id": "provider-ready",
                "model": "gpt-5.4-mini"
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

    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.dependency_node_ids = vec!["node-llm-2".to_string()];
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "selector".to_string(),
            selector_paths: vec![vec!["node-llm-2".to_string(), "text".to_string()]],
            raw_value: json!(["node-llm-2", "text"]),
        },
    )]);

    plan
}

pub(super) fn tool_call_response(tool_calls: Vec<ProviderToolCall>) -> ProviderInvocationResult {
    ProviderInvocationResult {
        final_content: Some("need tools".to_string()),
        tool_calls,
        finish_reason: Some(ProviderFinishReason::ToolCall),
        usage: ProviderUsage {
            input_tokens: Some(11),
            input_cache_hit_tokens: Some(5),
            output_tokens: Some(3),
            total_tokens: Some(14),
            ..ProviderUsage::default()
        },
        ..ProviderInvocationResult::default()
    }
}

pub(super) fn final_llm_response(text: &str) -> ProviderInvocationResult {
    ProviderInvocationResult {
        final_content: Some(text.to_string()),
        finish_reason: Some(ProviderFinishReason::Stop),
        usage: ProviderUsage {
            input_tokens: Some(20),
            input_cache_hit_tokens: Some(8),
            output_tokens: Some(4),
            total_tokens: Some(24),
            ..ProviderUsage::default()
        },
        ..ProviderInvocationResult::default()
    }
}

pub(super) fn sequential_tool_invoker(
    responses_in_call_order: Vec<ProviderInvocationResult>,
) -> (
    SequentialLlmToolCallInvoker,
    Arc<Mutex<Vec<ProviderInvocationInput>>>,
) {
    sequential_tool_output_invoker(
        responses_in_call_order
            .into_iter()
            .map(provider_output)
            .collect(),
    )
}

pub(super) fn sequential_tool_output_invoker(
    responses_in_call_order: Vec<ProviderInvocationOutput>,
) -> (
    SequentialLlmToolCallInvoker,
    Arc<Mutex<Vec<ProviderInvocationInput>>>,
) {
    let captured_inputs = Arc::new(Mutex::new(Vec::new()));
    let invoker = SequentialLlmToolCallInvoker {
        responses: Arc::new(Mutex::new(
            responses_in_call_order.into_iter().rev().collect(),
        )),
        captured_inputs: captured_inputs.clone(),
    };
    (invoker, captured_inputs)
}

pub(super) fn provider_output(result: ProviderInvocationResult) -> ProviderInvocationOutput {
    ProviderInvocationOutput {
        events: result
            .finish_reason
            .clone()
            .map(|reason| ProviderStreamEvent::Finish { reason })
            .into_iter()
            .collect(),
        result,
        first_token_at: None,
        time_to_first_token_ms: None,
    }
}
