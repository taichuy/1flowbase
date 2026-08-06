use super::*;
use serde_json::Map;

fn variable_aggregator_plan() -> CompiledPlan {
    let start = CompiledNode {
        node_id: "node-start".to_string(),
        node_type: "start".to_string(),
        alias: "Start".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: vec!["node-aggregator".to_string()],
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };
    let aggregator = CompiledNode {
        node_id: "node-aggregator".to_string(),
        node_type: "variable_aggregator".to_string(),
        alias: "First available".to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-start".to_string()],
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::from([(
            "candidates".to_string(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "selector_list".to_string(),
                raw_value: json!([
                    ["missing", "value"],
                    ["node-start", "primary"],
                    ["node-start", "fallback"]
                ]),
                selector_paths: vec![
                    vec!["missing".to_string(), "value".to_string()],
                    vec!["node-start".to_string(), "primary".to_string()],
                    vec!["node-start".to_string(), "fallback".to_string()],
                ],
            },
        )]),
        outputs: vec![CompiledOutput {
            key: "value".to_string(),
            title: "Value".to_string(),
            value_type: "any".to_string(),
            selector: vec!["value".to_string()],
            json_schema: None,
        }],
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };

    CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec!["node-start".to_string(), "node-aggregator".to_string()],
        edges: vec![CompiledEdge {
            edge_id: "edge-start-aggregator".to_string(),
            source: "node-start".to_string(),
            target: "node-aggregator".to_string(),
            source_handle: None,
            target_handle: None,
        }],
        nodes: BTreeMap::from([
            ("node-start".to_string(), start),
            ("node-aggregator".to_string(), aggregator),
        ]),
        compile_issues: Vec::new(),
    }
}

// Root AC-001/002/003: missing candidates are skipped and null/empty values are matches.
#[tokio::test]
async fn variable_aggregator_uses_first_existing_candidate_in_normal_run() {
    for primary in [Value::Null, json!("")] {
        let outcome = start_flow_debug_run(
            &variable_aggregator_plan(),
            &json!({
                "node-start": {
                    "primary": primary.clone(),
                    "fallback": "later"
                }
            }),
            &successful_invoker(),
        )
        .await
        .expect("variable aggregator run should execute");

        assert!(matches!(
            outcome.stop_reason,
            ExecutionStopReason::Completed
        ));
        assert_eq!(outcome.variable_pool["node-aggregator"]["value"], primary);
        let trace = outcome
            .node_traces
            .last()
            .expect("aggregator trace should exist");
        assert_eq!(
            trace.debug_payload["matched_candidate"],
            json!({ "index": 1, "selector": ["node-start", "primary"] })
        );
        assert_eq!(trace.debug_payload.as_object().map(Map::len), Some(1));
    }
}

// Root AC-004: an entirely missing candidate list has a stable node error code.
#[tokio::test]
async fn variable_aggregator_fails_when_all_candidates_are_missing() {
    let outcome = start_flow_debug_run(
        &variable_aggregator_plan(),
        &json!({ "node-start": {} }),
        &successful_invoker(),
    )
    .await
    .expect("missing candidates should be a node outcome");

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => assert_eq!(
            failure.error_payload["error_code"],
            json!("variable_aggregator_no_candidate_value")
        ),
        other => panic!("expected variable aggregator failure, got {other:?}"),
    }
}

// Root AC-006: single-node preview uses the same first-existing rule and debug shape.
#[tokio::test]
async fn variable_aggregator_preview_matches_normal_runtime_semantics() {
    let preview = crate::preview_executor::run_node_preview(
        &variable_aggregator_plan(),
        "node-aggregator",
        &json!({ "node-start": { "fallback": 42 } }),
        &successful_invoker(),
    )
    .await
    .expect("variable aggregator preview should execute");

    assert_eq!(preview.node_output, json!({ "value": 42 }));
    assert_eq!(
        preview.debug_payload["matched_candidate"],
        json!({ "index": 2, "selector": ["node-start", "fallback"] })
    );
    assert!(!preview.is_failed());
}
