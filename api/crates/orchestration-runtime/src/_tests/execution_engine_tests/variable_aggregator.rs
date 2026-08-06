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
    let groups = json!([
        { "key": "text", "valueType": "string", "candidates": [["missing", "value"], ["node-start", "text"], ["node-start", "text_fallback"]] },
        { "key": "count", "valueType": "number", "candidates": [["node-start", "count"]] },
        { "key": "enabled", "valueType": "boolean", "candidates": [["node-start", "enabled"]] },
        { "key": "metadata", "valueType": "object", "candidates": [["node-start", "metadata"]] },
        { "key": "items", "valueType": "array", "candidates": [["node-start", "items"]] }
    ]);
    let selector_paths = groups
        .as_array()
        .expect("groups fixture must be an array")
        .iter()
        .flat_map(|group| {
            group["candidates"]
                .as_array()
                .expect("candidates fixture must be an array")
        })
        .map(|selector| {
            selector
                .as_array()
                .expect("selector fixture must be an array")
                .iter()
                .map(|segment| {
                    segment
                        .as_str()
                        .expect("selector segment must be a string")
                        .to_string()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let aggregator = CompiledNode {
        node_id: "node-aggregator".to_string(),
        node_type: "variable_aggregator".to_string(),
        alias: "First available by group".to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-start".to_string()],
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::from([(
            "groups".to_string(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "variable_groups".to_string(),
                raw_value: groups,
                selector_paths,
            },
        )]),
        outputs: [
            ("text", "string"),
            ("count", "number"),
            ("enabled", "boolean"),
            ("metadata", "object"),
            ("items", "array"),
        ]
        .into_iter()
        .map(|(key, value_type)| CompiledOutput {
            key: key.to_string(),
            title: key.to_string(),
            value_type: value_type.to_string(),
            selector: vec![key.to_string()],
            json_schema: None,
        })
        .collect(),
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

fn complete_values(text: Value) -> Value {
    json!({
        "text": text,
        "text_fallback": "later",
        "count": 7,
        "enabled": false,
        "metadata": { "source": "fixture" },
        "items": [1, 2]
    })
}

// Root AC-001/002/003/015: all concrete group types use first-existing order; empty string exists.
#[tokio::test]
async fn variable_aggregator_emits_all_groups_and_matched_candidates_in_normal_run() {
    let outcome = start_flow_debug_run(
        &variable_aggregator_plan(),
        &json!({ "node-start": complete_values(json!("")) }),
        &successful_invoker(),
    )
    .await
    .expect("variable aggregator run should execute");

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        outcome.variable_pool["node-aggregator"],
        json!({
            "text": "",
            "count": 7,
            "enabled": false,
            "metadata": { "source": "fixture" },
            "items": [1, 2]
        })
    );
    let trace = outcome
        .node_traces
        .last()
        .expect("aggregator trace should exist");
    assert_eq!(
        trace.debug_payload["matched_candidates"]["text"],
        json!({ "index": 1, "selector": ["node-start", "text"] })
    );
    assert_eq!(
        trace.input_payload["groups"].as_array().map(Vec::len),
        Some(5)
    );
}

// Root AC-004: a group with all candidates missing fails without partial output.
#[tokio::test]
async fn variable_aggregator_fails_with_group_key_when_all_candidates_are_missing() {
    let mut values = complete_values(json!("present"));
    values
        .as_object_mut()
        .expect("fixture object")
        .remove("count");
    let outcome = start_flow_debug_run(
        &variable_aggregator_plan(),
        &json!({ "node-start": values }),
        &successful_invoker(),
    )
    .await
    .expect("missing candidates should be a node outcome");

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("variable_aggregator_no_candidate_value")
            );
            assert_eq!(failure.error_payload["group_key"], json!("count"));
            assert!(failure.error_payload.get("value").is_none());
        }
        other => panic!("expected variable aggregator failure, got {other:?}"),
    }
    assert_eq!(
        outcome.node_traces.last().expect("trace").output_payload,
        json!({})
    );
}

// Root AC-003/014: null is existing but mismatches a concrete type immediately, without fallback.
#[tokio::test]
async fn variable_aggregator_type_mismatch_fails_immediately_without_value_or_partial_output() {
    let outcome = start_flow_debug_run(
        &variable_aggregator_plan(),
        &json!({ "node-start": complete_values(Value::Null) }),
        &successful_invoker(),
    )
    .await
    .expect("type mismatch should be a node outcome");

    let failure = match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => failure,
        other => panic!("expected variable aggregator failure, got {other:?}"),
    };
    assert_eq!(
        failure.error_payload["error_code"],
        json!("variable_aggregator_output_type_mismatch")
    );
    assert_eq!(failure.error_payload["group_key"], json!("text"));
    assert_eq!(
        failure.error_payload["expected_value_type"],
        json!("string")
    );
    assert_eq!(failure.error_payload["actual_value_type"], json!("null"));
    assert_eq!(failure.error_payload["candidate_index"], json!(1));
    assert_eq!(
        failure.error_payload["selector"],
        json!(["node-start", "text"])
    );
    assert!(failure.error_payload.get("value").is_none());
    assert_eq!(
        outcome.node_traces.last().expect("trace").output_payload,
        json!({})
    );
}

// Root AC-006: single-node preview shares the normal executor and debug contract.
#[tokio::test]
async fn variable_aggregator_preview_matches_normal_runtime_semantics() {
    let preview = crate::preview_executor::run_node_preview(
        &variable_aggregator_plan(),
        "node-aggregator",
        &json!({ "node-start": complete_values(json!("preview")) }),
        &successful_invoker(),
    )
    .await
    .expect("variable aggregator preview should execute");

    assert_eq!(preview.node_output["text"], json!("preview"));
    assert_eq!(
        preview.debug_payload["matched_candidates"]["text"],
        json!({ "index": 1, "selector": ["node-start", "text"] })
    );
    assert!(!preview.is_failed());
}
