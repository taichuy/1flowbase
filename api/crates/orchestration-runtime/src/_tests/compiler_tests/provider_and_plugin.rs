use super::*;

#[test]
fn compile_rejects_edge_that_targets_unknown_node() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["edges"][0]["target"] = json!("missing-node");

    let error =
        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap_err();

    assert!(error.to_string().contains("missing-node"));
}

#[test]
fn compile_rejects_internal_public_output_keys() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["outputs"] =
        json!([{ "key": "__trace", "title": "Trace", "valueType": "json" }]);

    let error =
        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap_err();

    assert!(format!("{error:#}").contains("__trace"));
}

#[test]
fn compile_collects_provider_compile_issues() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"] = json!({
        "model_provider": {
            "provider_code": "fixture_provider",
            "model_id": "unknown-model"
        }
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(plan.compile_issues.len(), 1);
    assert_eq!(
        plan.compile_issues[0].code,
        CompileIssueCode::ModelNotAvailable
    );
}

#[test]
fn compile_visible_internal_llm_tool_entry_from_source_handle_edge() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "internal_llm_node_policy": "allowed",
            "input_schema": { "type": "object" }
        }
    ]);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-mounted-llm",
            "type": "llm",
            "alias": "Mounted LLM",
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                }
            },
            "bindings": {
                "prompt_messages": {
                    "kind": "prompt_messages",
                    "value": [
                        {
                            "id": "mounted-user",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "{{ node-start.query }}"
                            }
                        }
                    ]
                }
            },
            "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
        }));
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-tool-result",
            "type": "tool_result",
            "alias": "Tool Result",
            "description": "",
            "containerId": null,
            "position": { "x": 720, "y": 0 },
            "configVersion": 1,
            "config": {},
            "bindings": {
                "result_template": {
                    "kind": "templated_text",
                    "value": "{{ node-mounted-llm.text }}"
                }
            },
            "outputs": [{ "key": "result", "title": "Tool Result", "valueType": "string" }]
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-llm-visible-tool-mounted",
            "source": "node-llm",
            "target": "node-mounted-llm",
            "sourceHandle": "visible_internal_llm_tool:inspect_visible_context",
            "targetHandle": null,
            "containerId": null
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-mounted-tool-result",
            "source": "node-mounted-llm",
            "target": "node-tool-result",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null
        }));

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let main_llm = plan.nodes.get("node-llm").expect("main llm should compile");
    let mounted_llm = plan
        .nodes
        .get("node-mounted-llm")
        .expect("mounted llm should compile");

    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    assert_eq!(
        main_llm.config["visible_internal_llm_tools"][0]["target_node_id"],
        json!("node-mounted-llm")
    );
    assert!(plan.edges.iter().any(|edge| {
        edge.source == "node-llm"
            && edge.target == "node-mounted-llm"
            && edge.source_handle.as_deref()
                == Some("visible_internal_llm_tool:inspect_visible_context")
    }));
    assert!(main_llm
        .downstream_node_ids
        .contains(&"node-mounted-llm".to_string()));
    assert!(mounted_llm
        .dependency_node_ids
        .contains(&"node-llm".to_string()));
    assert!(plan
        .nodes
        .get("node-tool-result")
        .expect("tool result should compile")
        .dependency_node_ids
        .contains(&"node-mounted-llm".to_string()));
}

#[test]
fn compile_materializes_visible_internal_llm_tool_mode_policies() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "agent_tool",
            "connector_id": "agent_tool",
            "tool_mode": "agent",
            "external_tool_policy": "inherited",
            "input_schema": { "type": "object" }
        },
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "fusion_tool",
            "connector_id": "fusion_tool",
            "tool_mode": "fusion",
            "external_tool_policy": "inherited",
            "input_schema": { "type": "object" }
        },
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "legacy_tool",
            "connector_id": "legacy_tool",
            "external_tool_policy": "inherited",
            "input_schema": { "type": "object" }
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let tools = plan.nodes["node-llm"].config["visible_internal_llm_tools"]
        .as_array()
        .expect("visible tools should compile as an array");

    assert_eq!(tools[0]["tool_mode"], json!("agent"));
    assert_eq!(tools[0]["external_tool_policy"], json!("inherited"));
    assert_eq!(tools[0]["external_callback_policy"], json!("inherited"));
    assert_eq!(tools[0]["execution_mode"], json!("sequential_resume"));
    assert_eq!(tools[1]["tool_mode"], json!("fusion"));
    assert_eq!(tools[1]["external_tool_policy"], json!("forbidden"));
    assert_eq!(tools[1]["external_callback_policy"], json!("forbidden"));
    assert_eq!(tools[1]["execution_mode"], json!("bounded_parallel_panel"));
    assert_eq!(tools[2]["tool_mode"], json!("agent"));
    assert_eq!(tools[2]["external_tool_policy"], json!("inherited"));
    assert_eq!(tools[2]["external_callback_policy"], json!("inherited"));
    assert_eq!(tools[2]["execution_mode"], json!("sequential_resume"));
}

#[test]
fn compile_materializes_fusion_visible_internal_llm_tool_direct_panel_roots() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "problem_review",
            "connector_id": "problem_review",
            "tool_mode": "fusion",
            "internal_llm_node_policy": "allowed",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string" },
                    "background": { "type": "string" }
                }
            }
        }
    ]);

    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array");
    for (node_id, prompt) in [
        ("node-panel-a", "Panel A"),
        ("node-panel-b", "Panel B"),
        ("node-panel-c", "Panel C"),
    ] {
        nodes.push(json!({
            "id": node_id,
            "type": "llm",
            "alias": prompt,
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                }
            },
            "bindings": {
                "prompt_messages": {
                    "kind": "prompt_messages",
                    "value": [
                        {
                            "id": format!("{node_id}-user"),
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": format!("{prompt}: {{{{ visible_internal_llm_tool.arguments.task }}}}")
                            }
                        }
                    ]
                }
            },
            "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
        }));
    }
    nodes.push(json!({
        "id": "node-judge",
        "type": "llm",
        "alias": "Judge",
        "description": "",
        "containerId": null,
        "position": { "x": 720, "y": 0 },
        "configVersion": 1,
        "config": {
            "model_provider": {
                "provider_code": "fixture_provider",
                "model_id": "gpt-5.4-mini"
            }
        },
        "bindings": {
            "prompt_messages": {
                "kind": "prompt_messages",
                "value": [
                    {
                        "id": "judge-user",
                        "role": "user",
                        "content": {
                            "kind": "templated_text",
                            "value": "Judge {{ node-panel-a.text }} / {{ node-panel-b.text }} / {{ node-panel-c.text }}"
                        }
                    }
                ]
            }
        },
        "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
    }));
    nodes.push(json!({
        "id": "node-tool-result",
        "type": "tool_result",
        "alias": "Tool Result",
        "description": "",
        "containerId": null,
        "position": { "x": 960, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {
            "result_template": {
                "kind": "templated_text",
                "value": "{{ node-judge.text }}"
            }
        },
        "outputs": [{ "key": "result", "title": "Tool Result", "valueType": "string" }]
    }));

    let edges = document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array");
    for target in ["node-panel-a", "node-panel-b", "node-panel-c"] {
        edges.push(json!({
            "id": format!("edge-llm-{target}"),
            "source": "node-llm",
            "target": target,
            "sourceHandle": "visible_internal_llm_tool:problem_review",
            "targetHandle": null,
            "containerId": null
        }));
    }
    for source in ["node-panel-a", "node-panel-b", "node-panel-c"] {
        edges.push(json!({
            "id": format!("edge-{source}-judge"),
            "source": source,
            "target": "node-judge",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null
        }));
    }
    edges.push(json!({
        "id": "edge-judge-tool-result",
        "source": "node-judge",
        "target": "node-tool-result",
        "sourceHandle": null,
        "targetHandle": null,
        "containerId": null
    }));

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let tools = plan.nodes["node-llm"].config["visible_internal_llm_tools"]
        .as_array()
        .expect("visible tools should compile as an array");

    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    assert_eq!(tools[0]["target_node_id"], json!("node-panel-a"));
    assert_eq!(
        tools[0]["target_node_ids"],
        json!(["node-panel-a", "node-panel-b", "node-panel-c"])
    );
    assert_eq!(tools[0]["external_tool_policy"], json!("forbidden"));
    assert_eq!(tools[0]["external_callback_policy"], json!("forbidden"));
    assert_eq!(tools[0]["execution_mode"], json!("bounded_parallel_panel"));
}

#[test]
fn compile_flags_visible_internal_llm_tool_without_tool_result_node() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "input_schema": { "type": "object" }
        }
    ]);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-mounted-llm",
            "type": "llm",
            "alias": "Mounted LLM",
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                }
            },
            "bindings": {
                "prompt_messages": {
                    "kind": "prompt_messages",
                    "value": [
                        {
                            "id": "mounted-user",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "{{ node-start.query }}"
                            }
                        }
                    ]
                }
            },
            "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-llm-visible-tool-mounted",
            "source": "node-llm",
            "target": "node-mounted-llm",
            "sourceHandle": "visible_internal_llm_tool:inspect_visible_context",
            "targetHandle": null,
            "containerId": null
        }));

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool
            && issue.node_id == "node-llm"
            && issue.message.contains("tool_result")
    }));
}

#[test]
fn compile_flags_visible_internal_llm_tool_branch_llm_without_tool_allowed_policy() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "input_schema": { "type": "object" }
        }
    ]);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-mounted-llm",
            "type": "llm",
            "alias": "Mounted LLM",
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                }
            },
            "bindings": {
                "prompt_messages": {
                    "kind": "prompt_messages",
                    "value": [
                        {
                            "id": "mounted-user",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "{{ node-start.query }}"
                            }
                        }
                    ]
                }
            },
            "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
        }));
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-tool-result",
            "type": "tool_result",
            "alias": "Tool Result",
            "description": "",
            "containerId": null,
            "position": { "x": 720, "y": 0 },
            "configVersion": 1,
            "config": {},
            "bindings": {
                "result_template": {
                    "kind": "templated_text",
                    "value": "{{ node-mounted-llm.text }}"
                }
            },
            "outputs": [{ "key": "result", "title": "Tool Result", "valueType": "string" }]
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-llm-visible-tool-mounted",
            "source": "node-llm",
            "target": "node-mounted-llm",
            "sourceHandle": "visible_internal_llm_tool:inspect_visible_context",
            "targetHandle": null,
            "containerId": null
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-mounted-tool-result",
            "source": "node-mounted-llm",
            "target": "node-tool-result",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null
        }));

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool
            && issue.node_id == "node-llm"
            && issue.message.contains("internal_llm_node_policy")
    }));

    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"][0]
        ["internal_llm_node_policy"] = json!("allowed");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(!plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool
            && issue.node_id == "node-llm"
            && issue.message.contains("internal_llm_node_policy")
    }));
}

#[test]
fn compile_flags_visible_internal_llm_tool_without_connector_edge() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "input_schema": { "type": "object" }
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool && issue.node_id == "node-llm"
    }));
}

#[test]
fn compile_uses_selected_instance_models_instead_of_provider_family_aggregate() {
    let flow_id = Uuid::now_v7();
    let mut context = compile_context();
    context.provider_families.insert(
        "fixture_provider".to_string(),
        FlowCompileProviderFamily {
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            is_ready: true,
            available_models: BTreeSet::from([
                "gpt-5.4-mini".to_string(),
                "other-model".to_string(),
            ]),
            allow_custom_models: false,
        },
    );
    context.provider_instances.insert(
        "provider-selected".to_string(),
        FlowCompileProviderInstance {
            provider_instance_id: "provider-selected".to_string(),
            display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            is_ready: true,
            is_runnable: true,
            included_in_main: true,
            available_models: BTreeSet::from(["other-model".to_string()]),
            allow_custom_models: false,
        },
    );

    let plan =
        FlowCompiler::compile(flow_id, "draft-1", &sample_document(flow_id), &context).unwrap();

    assert!(plan
        .compile_issues
        .iter()
        .any(|issue| issue.code == CompileIssueCode::ModelNotAvailable));
}

#[test]
fn compile_failover_queue_routes_with_frozen_targets() {
    let flow_id = Uuid::now_v7();
    let mut context = compile_context();
    context.provider_instances.insert(
        "provider-backup".to_string(),
        FlowCompileProviderInstance {
            provider_instance_id: "provider-backup".to_string(),
            display_name: "Backup Provider".to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            is_ready: true,
            is_runnable: true,
            included_in_main: true,
            available_models: BTreeSet::from(["backup-model".to_string()]),
            allow_custom_models: false,
        },
    );
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["model_provider"] = json!({
        "routing_mode": "failover_queue",
        "queue_template_id": "queue-template-1",
        "queue_snapshot_id": "queue-snapshot-1",
        "queue_targets": [
            {
                "provider_instance_id": "provider-selected",
                "provider_code": "fixture_provider",
                "protocol": "openai_compatible",
                "upstream_model_id": "gpt-5.4-mini"
            },
            {
                "provider_instance_id": "provider-backup",
                "provider_code": "fixture_provider",
                "protocol": "openai_compatible",
                "upstream_model_id": "backup-model"
            }
        ]
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &context).unwrap();
    let plan_json = serde_json::to_value(&plan).unwrap();
    let routing = &plan_json["nodes"]["node-llm"]["llm_runtime"]["routing"];

    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    assert_eq!(routing["routing_mode"], json!("failover_queue"));
    assert_eq!(routing["queue_template_id"], json!("queue-template-1"));
    assert_eq!(routing["queue_snapshot_id"], json!("queue-snapshot-1"));
    assert_eq!(
        routing["queue_targets"][0]["upstream_model_id"],
        json!("gpt-5.4-mini")
    );
    assert_eq!(
        routing["queue_targets"][1]["provider_instance_id"],
        json!("provider-backup")
    );
    assert_eq!(
        routing["queue_targets"][1]["provider_instance_display_name"],
        json!("Backup Provider")
    );
}

#[test]
fn compile_ignores_legacy_source_instance_id_in_failover_targets() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["model_provider"] = json!({
        "routing_mode": "failover_queue",
        "queue_template_id": "queue-template-1",
        "queue_targets": [
            {
                "source_instance_id": "provider-selected",
                "provider_code": "fixture_provider",
                "protocol": "openai_compatible",
                "upstream_model_id": "gpt-5.4-mini"
            }
        ]
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let routing = plan.nodes["node-llm"]
        .llm_runtime
        .as_ref()
        .and_then(|runtime| runtime.routing.as_ref())
        .expect("llm runtime should still expose the failover routing shell");

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::MissingProviderInstance
            && issue
                .message
                .contains("failover target 0 is missing provider_instance_id")
    }));
    assert_eq!(
        routing.queue_targets.len(),
        0,
        "legacy source_instance_id must not compile into a runnable target"
    );
}

#[test]
fn compile_collects_missing_provider_issue() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["model_provider"]["provider_code"] = Value::Null;

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan
        .compile_issues
        .iter()
        .any(|issue| issue.code == CompileIssueCode::MissingProviderInstance));
}

#[test]
fn compile_routes_duplicate_stable_provider_model_binding_as_ordered_targets() {
    let flow_id = Uuid::now_v7();
    let mut context = compile_context();
    context.provider_instances.insert(
        "provider-recreated".to_string(),
        FlowCompileProviderInstance {
            provider_instance_id: "provider-recreated".to_string(),
            display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            is_ready: true,
            is_runnable: true,
            included_in_main: true,
            available_models: BTreeSet::from(["gpt-5.4-mini".to_string()]),
            allow_custom_models: false,
        },
    );

    let plan =
        FlowCompiler::compile(flow_id, "draft-1", &sample_document(flow_id), &context).unwrap();
    let plan_json = serde_json::to_value(&plan).unwrap();
    let routing = &plan_json["nodes"]["node-llm"]["llm_runtime"]["routing"];

    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    assert_eq!(routing["routing_mode"], json!("failover_queue"));
    assert_eq!(routing["queue_template_id"], Value::Null);
    assert_eq!(
        routing["queue_targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|target| target["provider_instance_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["provider-recreated", "provider-selected"]
    );
    assert_eq!(
        routing["queue_targets"][0]["upstream_model_id"],
        json!("gpt-5.4-mini")
    );
}

#[test]
fn compile_rejects_legacy_top_level_llm_config_shape() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"] = json!({
        "provider_code": "fixture_provider",
        "source_instance_id": "provider-selected",
        "model": "gpt-5.4-mini"
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan
        .compile_issues
        .iter()
        .any(|issue| issue.code == CompileIssueCode::MissingProviderInstance));
    assert!(plan
        .compile_issues
        .iter()
        .any(|issue| issue.code == CompileIssueCode::MissingModel));
}

#[test]
fn compile_plugin_node_emits_runtime_reference_from_registry_identity() {
    let flow_id = Uuid::now_v7();
    let plugin_context = plugin_compile_context();
    let installation_id = plugin_context
        .node_contributions
        .values()
        .next()
        .expect("plugin contribution should exist")
        .installation_id;
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &plugin_document(flow_id),
        &plugin_context,
    )
    .unwrap();

    let plan_json = serde_json::to_value(&plan).unwrap();

    assert_eq!(
        plan_json["nodes"]["node-plugin"]["node_type"],
        "plugin_node"
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["contribution_code"],
        "openai_prompt"
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["installation_id"],
        installation_id.to_string()
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["plugin_unique_identifier"],
        "prompt_pack"
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["package_id"],
        "prompt_pack@0.1.0"
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["contribution_checksum"],
        "sha256:contribution"
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["compiled_contribution_hash"],
        "sha256:compiled"
    );
    assert_eq!(
        plan_json["nodes"]["node-plugin"]["plugin_runtime"]["output_schema_snapshot"][0]["key"],
        "answer"
    );
}

#[test]
fn compile_plugin_node_reports_dependency_issue_when_registry_information_is_missing() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &plugin_document(flow_id),
        &compile_context(),
    )
    .unwrap();

    assert!(
        plan.compile_issues
            .iter()
            .any(|issue| issue.node_id == "node-plugin"),
        "expected a compile issue for the plugin node, got {:?}",
        plan.compile_issues
    );
}

#[test]
fn compile_plugin_node_rejects_legacy_contribution_schema_version() {
    let flow_id = Uuid::now_v7();
    let mut document = plugin_document(flow_id);
    document["graph"]["nodes"][1]["schema_version"] = json!("1flowbase.node-contribution/v1");

    let plan =
        FlowCompiler::compile(flow_id, "draft-1", &document, &plugin_compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-plugin"
            && issue.code == CompileIssueCode::UnsupportedPluginContributionSchemaVersion
    }));
    assert!(plan.nodes["node-plugin"].plugin_runtime.is_none());
}

#[test]
fn compile_plugin_node_reports_issue_when_contribution_checksum_drifts() {
    let flow_id = Uuid::now_v7();
    let mut document = plugin_document(flow_id);
    document["graph"]["nodes"][1]["contribution_checksum"] = json!("sha256:changed");

    let plan =
        FlowCompiler::compile(flow_id, "draft-1", &document, &plugin_compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-plugin"
            && issue.code == CompileIssueCode::PluginContributionChecksumMismatch
    }));
}

#[test]
fn compile_plugin_node_reports_issue_when_output_schema_snapshot_drifts() {
    let flow_id = Uuid::now_v7();
    let mut document = plugin_document(flow_id);
    document["graph"]["nodes"][1]["output_schema_snapshot"] = json!({
        "outputs": [{ "key": "changed", "title": "Changed", "valueType": "string" }]
    });

    let plan =
        FlowCompiler::compile(flow_id, "draft-1", &document, &plugin_compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-plugin"
            && issue.code == CompileIssueCode::PluginContributionOutputSchemaMismatch
    }));
}

#[test]
fn compile_unresolved_node_reports_blocking_issue() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    let original_node = document["graph"]["nodes"][1].clone();
    document["graph"]["nodes"][1] = json!({
        "id": "node-llm",
        "type": "unresolved_node",
        "alias": "LLM",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {
            "unresolved": {
                "dependency_status": "missing_dependency",
                "reason": "missing_model_provider",
                "original_type": "llm",
                "original_node": original_node
            }
        },
        "bindings": {},
        "outputs": []
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm" && issue.code == CompileIssueCode::UnresolvedNode
    }));
    assert_eq!(plan.nodes["node-llm"].node_type, "unresolved_node");
}

#[test]
fn compile_flags_visible_internal_llm_tool_with_invalid_external_tool_policy() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "external_tool_policy": "open",
            "input_schema": { "type": "object" }
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool
            && issue.node_id == "node-llm"
            && issue.message.contains("external_tool_policy")
    }));

    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"][0]
        ["external_tool_policy"] = json!("inherited");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(!plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool
            && issue.message.contains("external_tool_policy")
    }));
}

#[test]
fn compile_flags_visible_internal_llm_tool_with_invalid_tool_mode() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "inspect_visible_context",
            "connector_id": "inspect_visible_context",
            "tool_mode": "router",
            "input_schema": { "type": "object" }
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.code == CompileIssueCode::InvalidVisibleInternalLlmTool
            && issue.node_id == "node-llm"
            && issue.message.contains("tool_mode")
    }));
}
