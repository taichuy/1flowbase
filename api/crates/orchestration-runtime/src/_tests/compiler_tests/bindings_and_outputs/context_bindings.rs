use super::*;

#[test]
fn compile_llm_node_ignores_removed_prompt_bindings() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["bindings"]["user_prompt"] =
        json!({ "kind": "selector", "value": ["node-start", "query"] });
    document["graph"]["nodes"][1]["bindings"]["system_prompt"] =
        json!({ "kind": "templated_text", "value": "Legacy system prompt" });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.nodes["node-llm"]
        .bindings
        .contains_key("prompt_messages"));
    assert!(!plan.nodes["node-llm"].bindings.contains_key("user_prompt"));
    assert!(!plan.nodes["node-llm"]
        .bindings
        .contains_key("system_prompt"));
}

#[test]
fn compile_llm_node_keeps_context_policy_in_node_config() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["context_policy"] = json!({
        "integration_context": "enabled",
        "context_selector": ["node-start", "history"]
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    assert_eq!(
        plan.nodes["node-llm"].config["context_policy"],
        json!({ "integration_context": "enabled", "context_selector": ["node-start", "history"] })
    );
    assert!(plan.nodes["node-llm"]
        .llm_runtime
        .as_ref()
        .is_some_and(|runtime| runtime.routing.is_none()));
}

#[test]
fn wp_fua_host_compiled_llm_protocol_context_preserves_three_state_compatibility() {
    let flow_id = Uuid::now_v7();
    let document = sample_document(flow_id);
    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let llm = &plan.nodes["node-llm"];

    let system_protocol_context =
        VariableReference::selector(vec!["sys".to_string(), "protocol_context".to_string()]);
    assert_eq!(
        llm.config["protocol_context"],
        serde_json::to_value(&system_protocol_context).unwrap()
    );
    assert_eq!(
        llm.protocol_context_reference().unwrap(),
        Some(system_protocol_context.clone())
    );

    let mut document = document;
    document["graph"]["nodes"][1]["config"]["protocol_context"] = Value::Null;
    let plan = FlowCompiler::compile(flow_id, "draft-2", &document, &compile_context()).unwrap();
    let llm = &plan.nodes["node-llm"];

    assert_eq!(llm.config["protocol_context"], Value::Null);
    assert_eq!(llm.protocol_context_reference().unwrap(), None);

    document["graph"]["nodes"][1]["config"]["protocol_context"] = json!({
        "kind": "selector",
        "value": ["sys", "protocol_context"]
    });
    let plan = FlowCompiler::compile(flow_id, "draft-3", &document, &compile_context()).unwrap();
    let llm = &plan.nodes["node-llm"];

    assert_eq!(
        llm.config["protocol_context"],
        json!({
            "kind": "selector",
            "value": ["sys", "protocol_context"]
        })
    );
    assert_eq!(
        llm.protocol_context_reference().unwrap(),
        Some(system_protocol_context)
    );
    assert!(!plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm"
            && matches!(
                issue.code,
                CompileIssueCode::InvalidLlmContextSelector
                    | CompileIssueCode::IncompatibleLlmContextSchema
            )
    }));
}

#[test]
fn ac_003_protocol_context_requires_the_nominal_output_type() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    let nodes = document["graph"]["nodes"].as_array_mut().unwrap();
    nodes.insert(
        1,
        json!({
            "id": "node-code",
            "type": "code",
            "alias": "Protocol Context",
            "description": "",
            "containerId": null,
            "position": { "x": 180, "y": 0 },
            "configVersion": 1,
            "config": {},
            "bindings": {},
            "outputs": [{
                "key": "protocol_context",
                "title": "Protocol Context",
                "valueType": "protocol_context"
            }]
        }),
    );
    nodes[2]["config"]["protocol_context"] = json!({
        "kind": "selector",
        "value": ["node-code", "result", "protocol_context"]
    });
    document["graph"]["edges"] = json!([
        {
            "id": "edge-start-code",
            "source": "node-start",
            "target": "node-code",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        },
        {
            "id": "edge-code-llm",
            "source": "node-code",
            "target": "node-llm",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-code"].outputs[0].selector,
        vec!["result".to_string(), "protocol_context".to_string()]
    );
    assert_eq!(
        plan.nodes["node-llm"].protocol_context_reference().unwrap(),
        Some(VariableReference::selector(vec![
            "node-code".to_string(),
            "result".to_string(),
            "protocol_context".to_string(),
        ]))
    );
    assert!(!plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm" && issue.code == CompileIssueCode::IncompatibleLlmContextSchema
    }));

    document["graph"]["nodes"][1]["outputs"][0]["valueType"] = json!("json");
    let plan = FlowCompiler::compile(flow_id, "draft-2", &document, &compile_context()).unwrap();
    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm" && issue.code == CompileIssueCode::IncompatibleLlmContextSchema
    }));
}

#[test]
fn compile_reports_invalid_llm_context_selector() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["context_policy"] = json!({
        "integration_context": "enabled",
        "context_selector": ["node-start", "missing_history"]
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("document should compile with context selector issue");

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm" && issue.code == CompileIssueCode::InvalidLlmContextSelector
    }));
}

#[test]
fn compile_reports_incompatible_llm_context_schema() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][0]["config"]["input_fields"] = json!([{
        "key": "context",
        "label": "Context",
        "inputType": "json",
        "valueType": "array",
        "required": false
    }]);
    document["graph"]["nodes"][1]["config"]["context_policy"] = json!({
        "integration_context": "enabled",
        "context_selector": ["node-start", "context"]
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("document should compile with context schema issue");

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm" && issue.code == CompileIssueCode::IncompatibleLlmContextSchema
    }));
}

#[test]
fn compiled_llm_routing_deserializes_missing_context_policy_with_default() {
    let routing: CompiledLlmRouting = serde_json::from_value(json!({
        "routing_mode": "fixed_model",
        "fixed_model_target": {
            "provider_instance_id": "provider-selected",
            "provider_code": "fixture_provider",
            "protocol": "openai_compatible",
            "upstream_model_id": "gpt-5.4-mini"
        },
        "queue_template_id": null,
        "queue_targets": [],
        "stream_policy": {}
    }))
    .unwrap();

    assert_eq!(
        routing.context_policy,
        json!({ "integration_context": "enabled" })
    );
}

#[test]
fn compile_prompt_messages_extracts_selector_dependencies_from_message_content() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["bindings"] = json!({
        "prompt_messages": {
            "kind": "prompt_messages",
            "value": [
                {
                    "id": "system-1",
                    "role": "system",
                    "content": {
                        "kind": "templated_text",
                        "value": "You are helpful."
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
            ]
        }
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-llm"].bindings["prompt_messages"].selector_paths,
        vec![vec!["node-start".to_string(), "query".to_string()]]
    );
}

#[test]
fn compile_named_bindings_extracts_selector_dependencies_from_templated_content() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1] = json!({
        "id": "node-code",
        "type": "code",
        "alias": "Code",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {
            "language": "javascript",
            "source": "function main({arg1}) { return { result: arg1 }; }",
            "entrypoint": "main"
        },
        "bindings": {
            "named_bindings": {
                "kind": "named_bindings",
                "value": [
                    {
                        "name": "arg1",
                        "content": {
                            "kind": "templated_text",
                            "value": "Question: {{ node-start.query }}"
                        }
                    }
                ]
            }
        },
        "outputs": [{ "key": "result", "title": "result", "valueType": "string" }]
    });
    document["graph"]["edges"][0]["target"] = json!("node-code");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-code"].bindings["named_bindings"].selector_paths,
        vec![vec!["node-start".to_string(), "query".to_string()]]
    );
}

#[test]
fn compile_named_bindings_ignores_constant_dependencies() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1] = json!({
        "id": "node-code",
        "type": "code",
        "alias": "Code",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {
            "language": "javascript",
            "source": "function main({limit}) { return { result: limit }; }",
            "entrypoint": "main"
        },
        "bindings": {
            "named_bindings": {
                "kind": "named_bindings",
                "value": [
                    {
                        "name": "limit",
                        "valueType": "number",
                        "value": {
                            "kind": "constant",
                            "value": 10
                        }
                    }
                ]
            }
        },
        "outputs": [{ "key": "result", "title": "result", "valueType": "number" }]
    });
    document["graph"]["edges"][0]["target"] = json!("node-code");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.nodes["node-code"].bindings["named_bindings"]
        .selector_paths
        .is_empty());
}

#[test]
fn compile_templated_text_extracts_nested_selector_dependencies() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["bindings"] = json!({
        "prompt_messages": {
            "kind": "prompt_messages",
            "value": [
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "History: {{ node-code.result.chat_history }}"
                    }
                }
            ]
        }
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-llm"].bindings["prompt_messages"].selector_paths,
        vec![vec![
            "node-code".to_string(),
            "result".to_string(),
            "chat_history".to_string()
        ]]
    );
}

#[test]
fn compile_data_model_query_extracts_selector_dependencies() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1] = json!({
        "id": "node-data-model",
        "type": "data_model_list",
        "alias": "Orders",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": { "data_model_code": "orders" },
        "bindings": {
            "query": {
                "kind": "data_model_query",
                "value": {
                    "filters": [
                        {
                            "field_code": "status",
                            "operator": "eq",
                            "value": { "kind": "selector", "selector": ["node-start", "query"] }
                        }
                    ],
                    "sorts": [],
                    "expand_relations": [],
                    "page": { "kind": "constant", "value": 1 },
                    "page_size": { "kind": "selector", "selector": ["node-start", "page_size"] }
                }
            }
        },
        "outputs": [
            { "key": "records", "title": "记录列表", "valueType": "array" },
            { "key": "total", "title": "记录总数", "valueType": "number" }
        ]
    });
    document["graph"]["edges"][0]["target"] = json!("node-data-model");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-data-model"].bindings["query"].selector_paths,
        vec![
            vec!["node-start".to_string(), "query".to_string()],
            vec!["node-start".to_string(), "page_size".to_string()]
        ]
    );
}

#[test]
fn ac_006_compile_accepts_sql_node_and_preserves_templated_sql_binding() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    let sql = "\n-- keep whitespace\nselect ';'::text;\n";
    document["graph"]["nodes"][1] = json!({
        "id": "node-sql",
        "type": "sql",
        "alias": "SQL",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {
            "data_source_instance_id": "main"
        },
        "bindings": {
            "sql": { "kind": "templated_text", "value": sql }
        },
        "outputs": [
            { "key": "results", "title": "Results", "valueType": "array" }
        ]
    });
    document["graph"]["edges"][0]["target"] = json!("node-sql");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.compile_issues.is_empty());
    assert_eq!(plan.nodes["node-sql"].bindings["sql"].raw_value, sql);
    ensure_plan_execution_contract(&plan).unwrap();
}

#[test]
fn compile_rejects_retired_sql_config_even_when_current_binding_is_present() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1] = json!({
        "id": "node-sql",
        "type": "sql",
        "alias": "SQL",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {
            "data_source_instance_id": "main",
            "sql": "select {{node-start.query}}"
        },
        "bindings": {},
        "outputs": [
            { "key": "results", "title": "Results", "valueType": "array" }
        ]
    });
    document["graph"]["edges"][0]["target"] = json!("node-sql");

    for bindings in [
        json!({}),
        json!({"sql":{"kind":"templated_text","value":"select 1"}}),
    ] {
        document["graph"]["nodes"][1]["bindings"] = bindings;
        let error =
            FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap_err();
        assert!(error.to_string().contains("config.sql is unsupported"));
    }
}

#[test]
fn ac_003_compile_rejects_invalid_sql_data_source_binding() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["type"] = json!("sql");
    document["graph"]["nodes"][1]["config"] = json!({
        "data_source_instance_id": "postgres-by-name"
    });
    document["graph"]["nodes"][1]["bindings"] = json!({
        "sql": { "kind": "templated_text", "value": "select 1" }
    });
    document["graph"]["nodes"][1]["outputs"] = json!([
        { "key": "results", "title": "Results", "valueType": "array" }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan
        .compile_issues
        .iter()
        .any(|issue| { issue.code == CompileIssueCode::InvalidDataSourceInstance }));
}

#[test]
fn compile_data_model_filters_inactive_bindings_by_node_type() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1] = json!({
        "id": "node-data-model",
        "type": "data_model_create",
        "alias": "Orders",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": { "data_model_code": "orders" },
        "bindings": {
            "query": {
                "kind": "data_model_query",
                "value": {
                    "filters": [
                        {
                            "field_code": "status",
                            "operator": "eq",
                            "value": { "kind": "selector", "selector": ["missing-node", "answer"] }
                        }
                    ],
                    "sorts": [],
                    "expand_relations": [],
                    "page": { "kind": "constant", "value": 1 },
                    "page_size": { "kind": "constant", "value": 20 }
                }
            },
            "payload": {
                "kind": "named_bindings",
                "value": [{ "name": "title", "selector": ["node-start", "query"] }]
            }
        },
        "outputs": [{ "key": "record", "title": "记录", "valueType": "json" }]
    });
    document["graph"]["edges"][0]["target"] = json!("node-data-model");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(!plan.nodes["node-data-model"].bindings.contains_key("query"));
    assert!(plan.nodes["node-data-model"]
        .bindings
        .contains_key("payload"));
}

#[test]
fn compile_data_model_create_node_filters_inactive_bindings_by_type() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1] = json!({
        "id": "node-data-model-create",
        "type": "data_model_create",
        "alias": "Create Order",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": { "data_model_code": "orders" },
        "bindings": {
            "query": {
                "kind": "data_model_query",
                "value": {
                    "filters": [
                        {
                            "field_code": "status",
                            "operator": "eq",
                            "value": { "kind": "selector", "selector": ["missing-node", "answer"] }
                        }
                    ],
                    "sorts": [],
                    "expand_relations": [],
                    "page": { "kind": "constant", "value": 1 },
                    "page_size": { "kind": "constant", "value": 20 }
                }
            },
            "payload": {
                "kind": "named_bindings",
                "value": [{ "name": "title", "selector": ["node-start", "query"] }]
            }
        },
        "outputs": [{ "key": "record", "title": "记录", "valueType": "json" }]
    });
    document["graph"]["edges"][0]["target"] = json!("node-data-model-create");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(!plan.nodes["node-data-model-create"]
        .bindings
        .contains_key("query"));
    assert!(plan.nodes["node-data-model-create"]
        .bindings
        .contains_key("payload"));
}
