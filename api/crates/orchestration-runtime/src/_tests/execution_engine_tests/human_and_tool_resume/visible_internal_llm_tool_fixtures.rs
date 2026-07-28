use super::*;

pub(super) fn visible_internal_llm_tool_plan_with_result() -> CompiledPlan {
    let mut plan = visible_internal_llm_tool_plan();
    let tool_result = plan
        .nodes
        .get_mut("node-tool-result")
        .expect("tool result node should exist");
    tool_result.bindings = BTreeMap::from([(
        "result_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "templated_text".to_string(),
            selector_paths: vec![vec!["node-mounted-llm".to_string(), "text".to_string()]],
            raw_value: json!("tool-result: {{ node-mounted-llm.text }}"),
        },
    )]);

    plan
}

pub(super) fn visible_internal_llm_tool_plan() -> CompiledPlan {
    let mut plan = llm_answer_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-mounted-llm".to_string(),
        "node-tool-result".to_string(),
        "node-answer".to_string(),
    ];
    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools_enabled"] = json!(true);
    main_llm.config["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "internal_llm_node_policy": "allowed",
            "description": "Inspect the current user content with a mounted LLM",
            "target_node_id": "node-mounted-llm",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }
        }
    ]);
    main_llm
        .downstream_node_ids
        .push("node-mounted-llm".to_string());

    plan.nodes.insert(
        "node-mounted-llm".to_string(),
        CompiledNode {
            node_id: "node-mounted-llm".to_string(),
            node_type: "llm".to_string(),
            alias: "Mounted LLM".to_string(),
            container_id: None,
            dependency_node_ids: Vec::new(),
            downstream_node_ids: vec!["node-tool-result".to_string()],
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
                            "id": "mounted-user",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "Inspect {{ visible_internal_llm_tool.arguments.query }}"
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
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                },
                "context_policy": {
                    "integration_context": "enabled",
                    "context_selector": ["node-start", "history"]
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
            dependency_node_ids: vec!["node-mounted-llm".to_string()],
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::from([(
                "result_template".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "templated_text".to_string(),
                    selector_paths: vec![vec!["node-mounted-llm".to_string(), "text".to_string()]],
                    raw_value: json!("{{ node-mounted-llm.text }}"),
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
    plan.edges.push(CompiledEdge {
        edge_id: "edge-start-llm".to_string(),
        source: "node-start".to_string(),
        target: "node-llm".to_string(),
        source_handle: None,
        target_handle: None,
    });
    plan.edges.push(CompiledEdge {
        edge_id: "edge-llm-answer".to_string(),
        source: "node-llm".to_string(),
        target: "node-answer".to_string(),
        source_handle: None,
        target_handle: None,
    });
    plan.edges.push(CompiledEdge {
        edge_id: "edge-llm-visible-tool-mounted".to_string(),
        source: "node-llm".to_string(),
        target: "node-mounted-llm".to_string(),
        source_handle: Some("visible_internal_llm_tool:inspect_visible_context".to_string()),
        target_handle: None,
    });
    plan.edges.push(CompiledEdge {
        edge_id: "edge-mounted-tool-result".to_string(),
        source: "node-mounted-llm".to_string(),
        target: "node-tool-result".to_string(),
        source_handle: None,
        target_handle: None,
    });

    plan
}

pub(super) fn visible_internal_llm_tool_plan_behind_if_else() -> CompiledPlan {
    let mut plan = visible_internal_llm_tool_plan();
    plan.topological_order.insert(1, "node-if".to_string());
    plan.edges.retain(|edge| edge.edge_id != "edge-start-llm");
    plan.edges.extend([
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
    ]);
    plan.nodes
        .get_mut("node-start")
        .expect("start node should exist")
        .downstream_node_ids = vec!["node-if".to_string()];
    plan.nodes
        .get_mut("node-llm")
        .expect("main llm node should exist")
        .dependency_node_ids = vec!["node-if".to_string()];
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
                    i18n_text_ref: None,
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

pub(super) fn visible_internal_llm_tool_chain_plan() -> CompiledPlan {
    let mut plan = visible_internal_llm_tool_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-tool-transform".to_string(),
        "node-mounted-llm".to_string(),
        "node-tool-result".to_string(),
        "node-answer".to_string(),
    ];

    let main_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("main llm node should exist");
    main_llm.config["visible_internal_llm_tools"][0]["target_node_id"] =
        json!("node-tool-transform");
    main_llm.downstream_node_ids =
        vec!["node-answer".to_string(), "node-tool-transform".to_string()];

    plan.nodes.insert(
        "node-tool-transform".to_string(),
        CompiledNode {
            node_id: "node-tool-transform".to_string(),
            node_type: "template_transform".to_string(),
            alias: "Tool Transform".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-mounted-llm".to_string()],
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
                    raw_value: json!("transformed {{ visible_internal_llm_tool.arguments.query }}"),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "转换结果".to_string(),
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

    let mounted_llm = plan
        .nodes
        .get_mut("node-mounted-llm")
        .expect("mounted llm node should exist");
    mounted_llm.dependency_node_ids = vec!["node-tool-transform".to_string()];
    mounted_llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "prompt_messages".to_string(),
            selector_paths: vec![vec!["node-tool-transform".to_string(), "text".to_string()]],
            raw_value: json!([
                {
                    "id": "mounted-user",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "Inspect {{ node-tool-transform.text }}"
                    }
                }
            ]),
        },
    )]);

    if let Some(edge) = plan
        .edges
        .iter_mut()
        .find(|edge| edge.edge_id == "edge-llm-visible-tool-mounted")
    {
        edge.edge_id = "edge-llm-visible-tool-transform".to_string();
        edge.target = "node-tool-transform".to_string();
    }
    plan.edges.push(CompiledEdge {
        edge_id: "edge-tool-transform-mounted".to_string(),
        source: "node-tool-transform".to_string(),
        target: "node-mounted-llm".to_string(),
        source_handle: None,
        target_handle: None,
    });

    plan
}
