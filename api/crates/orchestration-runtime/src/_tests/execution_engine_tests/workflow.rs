use super::*;

fn workflow_plan() -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-workflow-start".to_string(),
        CompiledNode {
            node_id: "node-workflow-start".to_string(),
            node_type: "workflow_start".to_string(),
            alias: "Workflow Start".to_string(),
            container_id: None,
            dependency_node_ids: Vec::new(),
            downstream_node_ids: vec!["node-transform".to_string()],
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({
                "input_fields": [
                    {
                        "key": "customer_id",
                        "label": "Customer ID",
                        "inputType": "text",
                        "valueType": "string",
                        "required": true
                    }
                ],
                "sync_timeout_ms": 30000
            }),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-transform".to_string(),
        CompiledNode {
            node_id: "node-transform".to_string(),
            node_type: "template_transform".to_string(),
            alias: "Template Transform".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-workflow-start".to_string()],
            downstream_node_ids: vec!["node-workflow-end".to_string()],
            bindings: BTreeMap::from([(
                "template".to_string(),
                CompiledBinding {
                    kind: "templated_text".to_string(),
                    raw_value: json!("ticket-{{ node-workflow-start.customer_id }}"),
                    selector_paths: vec![vec![
                        "node-workflow-start".to_string(),
                        "customer_id".to_string(),
                    ]],
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "ticket_id".to_string(),
                title: "Ticket ID".to_string(),
                value_type: "string".to_string(),
                selector: vec!["ticket_id".to_string()],
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-workflow-end".to_string(),
        CompiledNode {
            node_id: "node-workflow-end".to_string(),
            node_type: "workflow_end".to_string(),
            alias: "Workflow End".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-transform".to_string()],
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::from([(
                "ticket_id".to_string(),
                CompiledBinding {
                    kind: "selector".to_string(),
                    raw_value: json!(["node-transform", "ticket_id"]),
                    selector_paths: vec![vec![
                        "node-transform".to_string(),
                        "ticket_id".to_string(),
                    ]],
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "ticket_id".to_string(),
                title: "Ticket ID".to_string(),
                value_type: "string".to_string(),
                selector: vec!["ticket_id".to_string()],
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );

    CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec![
            "node-workflow-start".to_string(),
            "node-transform".to_string(),
            "node-workflow-end".to_string(),
        ],
        edges: vec![
            CompiledEdge {
                edge_id: "edge-start-transform".to_string(),
                source: "node-workflow-start".to_string(),
                target: "node-transform".to_string(),
                source_handle: None,
                target_handle: None,
            },
            CompiledEdge {
                edge_id: "edge-transform-end".to_string(),
                source: "node-transform".to_string(),
                target: "node-workflow-end".to_string(),
                source_handle: None,
                target_handle: None,
            },
        ],
        nodes,
        compile_issues: Vec::new(),
    }
}

#[tokio::test]
async fn workflow_start_and_end_project_input_and_return_fields() {
    let outcome = start_flow_debug_run(
        &workflow_plan(),
        &json!({
            "node-workflow-start": {
                "customer_id": "C-42"
            }
        }),
        &successful_invoker(),
    )
    .await
    .expect("workflow execution should succeed");

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        outcome.variable_pool["node-workflow-start"]["customer_id"],
        json!("C-42")
    );
    assert_eq!(
        outcome.variable_pool["node-workflow-end"],
        json!({ "ticket_id": "ticket-C-42" })
    );
    assert_eq!(
        outcome.node_traces.last().unwrap().output_payload,
        json!({ "ticket_id": "ticket-C-42" })
    );
}
