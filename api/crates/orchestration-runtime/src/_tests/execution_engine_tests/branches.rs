use super::*;
use crate::compiled_plan::CompiledEdge;

fn answer_node(node_id: &str, text: &str) -> CompiledNode {
    CompiledNode {
        node_id: node_id.to_string(),
        node_type: "answer".to_string(),
        alias: node_id.to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-if".to_string()],
        downstream_node_ids: vec![],
        bindings: BTreeMap::from([(
            "answer_template".to_string(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "templated_text".to_string(),
                selector_paths: Vec::new(),
                raw_value: json!(text),
            },
        )]),
        outputs: vec![CompiledOutput {
            key: "answer".to_string(),
            title: "Answer".to_string(),
            value_type: "string".to_string(),
            selector: Vec::new(),
            json_schema: None,
        }],
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

fn branch_plan(include_else_if_edge: bool, regex_pattern: Option<&str>) -> CompiledPlan {
    let regex_pattern = regex_pattern.unwrap_or("^enterprise-");
    let mut nodes = BTreeMap::new();

    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-if".to_string()],
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-if".to_string(),
        CompiledNode {
            node_id: "node-if".to_string(),
            node_type: "if_else".to_string(),
            alias: "If / Else".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec![
                "node-if-answer".to_string(),
                "node-elseif-answer".to_string(),
                "node-else-answer".to_string(),
            ],
            bindings: BTreeMap::from([(
                "branches".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "if_else_branches".to_string(),
                    selector_paths: vec![
                        vec!["node-start".to_string(), "status".to_string()],
                        vec!["node-start".to_string(), "segment".to_string()],
                    ],
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
                                        "left": ["node-start", "status"],
                                        "comparator": "equals",
                                        "right": { "kind": "constant", "value": "vip" }
                                    }]
                                }
                            },
                            {
                                "id": "else-if-1",
                                "kind": "else_if",
                                "title": "Else If 1",
                                "sourceHandle": "else-if-1",
                                "condition": {
                                    "operator": "and",
                                    "conditions": [{
                                        "kind": "rule",
                                        "left": ["node-start", "segment"],
                                        "comparator": "matches_regex",
                                        "right": { "kind": "constant", "value": regex_pattern }
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
    nodes.insert(
        "node-if-answer".to_string(),
        answer_node("node-if-answer", "if"),
    );
    nodes.insert(
        "node-elseif-answer".to_string(),
        answer_node("node-elseif-answer", "else-if"),
    );
    nodes.insert(
        "node-else-answer".to_string(),
        answer_node("node-else-answer", "else"),
    );

    let mut edges = vec![
        CompiledEdge {
            edge_id: "edge-start-if".to_string(),
            source: "node-start".to_string(),
            target: "node-if".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-if-answer".to_string(),
            source: "node-if".to_string(),
            target: "node-if-answer".to_string(),
            source_handle: Some("if".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-else-answer".to_string(),
            source: "node-if".to_string(),
            target: "node-else-answer".to_string(),
            source_handle: Some("else".to_string()),
            target_handle: None,
        },
    ];

    if include_else_if_edge {
        edges.push(CompiledEdge {
            edge_id: "edge-elseif-answer".to_string(),
            source: "node-if".to_string(),
            target: "node-elseif-answer".to_string(),
            source_handle: Some("else-if-1".to_string()),
            target_handle: None,
        });
    }

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-branches".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec![
            "node-start".to_string(),
            "node-if".to_string(),
            "node-if-answer".to_string(),
            "node-elseif-answer".to_string(),
            "node-else-answer".to_string(),
        ],
        edges,
        nodes,
        compile_issues: Vec::new(),
    }
}

async fn run_branch_plan(input_payload: Value, include_else_if_edge: bool) -> Vec<String> {
    start_flow_debug_run(
        &branch_plan(include_else_if_edge, None),
        &input_payload,
        &successful_invoker(),
    )
    .await
    .unwrap()
    .node_traces
    .into_iter()
    .map(|trace| trace.node_id)
    .collect()
}

#[tokio::test]
async fn if_else_runs_first_matching_if_branch() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "vip", "segment": "enterprise-a" } }),
        true,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if", "node-if-answer"]);
}

#[tokio::test]
async fn if_else_runs_else_if_branch_after_if_misses() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "regular", "segment": "enterprise-a" } }),
        true,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if", "node-elseif-answer"]);
}

#[tokio::test]
async fn if_else_falls_back_to_else_branch() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "regular", "segment": "consumer" } }),
        true,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if", "node-else-answer"]);
}

#[tokio::test]
async fn if_else_selected_unconnected_branch_naturally_ends() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "regular", "segment": "enterprise-a" } }),
        false,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if"]);
}

#[tokio::test]
async fn if_else_invalid_regex_does_not_match() {
    let outcome = start_flow_debug_run(
        &branch_plan(true, Some("[")),
        &json!({ "node-start": { "status": "regular", "segment": "enterprise-a" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();
    let traces = outcome
        .node_traces
        .into_iter()
        .map(|trace| trace.node_id)
        .collect::<Vec<_>>();

    assert_eq!(traces, vec!["node-start", "node-if", "node-else-answer"]);
}

#[tokio::test]
async fn if_else_empty_comparator_matches_missing_null_empty_string_and_empty_array() {
    let mut plan = branch_plan(true, None);
    let branch_binding = plan
        .nodes
        .get_mut("node-if")
        .expect("branch plan should include if_else node")
        .bindings
        .get_mut("branches")
        .expect("if_else node should include branches binding");

    branch_binding.raw_value = json!({
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
                        "left": ["node-start", "maybe"],
                        "comparator": "empty"
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
    });

    for payload in [
        json!({ "node-start": {} }),
        json!({ "node-start": { "maybe": null } }),
        json!({ "node-start": { "maybe": "" } }),
        json!({ "node-start": { "maybe": [] } }),
    ] {
        let outcome = start_flow_debug_run(&plan, &payload, &successful_invoker())
            .await
            .unwrap();
        let traces = outcome
            .node_traces
            .into_iter()
            .map(|trace| trace.node_id)
            .collect::<Vec<_>>();

        assert_eq!(traces, vec!["node-start", "node-if", "node-if-answer"]);
    }

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "maybe": "ready" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();
    let traces = outcome
        .node_traces
        .into_iter()
        .map(|trace| trace.node_id)
        .collect::<Vec<_>>();

    assert_eq!(traces, vec!["node-start", "node-if", "node-else-answer"]);
}

#[tokio::test]
async fn if_else_evaluates_nested_groups_and_selector_right_values() {
    let mut plan = branch_plan(true, None);
    let branch_binding = plan
        .nodes
        .get_mut("node-if")
        .expect("branch plan should include if_else node")
        .bindings
        .get_mut("branches")
        .expect("if_else node should include branches binding");

    branch_binding.raw_value = json!({
        "branches": [
            {
                "id": "if",
                "kind": "if",
                "title": "If",
                "sourceHandle": "if",
                "condition": {
                    "operator": "and",
                    "conditions": [
                        {
                            "kind": "rule",
                            "left": ["node-start", "status"],
                            "comparator": "exists"
                        },
                        {
                            "operator": "or",
                            "conditions": [
                                {
                                    "kind": "rule",
                                    "left": ["node-start", "segment"],
                                    "comparator": "equals",
                                    "right": {
                                        "kind": "selector",
                                        "selector": ["node-start", "expected_segment"]
                                    }
                                },
                                {
                                    "kind": "rule",
                                    "left": ["node-start", "score"],
                                    "comparator": "greater_than",
                                    "right": { "kind": "constant", "value": 90 }
                                }
                            ]
                        }
                    ]
                }
            },
            {
                "id": "else",
                "kind": "else",
                "title": "Else",
                "sourceHandle": "else"
            }
        ]
    });

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "status": "ready",
                "segment": "enterprise-a",
                "expected_segment": "enterprise-a",
                "score": 10
            }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();
    let traces = outcome
        .node_traces
        .into_iter()
        .map(|trace| trace.node_id)
        .collect::<Vec<_>>();

    assert_eq!(traces, vec!["node-start", "node-if", "node-if-answer"]);
}
