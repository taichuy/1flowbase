use super::*;
use orchestration_runtime::node_error_policy::ERROR_BRANCH_SOURCE_HANDLE;

fn branch_document(flow_id: Uuid) -> Value {
    let if_condition = json!({
        "operator": "or",
        "conditions": [
            {
                "kind": "rule",
                "left": ["node-start", "query"],
                "comparator": "contains",
                "right": { "kind": "constant", "value": "vip" }
            },
            {
                "operator": "and",
                "conditions": [{
                    "kind": "rule",
                    "left": ["node-start", "tier"],
                    "comparator": "equals",
                    "right": { "kind": "selector", "selector": ["node-start", "expected_tier"] }
                }]
            }
        ]
    });

    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Branches", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-if",
                    "type": "if_else",
                    "alias": "If / Else",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "branches": {
                            "kind": "if_else_branches",
                            "value": {
                                "branches": [
                                    {
                                        "id": "if",
                                        "kind": "if",
                                        "title": "If",
                                        "sourceHandle": "if",
                                        "condition": if_condition
                                    },
                                    {
                                        "id": "else",
                                        "kind": "else",
                                        "title": "Else",
                                        "sourceHandle": "else"
                                    }
                                ]
                            }
                        }
                    },
                    "outputs": []
                },
                {
                    "id": "node-answer-if",
                    "type": "answer",
                    "alias": "Answer If",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "templated_text", "value": "if" }
                    },
                    "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
                },
                {
                    "id": "node-answer-else",
                    "type": "answer",
                    "alias": "Answer Else",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 160 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "templated_text", "value": "else" }
                    },
                    "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-if",
                    "source": "node-start",
                    "target": "node-if",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-if-answer",
                    "source": "node-if",
                    "target": "node-answer-if",
                    "sourceHandle": "if",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-else-answer",
                    "source": "node-if",
                    "target": "node-answer-else",
                    "sourceHandle": "else",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

#[test]
fn compile_if_else_branches_preserves_source_handles_and_condition_selectors() {
    let flow_id = Uuid::nil();
    let document = branch_document(flow_id);

    let plan = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect("if_else branch document should compile");

    assert_eq!(
        plan.edges
            .iter()
            .map(|edge| (edge.edge_id.as_str(), edge.source_handle.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("edge-start-if", None),
            ("edge-if-answer", Some("if")),
            ("edge-else-answer", Some("else")),
        ]
    );
    assert_eq!(
        plan.nodes["node-if"].bindings["branches"].selector_paths,
        vec![
            vec!["node-start".to_string(), "query".to_string()],
            vec!["node-start".to_string(), "tier".to_string()],
            vec!["node-start".to_string(), "expected_tier".to_string()],
        ]
    );
}

#[test]
fn compile_if_else_rejects_unknown_source_handles() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["edges"][1]["sourceHandle"] = json!("stale-branch");

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("stale if_else source handle should fail compile");

    assert!(error
        .to_string()
        .contains("edge edge-if-answer references unknown if_else sourceHandle stale-branch"));
}

#[test]
fn compile_if_else_rejects_missing_else_branch() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"] =
        json!([document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"][0]]);
    document["graph"]["edges"] =
        json!([document["graph"]["edges"][0], document["graph"]["edges"][1],]);

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("if_else branch document without else fallback should fail compile");

    assert!(error
        .to_string()
        .contains("if_else node node-if must include an else branch"));
}

#[test]
fn compile_if_else_rejects_duplicate_source_handles() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"][1]["sourceHandle"] =
        json!("if");
    document["graph"]["edges"][2]["sourceHandle"] = json!("if");

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("if_else branch document with duplicate source handles should fail compile");

    assert!(error
        .to_string()
        .contains("if_else node node-if duplicate branch sourceHandle if"));
}

#[test]
fn compile_if_else_rejects_missing_branch_condition() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"][0]
        .as_object_mut()
        .expect("branch should be an object")
        .remove("condition");

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("if_else branch without condition should fail compile");

    assert!(error
        .to_string()
        .contains("if_else node node-if branch if must include a complete condition"));
}

#[test]
fn compile_if_else_rejects_empty_branch_condition() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"][0]["condition"] = json!({
        "operator": "and",
        "conditions": []
    });

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("if_else branch with empty condition should fail compile");

    assert!(error
        .to_string()
        .contains("if_else node node-if branch if must include a complete condition"));
}

#[test]
fn compile_if_else_rejects_invalid_branch_contract_without_outgoing_edges() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["edges"] = json!([document["graph"]["edges"][0]]);
    document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"][0]["condition"] = json!({
        "operator": "and",
        "conditions": []
    });

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("reachable if_else node with invalid branches should fail compile");

    assert!(error
        .to_string()
        .contains("if_else node node-if branch if must include a complete condition"));
}

#[test]
fn compile_allows_error_branch_source_handle_for_nodes_with_error_branch_policy() {
    let flow_id = Uuid::nil();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"][1]["config"]["error_policy"] = json!("error_branch");
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-answer",
            "type": "answer",
            "alias": "Answer",
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {},
            "bindings": {
                "answer_template": { "kind": "templated_text", "value": "handled" }
            },
            "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-llm-error-answer",
            "source": "node-llm",
            "target": "node-answer",
            "sourceHandle": ERROR_BRANCH_SOURCE_HANDLE,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }));

    let plan = FlowCompiler::compile(flow_id, "draft-error-branch", &document, &compile_context())
        .expect("error branch source handle should compile when policy is enabled");

    assert_eq!(
        plan.edges
            .iter()
            .find(|edge| edge.edge_id == "edge-llm-error-answer")
            .and_then(|edge| edge.source_handle.as_deref()),
        Some(ERROR_BRANCH_SOURCE_HANDLE)
    );
}

#[test]
fn compile_rejects_error_branch_source_handle_without_error_branch_policy() {
    let flow_id = Uuid::nil();
    let mut document = sample_document(flow_id);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array")
        .push(json!({
            "id": "node-answer",
            "type": "answer",
            "alias": "Answer",
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {},
            "bindings": {
                "answer_template": { "kind": "templated_text", "value": "handled" }
            },
            "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
        }));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array")
        .push(json!({
            "id": "edge-llm-error-answer",
            "source": "node-llm",
            "target": "node-answer",
            "sourceHandle": ERROR_BRANCH_SOURCE_HANDLE,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }));

    let error = FlowCompiler::compile(flow_id, "draft-error-branch", &document, &compile_context())
        .expect_err("error branch source handle should require error_branch policy");

    assert!(error
        .to_string()
        .contains("edge edge-llm-error-answer uses error sourceHandle on node node-llm without error_branch policy"));
}

#[test]
fn compile_if_else_rejects_multiple_else_branches() {
    let flow_id = Uuid::nil();
    let mut document = branch_document(flow_id);
    document["graph"]["nodes"][1]["bindings"]["branches"]["value"]["branches"]
        .as_array_mut()
        .expect("branches should be an array")
        .push(json!({
            "id": "else-2",
            "kind": "else",
            "title": "Else 2",
            "sourceHandle": "else-2"
        }));

    let error = FlowCompiler::compile(flow_id, "draft-branches", &document, &compile_context())
        .expect_err("if_else branch document with multiple else branches should fail compile");

    assert!(error
        .to_string()
        .contains("if_else node node-if must include only one else branch"));
}

#[test]
fn compile_topology_preserves_document_rank_and_duplicate_edge_queue_order() {
    let mut document = branch_document(Uuid::nil());
    let edges = document["graph"]["edges"].as_array_mut().unwrap();
    edges.swap(1, 2);
    let mut duplicate = edges[1].clone();
    duplicate["id"] = json!("duplicate-else");
    edges.push(duplicate);

    let plan = FlowCompiler::compile(Uuid::nil(), "stable", &document, &compile_context()).unwrap();
    assert_eq!(
        plan.nodes["node-if"].downstream_node_ids,
        ["node-answer-if", "node-answer-else"]
    );
    assert_eq!(
        plan.nodes["node-answer-else"].dependency_node_ids,
        ["node-if"]
    );
    assert_eq!(
        plan.topological_order,
        [
            "node-start",
            "node-if",
            "node-answer-if",
            "node-answer-else"
        ]
    );
    assert_eq!(
        plan.edges
            .iter()
            .map(|edge| edge.edge_id.as_str())
            .collect::<Vec<_>>(),
        [
            "edge-start-if",
            "edge-else-answer",
            "edge-if-answer",
            "duplicate-else"
        ]
    );
}

fn indexed_visible_tool_document() -> Value {
    let mut document = sample_document(Uuid::nil());
    let mut nested = document["graph"]["nodes"][1].clone();
    nested["id"] = json!("root-a");
    nested["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    nested["config"]["visible_internal_llm_tools"] = json!([{
        "type": "visible_internal_llm_tool", "connector_id": "nested"
    }]);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([{
        "type": "visible_internal_llm_tool", "connector_id": "panel", "tool_mode": "fusion",
        "internal_llm_node_policy": "allowed"
    }]);
    let mut after_result = document["graph"]["nodes"][1].clone();
    after_result["id"] = json!("after-result");
    after_result["config"]["visible_internal_llm_tools_enabled"] = json!(false);
    let nodes = document["graph"]["nodes"].as_array_mut().unwrap();
    nodes.push(nested);
    nodes.push(
        json!({"id":"root-b", "type":"template_transform", "alias":"Root B",
        "containerId":null, "config":{"error_policy":"error_branch"},
        "bindings":{}, "outputs":[]}),
    );
    for id in ["shared-result", "excluded-result"] {
        nodes.push(
            json!({"id":id, "type":"tool_result", "alias":id, "containerId":null,
            "config":{}, "bindings":{"result_template":{"kind":"templated_text","value":"ok"}},
            "outputs":[{"key":"result","title":"Result","valueType":"string"}]}),
        );
    }
    nodes.push(after_result);
    let edges = document["graph"]["edges"].as_array_mut().unwrap();
    for (index, target) in ["root-b", "root-a", "root-b"].into_iter().enumerate() {
        edges.push(
            json!({"id":format!("panel-{index}"),"source":"node-llm","target":target,
            "sourceHandle":"visible_internal_llm_tool:panel"}),
        );
    }
    for (index, (source, target, handle)) in [
        ("root-a", "shared-result", None),
        ("root-b", "shared-result", None),
        (
            "root-b",
            "excluded-result",
            Some(ERROR_BRANCH_SOURCE_HANDLE),
        ),
        (
            "root-a",
            "excluded-result",
            Some("visible_internal_llm_tool:nested"),
        ),
        ("shared-result", "after-result", None),
        ("after-result", "excluded-result", None),
    ]
    .into_iter()
    .enumerate()
    {
        edges.push(
            json!({"id":format!("branch-{index}"),"source":source,"target":target,
            "sourceHandle":handle}),
        );
    }
    document
}

#[test]
fn compile_visible_tool_index_preserves_fusion_targets_and_branch_boundaries() {
    let document = indexed_visible_tool_document();
    let plan =
        FlowCompiler::compile(Uuid::nil(), "indexed", &document, &compile_context()).unwrap();
    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    let tool = &plan.nodes["node-llm"].config["visible_internal_llm_tools"][0];
    assert_eq!(tool["target_node_id"], json!("root-b"));
    assert_eq!(
        tool["target_node_ids"],
        json!(["root-b", "root-a", "root-b"])
    );
    assert_eq!(
        plan.nodes["shared-result"].dependency_node_ids,
        ["root-a", "root-b"]
    );
}

#[test]
fn compile_visible_tool_index_preserves_duplicate_agent_and_complete_branch_issues() {
    for (mode, messages) in [
        ("agent", vec![
            "node node-llm visible_internal_llm_tool connector panel has multiple graph edges",
        ]),
        ("fusion", vec![
            "node node-llm visible_internal_llm_tool connector panel has multiple reachable tool_result nodes",
            "node node-llm visible_internal_llm_tool connector panel reaches LLM node root-a without internal_llm_node_policy allowed",
        ]),
    ] {
        let mut document = indexed_visible_tool_document();
        let tool = &mut document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"][0];
        tool["tool_mode"] = json!(mode);
        tool["internal_llm_node_policy"] = json!("forbidden");
        // Ordinary instead of error edge: two results and root-a's LLM become reachable.
        let edges = document["graph"]["edges"].as_array_mut().unwrap();
        edges.iter_mut().find(|edge| edge["id"] == "branch-2").unwrap()["sourceHandle"] = Value::Null;
        let plan = FlowCompiler::compile(Uuid::nil(), mode, &document, &compile_context()).unwrap();
        assert_eq!(plan.compile_issues.len(), messages.len());
        for (issue, message) in plan.compile_issues.iter().zip(messages) {
            assert_eq!(issue.node_id, "node-llm");
            assert_eq!(issue.code, CompileIssueCode::InvalidVisibleInternalLlmTool);
            assert_eq!(issue.field_path, None);
            assert_eq!(issue.message, message);
        }
    }
}
