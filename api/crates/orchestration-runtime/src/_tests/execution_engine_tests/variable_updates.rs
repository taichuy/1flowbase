use super::*;
use crate::execution_engine::execute_variable_assignment_node;
use serde_json::Map;

#[tokio::test]
async fn variable_assigner_updates_conversation_for_current_run_only() {
    let mut plan = base_plan();

    plan.topological_order = vec![
        "node-start".to_string(),
        "node-variable-assigner".to_string(),
        "node-answer".to_string(),
    ];
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-variable-assigner".to_string(),
            source: "node-start".to_string(),
            target: "node-variable-assigner".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-variable-assigner-answer".to_string(),
            source: "node-variable-assigner".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];

    let start = plan
        .nodes
        .get_mut("node-start")
        .expect("start node should exist");
    start.downstream_node_ids = vec!["node-variable-assigner".to_string()];

    plan.nodes.insert(
        "node-variable-assigner".to_string(),
        CompiledNode {
            node_id: "node-variable-assigner".to_string(),
            node_type: "variable_assigner".to_string(),
            alias: "变量赋值".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-answer".to_string()],
            bindings: BTreeMap::from([(
                "operations".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "state_write".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                    raw_value: json!([
                        {
                            "path": ["conversation", "ApiBaseUrl"],
                            "operator": "set",
                            "value": {
                                "kind": "templated_text",
                                "value": "https://{{node-start.query}}/v1"
                            }
                        }
                    ]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "ApiBaseUrl".to_string(),
                title: "conversation.ApiBaseUrl".to_string(),
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

    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.dependency_node_ids = vec!["node-variable-assigner".to_string()];
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "templated_text".to_string(),
            selector_paths: vec![vec!["conversation".to_string(), "ApiBaseUrl".to_string()]],
            raw_value: json!("{{conversation.ApiBaseUrl}}"),
        },
    )]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "env": {
                "ApiBaseUrl": "https://old.example.com"
            },
            "conversation": {
                "ApiBaseUrl": "https://old-conversation.example.com"
            },
            "node-start": {
                "query": "new.example.com"
            }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["env"]["ApiBaseUrl"],
        json!("https://old.example.com")
    );
    assert_eq!(
        outcome.variable_pool["conversation"]["ApiBaseUrl"],
        json!("https://new.example.com/v1")
    );
    assert_eq!(
        outcome.variable_pool["node-variable-assigner"]["ApiBaseUrl"],
        json!("https://new.example.com/v1")
    );
    assert_eq!(
        outcome
            .node_traces
            .iter()
            .find(|trace| trace.node_id == "node-variable-assigner")
            .expect("variable assigner trace should exist")
            .output_payload,
        json!({ "ApiBaseUrl": "https://new.example.com/v1" })
    );
    assert_eq!(
        outcome
            .node_traces
            .iter()
            .find(|trace| trace.node_id == "node-answer")
            .expect("answer trace should exist")
            .output_payload["answer"],
        json!("https://new.example.com/v1")
    );
}

#[tokio::test]
async fn variable_assigner_rejects_readonly_env_targets() {
    let node = CompiledNode {
        node_id: "node-variable-assigner".to_string(),
        node_type: "variable_assigner".to_string(),
        alias: "变量赋值".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };
    let resolved_inputs = Map::from_iter([(
        "operations".to_string(),
        json!([
            {
                "path": ["env", "ApiBaseUrl"],
                "operator": "set",
                "value": {
                    "kind": "constant",
                    "value": "https://new.example.com"
                }
            }
        ]),
    )]);
    let mut variable_pool = Map::from_iter([(
        "env".to_string(),
        json!({ "ApiBaseUrl": "https://old.example.com" }),
    )]);

    let error = execute_variable_assignment_node(&node, &resolved_inputs, &mut variable_pool)
        .expect_err("env is readonly for variable assigner");

    assert!(
        error
            .to_string()
            .contains("variable assigner only supports setting conversation variables"),
        "unexpected error: {error}"
    );
    assert_eq!(
        variable_pool["env"]["ApiBaseUrl"],
        json!("https://old.example.com")
    );
}
