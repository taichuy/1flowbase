use super::*;
use orchestration_runtime::compiled_plan::{
    CompiledEdge, StartCompactDispatch, COMPACT_SOURCE_HANDLE_ID,
};

fn application_flow_compact_document(flow_id: Uuid) -> Value {
    let mut document = sample_document(flow_id);
    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample document nodes should be an array");
    nodes.retain(|node| node.get("id").and_then(Value::as_str) == Some("node-start"));
    nodes.push(json!({
        "id": "node-answer",
        "type": "answer",
        "alias": "Answer",
        "description": "",
        "containerId": null,
        "position": { "x": 260, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {
            "answer_template": {
                "kind": "templated_text",
                "value": "{{node-start.query}}"
            }
        },
        "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
    }));
    nodes.push(json!({
        "id": "node-compact-response",
        "type": "compact_response",
        "alias": "Compact Response",
        "description": "",
        "containerId": null,
        "position": { "x": 260, "y": 180 },
        "configVersion": 1,
        "config": {},
        "bindings": {},
        "outputs": []
    }));

    document["graph"]["nodes"][0]["config"] = json!({
        "compact_dispatch": "application_flow"
    });
    document["graph"]["edges"] = json!([
        {
            "id": "edge-start-answer",
            "source": "node-start",
            "target": "node-answer",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        },
        {
            "id": "edge-start-compact-response",
            "source": "node-start",
            "target": "node-compact-response",
            "sourceHandle": COMPACT_SOURCE_HANDLE_ID,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }
    ]);

    document
}

#[test]
fn compile_agent_flow_keeps_compact_dispatch_as_a_reserved_terminal_branch() {
    let flow_id = Uuid::now_v7();
    let document = application_flow_compact_document(flow_id);

    let plan = FlowCompiler::compile(flow_id, "draft-compact", &document, &compile_context())
        .expect("application-flow Compact topology should compile");

    let start = plan
        .nodes
        .get("node-start")
        .expect("compiled Start should exist");
    assert_eq!(
        StartCompactDispatch::from_start_config(&start.config)
            .expect("compiled Start dispatch should be valid"),
        StartCompactDispatch::ApplicationFlow
    );
    assert!(plan.edges.iter().any(|edge| {
        edge.source == "node-start"
            && edge.target == "node-compact-response"
            && edge.source_handle.as_deref() == Some(COMPACT_SOURCE_HANDLE_ID)
    }));
    assert!(plan
        .compile_issues
        .iter()
        .all(|issue| issue.node_id != "node-compact-response"));
}

#[test]
fn compile_agent_flow_treats_missing_compact_dispatch_as_transparent() {
    let flow_id = Uuid::now_v7();
    let mut document = application_flow_compact_document(flow_id);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("fixture nodes should be an array")
        .retain(|node| node.get("id").and_then(Value::as_str) != Some("node-compact-response"));
    document["graph"]["edges"]
        .as_array_mut()
        .expect("fixture edges should be an array")
        .retain(|edge| {
            edge.get("sourceHandle").and_then(Value::as_str) != Some(COMPACT_SOURCE_HANDLE_ID)
        });
    document["graph"]["nodes"][0]["config"] = json!({});

    let plan = FlowCompiler::compile(flow_id, "draft-legacy", &document, &compile_context())
        .expect("legacy Start should preserve transparent dispatch");
    let start = plan
        .nodes
        .get("node-start")
        .expect("compiled Start should exist");

    assert_eq!(
        StartCompactDispatch::from_start_config(&start.config)
            .expect("legacy dispatch should remain valid"),
        StartCompactDispatch::Transparent
    );
}

#[test]
fn compile_agent_flow_rejects_missing_duplicate_and_dangling_compact_edges() {
    let flow_id = Uuid::now_v7();
    let mut transparent = application_flow_compact_document(flow_id);
    transparent["graph"]["nodes"][0]["config"] = json!({});
    let transparent_error = FlowCompiler::compile(
        flow_id,
        "draft-transparent",
        &transparent,
        &compile_context(),
    )
    .expect_err("transparent Start must not retain a Compact edge");
    assert!(transparent_error
        .to_string()
        .contains("without application_flow dispatch"));

    let mut missing = application_flow_compact_document(flow_id);
    missing["graph"]["edges"]
        .as_array_mut()
        .expect("fixture edges should be an array")
        .retain(|edge| {
            edge.get("sourceHandle").and_then(Value::as_str) != Some(COMPACT_SOURCE_HANDLE_ID)
        });
    let missing_error =
        FlowCompiler::compile(flow_id, "draft-missing", &missing, &compile_context())
            .expect_err("application_flow Start must not omit its Compact edge");
    assert!(missing_error
        .to_string()
        .contains("exactly one compact edge"));

    let mut duplicate = application_flow_compact_document(flow_id);
    let duplicate_edge = duplicate["graph"]["edges"][1].clone();
    duplicate["graph"]["edges"]
        .as_array_mut()
        .expect("fixture edges should be an array")
        .push(duplicate_edge);
    let duplicate_error =
        FlowCompiler::compile(flow_id, "draft-duplicate", &duplicate, &compile_context())
            .expect_err("application_flow Start must not keep duplicate Compact edges");
    assert!(duplicate_error
        .to_string()
        .contains("exactly one compact edge"));

    let mut dangling = application_flow_compact_document(flow_id);
    dangling["graph"]["edges"][1]["target"] = json!("node-missing-compact-response");
    let dangling_error =
        FlowCompiler::compile(flow_id, "draft-dangling", &dangling, &compile_context())
            .expect_err("dangling Compact edge must be rejected");
    assert!(dangling_error
        .to_string()
        .contains("references unknown target node"));
}

#[test]
fn compile_agent_flow_rejects_cross_terminal_paths_and_raw_compact_contracts() {
    let flow_id = Uuid::now_v7();
    let mut wrong_target = application_flow_compact_document(flow_id);
    wrong_target["graph"]["edges"][1]["target"] = json!("node-answer");
    let wrong_target_error = FlowCompiler::compile(
        flow_id,
        "draft-wrong-target",
        &wrong_target,
        &compile_context(),
    )
    .expect_err("Compact handle must not select Answer");
    assert!(wrong_target_error
        .to_string()
        .contains("must target a compact_response node"));

    let mut ordinary_to_compact = application_flow_compact_document(flow_id);
    ordinary_to_compact["graph"]["edges"]
        .as_array_mut()
        .expect("fixture edges should be an array")
        .push(json!({
            "id": "edge-answer-compact",
            "source": "node-answer",
            "target": "node-compact-response",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }));
    let ordinary_to_compact_error = FlowCompiler::compile(
        flow_id,
        "draft-cross-terminal",
        &ordinary_to_compact,
        &compile_context(),
    )
    .expect_err("ordinary path must not reach Compact Response");
    assert!(ordinary_to_compact_error
        .to_string()
        .contains("must have exactly one incoming compact edge"));

    let mut compact_outgoing = application_flow_compact_document(flow_id);
    compact_outgoing["graph"]["edges"]
        .as_array_mut()
        .expect("fixture edges should be an array")
        .push(json!({
            "id": "edge-compact-answer",
            "source": "node-compact-response",
            "target": "node-answer",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }));
    let compact_outgoing_error = FlowCompiler::compile(
        flow_id,
        "draft-compact-outgoing",
        &compact_outgoing,
        &compile_context(),
    )
    .expect_err("Compact Response must be terminal");
    assert!(compact_outgoing_error
        .to_string()
        .contains("terminal node node-compact-response"));

    let mut raw_contract = application_flow_compact_document(flow_id);
    raw_contract["graph"]["nodes"][2]["config"] = json!({ "body": { "fake": "v2" } });
    let raw_contract_error = FlowCompiler::compile(
        flow_id,
        "draft-raw-contract",
        &raw_contract,
        &compile_context(),
    )
    .expect_err("Compact Response must not accept an authorable raw body");
    assert!(raw_contract_error
        .to_string()
        .contains("must not define config, bindings, or outputs"));
}

#[test]
fn loaded_plan_revalidates_compact_response_terminality() {
    let flow_id = Uuid::now_v7();
    let document = application_flow_compact_document(flow_id);
    let mut plan = FlowCompiler::compile(flow_id, "draft-loaded", &document, &compile_context())
        .expect("fixture should compile before loaded-plan mutation");
    plan.edges.push(CompiledEdge {
        edge_id: "edge-loaded-compact-answer".to_string(),
        source: "node-compact-response".to_string(),
        target: "node-answer".to_string(),
        source_handle: None,
        target_handle: None,
    });

    let error = ensure_plan_execution_contract(&plan)
        .expect_err("loaded plan must revalidate Compact Response terminality");
    assert!(error
        .to_string()
        .contains("terminal node node-compact-response"));
}

#[test]
fn compile_workflow_rejects_compact_response_node_family() {
    let flow_id = Uuid::now_v7();
    let mut document = workflow_document(flow_id);
    document["graph"]["nodes"]
        .as_array_mut()
        .expect("workflow fixture nodes should be an array")
        .push(json!({
            "id": "node-compact-response",
            "type": "compact_response",
            "alias": "Compact Response",
            "description": "",
            "containerId": null,
            "position": { "x": 480, "y": 0 },
            "configVersion": 1,
            "config": {},
            "bindings": {},
            "outputs": []
        }));

    let error =
        FlowCompiler::compile_workflow(flow_id, "workflow-compact", &document, &compile_context())
            .expect_err("Workflow must not gain Compact Response support in this delivery");
    assert!(error
        .to_string()
        .contains("workflow document cannot contain compact_response"));
}
