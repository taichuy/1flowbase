use super::*;

#[test]
fn execution_contract_rejects_unsupported_node_type_missing_from_legacy_issues() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["type"] = json!("knowledge_retrieval");

    let mut plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("unsupported node should remain representable as a compile issue");
    assert!(plan
        .compile_issues
        .iter()
        .any(|issue| issue.code == CompileIssueCode::UnsupportedNodeType));
    plan.compile_issues.clear();

    let error = ensure_plan_execution_contract(&plan)
        .expect_err("legacy plan must revalidate executable node types when loaded");

    assert!(error.to_string().contains("knowledge_retrieval"));
}

#[test]
fn compile_rejects_answer_presentation_reversing_real_dependency_order() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    add_second_llm_and_answer(
        &mut document,
        "{{ node-llm-2.text }}\n----\n{{ node-llm.text }}",
        true,
    );

    let compiled = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("document should compile with answer presentation issue");

    assert!(compiled.compile_issues.iter().any(|issue| {
        issue.node_id == "node-answer"
            && issue.code == CompileIssueCode::InvalidAnswerPresentationOrder
    }));
}

#[test]
fn compile_rejects_duplicate_answer_presentation_reference() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    add_second_llm_and_answer(
        &mut document,
        "{{ node-llm.text }}\n----\n{{ node-llm.text }}",
        true,
    );

    let compiled = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("document should compile with answer presentation issue");

    assert!(compiled.compile_issues.iter().any(|issue| {
        issue.node_id == "node-answer"
            && issue.code == CompileIssueCode::DuplicateAnswerPresentationReference
    }));
}

#[test]
fn compile_allows_parallel_answer_references_in_template_order() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    add_second_llm_and_answer(
        &mut document,
        "{{ node-llm-2.text }}\n----\n{{ node-llm.text }}",
        false,
    );

    let compiled = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("parallel answer references should compile");

    assert!(
        compiled.compile_issues.iter().all(|issue| !matches!(
            issue.code,
            CompileIssueCode::InvalidAnswerPresentationOrder
                | CompileIssueCode::DuplicateAnswerPresentationReference
        )),
        "parallel presentation order should not create answer issues: {:?}",
        compiled.compile_issues
    );
}

#[test]
fn compile_outputs_preserve_declared_selector_paths() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["outputs"] = json!([
        {
            "key": "token_usage",
            "title": "Token Usage",
            "valueType": "number",
            "selector": ["usage", "total_tokens"]
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(plan.nodes["node-llm"].outputs[0].key, "token_usage");
    assert_eq!(
        plan.nodes["node-llm"].outputs[0].selector,
        vec!["usage".to_string(), "total_tokens".to_string()]
    );
}

#[test]
fn compile_outputs_preserve_declared_json_schema() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["outputs"] = json!([
        {
            "key": "chat_history",
            "title": "Chat History",
            "valueType": "array",
            "jsonSchema": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["role", "content"],
                    "properties": {
                        "role": { "type": "string" },
                        "content": { "type": "string" }
                    }
                }
            }
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-llm"].outputs[0].json_schema,
        Some(json!({
            "type": "array",
            "items": {
                "type": "object",
                "required": ["role", "content"],
                "properties": {
                    "role": { "type": "string" },
                    "content": { "type": "string" }
                }
            }
        }))
    );
}

#[test]
fn compile_state_write_extracts_templated_value_dependencies() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);

    document["graph"]["nodes"][1] = json!({
        "id": "node-env-update",
        "type": "variable_assigner",
        "alias": "变量赋值",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {
            "operations": {
                "kind": "state_write",
                "value": [
                    {
                        "path": ["conversation", "ApiBaseUrl"],
                        "operator": "set",
                        "value": {
                            "kind": "templated_text",
                            "value": "https://{{node-start.query}}/v1"
                        }
                    }
                ]
            }
        },
        "outputs": [
            {
                "key": "ApiBaseUrl",
                "title": "conversation.ApiBaseUrl",
                "valueType": "string"
            }
        ]
    });
    document["graph"]["edges"][0]["target"] = json!("node-env-update");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-env-update"].bindings["operations"].selector_paths,
        vec![vec!["node-start".to_string(), "query".to_string()]]
    );
    assert_eq!(plan.nodes["node-env-update"].outputs[0].key, "ApiBaseUrl");
    assert_eq!(
        plan.nodes["node-env-update"].outputs[0].value_type,
        "string"
    );
}

#[test]
fn compile_reports_selector_source_outside_connected_upstream_graph() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-disconnected",
            "type": "template_transform",
            "alias": "Disconnected",
            "description": "",
            "containerId": null,
            "position": { "x": 0, "y": 240 },
            "configVersion": 1,
            "config": {},
            "bindings": {
                "template": { "kind": "templated_text", "value": "hidden" }
            },
            "outputs": [{ "key": "text", "title": "Text", "valueType": "string" }]
        }));
    document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]["value"] =
        json!("{{node-disconnected.text}}");

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("invalid selector scope should be reported as a compile issue");

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm"
            && issue.message.contains("node-disconnected")
            && issue.message.contains("not reachable")
    }));
}

#[test]
fn compile_reports_selector_field_missing_from_upstream_output_contract() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .insert(
            1,
            json!({
                "id": "node-transform",
                "type": "template_transform",
                "alias": "Transform",
                "description": "",
                "containerId": null,
                "position": { "x": 240, "y": 0 },
                "configVersion": 1,
                "config": {},
                "bindings": {
                    "template": { "kind": "templated_text", "value": "{{node-start.query}}" }
                },
                "outputs": [{ "key": "text", "title": "Text", "valueType": "string" }]
            }),
        );
    document["graph"]["nodes"][2]["bindings"]["prompt_messages"]["value"][0]["content"]["value"] =
        json!("{{node-transform.missing}}");
    document["graph"]["edges"] = json!([
        {
            "id": "edge-start-transform",
            "source": "node-start",
            "target": "node-transform",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        },
        {
            "id": "edge-transform-llm",
            "source": "node-transform",
            "target": "node-llm",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }
    ]);

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("invalid selector output should be reported as a compile issue");

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-llm"
            && issue.message.contains("node-transform.missing")
            && issue.message.contains("output contract")
    }));
}

#[test]
fn compile_rejects_unsupported_flow_schema_version() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["schemaVersion"] = json!("1flowbase.flow/v1");

    let error =
        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap_err();

    assert!(error
        .to_string()
        .contains("unsupported flow schemaVersion: 1flowbase.flow/v1"));
}

#[test]
fn compile_rejects_legacy_start_outputs() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][0]["outputs"] =
        json!([{ "key": "query", "title": "用户输入", "valueType": "string" }]);

    let error =
        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap_err();

    assert!(error
        .to_string()
        .contains("start node node-start outputs must be empty"));
}

#[test]
fn compile_agent_flow_rejects_workflow_start_and_end_nodes() {
    let flow_id = Uuid::now_v7();
    let document = workflow_document(flow_id);

    let error =
        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap_err();

    assert!(error
        .to_string()
        .contains("agent_flow document cannot contain workflow_start node node-workflow-start"));
}

#[test]
fn compile_workflow_rejects_agent_flow_start_and_answer_nodes() {
    let flow_id = Uuid::now_v7();
    let document = sample_document(flow_id);

    let error = FlowCompiler::compile_workflow(flow_id, "draft-1", &document, &compile_context())
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("workflow document cannot contain start node node-start"));
}

#[test]
fn compile_workflow_preserves_start_timeout_and_end_return_fields() {
    let flow_id = Uuid::now_v7();
    let document = workflow_document(flow_id);

    let plan =
        FlowCompiler::compile_workflow(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert_eq!(
        plan.nodes["node-workflow-start"].config["sync_timeout_ms"],
        json!(30000)
    );
    assert_eq!(
        plan.nodes["node-workflow-start"].config["input_fields"],
        json!([
            {
                "key": "customer_id",
                "label": "Customer ID",
                "inputType": "text",
                "valueType": "string",
                "required": true
            }
        ])
    );
    assert_eq!(plan.nodes["node-workflow-end"].outputs[0].key, "ticket_id");
    assert_eq!(
        plan.nodes["node-workflow-end"].outputs[0].value_type,
        "string"
    );
}

#[test]
fn compile_workflow_recognizes_start_input_fields_for_selector_validation() {
    let flow_id = Uuid::now_v7();
    let mut document = workflow_document(flow_id);
    document["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "node-workflow-llm",
            "type": "llm",
            "alias": "Workflow LLM",
            "description": "",
            "containerId": null,
            "position": { "x": 240, "y": 0 },
            "configVersion": 1,
            "config": {
                "model_provider": {
                    "provider_code": "fixture_provider",
                    "model_id": "gpt-5.4-mini"
                },
                "context_policy": {
                    "integration_context": "enabled",
                    "context_selector": ["node-workflow-start", "customer_id"]
                }
            },
            "bindings": {
                "prompt_messages": {
                    "kind": "prompt_messages",
                    "value": [{
                        "id": "workflow-user",
                        "role": "user",
                        "content": {
                            "kind": "templated_text",
                            "value": "Customer: {{ node-workflow-start.customer_id }}"
                        }
                    }]
                }
            },
            "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
        }));
    let edges = document["graph"]["edges"].as_array_mut().unwrap();
    edges.clear();
    edges.push(json!({
        "id": "edge-workflow-start-llm",
        "source": "node-workflow-start",
        "target": "node-workflow-llm",
        "sourceHandle": null,
        "targetHandle": null,
        "containerId": null,
        "points": []
    }));
    edges.push(json!({
        "id": "edge-workflow-llm-end",
        "source": "node-workflow-llm",
        "target": "node-workflow-end",
        "sourceHandle": null,
        "targetHandle": null,
        "containerId": null,
        "points": []
    }));

    let plan =
        FlowCompiler::compile_workflow(flow_id, "draft-1", &document, &compile_context()).unwrap();

    assert!(plan.nodes["node-workflow-llm"].bindings["prompt_messages"]
        .selector_paths
        .contains(&vec![
            "node-workflow-start".to_string(),
            "customer_id".to_string()
        ]));
    assert!(!plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-workflow-llm"
            && issue.code == CompileIssueCode::InvalidLlmContextSelector
    }));
    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-workflow-llm"
            && issue.code == CompileIssueCode::IncompatibleLlmContextSchema
    }));
}

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
fn compile_llm_node_carries_context_policy_into_routing() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["context_policy"] = json!({
        "integration_context": "enabled",
        "context_selector": ["node-start", "history"]
    });

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let routing = plan.nodes["node-llm"]
        .llm_runtime
        .as_ref()
        .and_then(|runtime| runtime.routing.as_ref())
        .expect("llm routing should exist");

    assert_eq!(
        routing.context_policy,
        json!({ "integration_context": "enabled", "context_selector": ["node-start", "history"] })
    );
}

#[test]
fn wp_d1b_compiled_llm_protocol_context_is_one_nullable_variable_reference() {
    let flow_id = Uuid::now_v7();
    let document = sample_document(flow_id);
    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
    let llm = &plan.nodes["node-llm"];

    assert_eq!(llm.config["protocol_context"], Value::Null);
    assert_eq!(llm.protocol_context_reference().unwrap(), None);

    let mut document = document;
    document["graph"]["nodes"][1]["config"]["protocol_context"] = json!({
        "kind": "selector",
        "value": ["sys", "protocol_context"]
    });
    let plan = FlowCompiler::compile(flow_id, "draft-2", &document, &compile_context()).unwrap();
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
        Some(VariableReference::selector(vec![
            "sys".to_string(),
            "protocol_context".to_string(),
        ]))
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
fn wp_d1b_protocol_context_accepts_existing_code_json_output_contract() {
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
                "valueType": "json"
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

    document["graph"]["nodes"][1]["outputs"][0]["valueType"] = json!("string");
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
