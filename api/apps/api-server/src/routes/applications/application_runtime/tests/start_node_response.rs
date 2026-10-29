use super::*;

#[test]
fn start_node_response_moves_legacy_output_payload_into_input() {
    let run = domain::NodeRunRecord {
        execution_identity: None,
        id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        node_id: "node-start".to_string(),
        node_type: "start".to_string(),
        node_alias: "Start".to_string(),
        status: domain::NodeRunStatus::Succeeded,
        input_payload: serde_json::json!({}),
        output_payload: serde_json::json!({
            "query": "ping",
            "tools": [
                {
                    "name": "read_file",
                    "source": "openai_compatible"
                }
            ]
        }),
        error_payload: None,
        metrics_payload: serde_json::json!({}),
        debug_payload: serde_json::json!({}),
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
    };

    let response = to_node_run_response(run);

    assert_eq!(response.input_payload["query"], serde_json::json!("ping"));
    assert_eq!(
        response.input_payload["tools"][0]["name"],
        serde_json::json!("read_file")
    );
    assert_eq!(response.output_payload, serde_json::json!({}));
}

#[test]
fn start_node_response_exposes_input_payload_truth_view() {
    let artifact_ref = Uuid::now_v7().to_string();
    let run = domain::NodeRunRecord {
        execution_identity: None,
        id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        node_id: "node-start".to_string(),
        node_type: "start".to_string(),
        node_alias: "Start".to_string(),
        status: domain::NodeRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "query": "say hello",
            "model": "deepseek-chat",
            "files": [{ "name": "brief.md" }],
            "sys": {
                "workflow_run_id": "run-1"
            },
            "env": {
                "ApiBaseUrl": "https://api.example.com"
            },
            "history": {
                "__runtime_debug_artifact": true,
                "artifact_ref": artifact_ref,
                "is_truncated": true,
                "field_path": ["history"],
                "preview": "[{\"role\":\"user\",\"content\":\"old"
            },
            "tools": []
        }),
        output_payload: serde_json::json!({ "query": "say hello" }),
        error_payload: None,
        metrics_payload: serde_json::json!({}),
        debug_payload: serde_json::json!({}),
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
    };

    let response = to_node_run_response(run);

    assert_eq!(response.input_payload["query"], "say hello");
    assert_eq!(response.input_payload["model"], "deepseek-chat");
    assert_eq!(response.input_payload["sys"]["workflow_run_id"], "run-1");
    assert_eq!(
        response.input_payload["env"]["ApiBaseUrl"],
        "https://api.example.com"
    );
    assert_eq!(
        response.input_payload["history"]["field_path"],
        serde_json::json!(["history"])
    );
    assert_eq!(response.input_payload_view, response.input_payload);
}
