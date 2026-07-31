use super::*;

#[tokio::test]
async fn ac_006_sql_node_output_enters_trace_and_variable_pool() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-sql".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-sql".to_string(),
        CompiledNode {
            node_id: "node-sql".to_string(),
            node_type: "sql".to_string(),
            alias: "SQL".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "results".to_string(),
                title: "Results".to_string(),
                value_type: "array".to_string(),
                selector: vec!["results".to_string()],
                json_schema: None,
            }],
            config: json!({
                "data_source_instance_id": "main",
                "sql": "select 1"
            }),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    let plan = CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec!["node-start".to_string(), "node-sql".to_string()],
        edges: vec![CompiledEdge {
            edge_id: "edge-start-sql".to_string(),
            source: "node-start".to_string(),
            target: "node-sql".to_string(),
            source_handle: None,
            target_handle: None,
        }],
        nodes,
        compile_issues: vec![],
    };
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: String::new(),
    };

    let outcome = start_flow_debug_run(&plan, &json!({}), &invoker)
        .await
        .unwrap();

    assert_eq!(
        outcome.variable_pool["node-sql"]["results"][0]["affected_rows"],
        1
    );
    let sql_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-sql")
        .unwrap();
    assert_eq!(sql_trace.output_payload["results"][0]["kind"], "completion");
    assert!(sql_trace.error_payload.is_none());
}
