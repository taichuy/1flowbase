use super::*;

fn plugin_plan() -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-plugin".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "query".to_string(),
                title: "用户输入".to_string(),
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
    nodes.insert(
        "node-plugin".to_string(),
        CompiledNode {
            node_id: "node-plugin".to_string(),
            node_type: "plugin_node".to_string(),
            alias: "Plugin Node".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::from([(
                "query".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "selector".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                    raw_value: json!(["node-start", "query"]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "answer".to_string(),
                title: "回答".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "prompt": "Hello {{ node-start.query }}"
            }),
            plugin_runtime: Some(CompiledPluginRuntime {
                installation_id: Uuid::nil(),
                plugin_unique_identifier: "fixture_capability".to_string(),
                package_id: "fixture_capability@0.1.0".to_string(),
                plugin_id: "fixture_capability@0.1.0".to_string(),
                plugin_version: "0.1.0".to_string(),
                contribution_code: "fixture_action".to_string(),
                node_shell: "action".to_string(),
                schema_version: "1flowbase.node-contribution/v2".to_string(),
                contribution_checksum: "sha256:contribution".to_string(),
                compiled_contribution_hash: "sha256:compiled".to_string(),
                output_schema_snapshot: vec![CompiledOutput {
                    key: "answer".to_string(),
                    title: "回答".to_string(),
                    value_type: "string".to_string(),
                    selector: Vec::new(),
                    json_schema: None,
                }],
                side_effect_policy: "external_read".to_string(),
            }),
            llm_runtime: None,
            code_runtime: None,
        },
    );

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-plugin".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec!["node-start".to_string(), "node-plugin".to_string()],
        edges: Vec::new(),
        nodes,
        compile_issues: Vec::new(),
    }
}

#[tokio::test]
async fn plugin_node_routes_to_capability_runtime_and_preserves_output_payload() {
    let plan = plugin_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "world" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(outcome.node_traces[1].node_type, "plugin_node");
    assert_eq!(outcome.node_traces[1].output_payload["answer"], "world");
}

#[tokio::test]
async fn plugin_node_keeps_executor_output_keys_outside_compiled_contract_hidden_from_variable_pool(
) {
    let outcome = start_flow_debug_run(
        &plugin_plan(),
        &json!({ "node-start": { "query": "world" } }),
        &UnknownCapabilityOutputInvoker,
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.node_traces[1].output_payload["unexpected"],
        json!(true)
    );
    assert!(outcome.variable_pool["node-plugin"]
        .get("unexpected")
        .is_none());
}

#[tokio::test]
async fn plugin_node_keeps_runtime_named_executor_output_keys_hidden_from_variable_pool() {
    let outcome = start_flow_debug_run(
        &plugin_plan(),
        &json!({ "node-start": { "query": "world" } }),
        &ReservedCapabilityOutputInvoker,
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.node_traces[1].output_payload["metadata"]["secret"],
        json!("x")
    );
    assert!(outcome.variable_pool["node-plugin"]
        .get("metadata")
        .is_none());
}

#[tokio::test]
async fn unknown_node_type_returns_not_implemented_failure_in_debug_runtime() {
    let mut plan = base_plan();
    if let Some(node_llm) = plan.nodes.get_mut("node-llm") {
        node_llm.node_type = "x_unknown".to_string();
        node_llm.alias = "Unknown".to_string();
    }

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(failure.node_alias, "Unknown");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("node_type_not_implemented")
            );
            assert_eq!(failure.error_payload["node_type"], json!("x_unknown"));
            assert_eq!(
                failure.error_payload["message"],
                json!("x_unknown nodes are not implemented in preview runtime")
            );
            assert_eq!(outcome.node_traces[1].node_type, "x_unknown");
            assert!(outcome.node_traces[1]
                .output_payload
                .as_object()
                .unwrap()
                .is_empty());
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["node_type"],
                json!("x_unknown")
            );
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }

    assert!(outcome.variable_pool.get("node-llm").is_none());
    assert_eq!(outcome.node_traces.len(), 2);
}
