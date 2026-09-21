use super::*;

#[test]
fn repeated_llm_node_ids_preserve_every_execution_identity() {
    let run_id = Uuid::now_v7();
    let first_id = Uuid::now_v7();
    let second_id = Uuid::now_v7();
    let node = |id| domain::NodeRunRecord {
        id,
        flow_run_id: run_id,
        node_id: "repeated-llm".into(),
        node_type: "llm".into(),
        node_alias: "LLM".into(),
        status: domain::NodeRunStatus::Succeeded,
        input_payload: serde_json::json!({"execution": id}),
        output_payload: serde_json::json!({}),
        error_payload: None,
        metrics_payload: serde_json::json!({}),
        debug_payload: serde_json::json!({}),
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
    };
    let groups = trace_visible_node_run_groups(&[node(first_id), node(second_id)]);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 1);
    assert_eq!(groups[1].len(), 1);
    assert_eq!(groups[0][0].id, first_id);
    assert_eq!(groups[1][0].id, second_id);
    assert_ne!(groups[0][0].input_payload, groups[1][0].input_payload);
}
