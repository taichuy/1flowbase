use orchestration_runtime::execution_state::{
    ExecutionStopReason, FlowDebugExecutionOutcome, NodeExecutionFailure, NodeExecutionTrace,
};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    checkpoint_node_id, checkpoint_snapshot_from_record, final_flow_output_payload,
    CheckpointLocatorPayload,
};

fn checkpoint_record(locator_payload: Value, variable_snapshot: Value) -> domain::CheckpointRecord {
    domain::CheckpointRecord {
        id: Uuid::nil(),
        flow_run_id: Uuid::nil(),
        node_run_id: None,
        status: "waiting_callback".to_string(),
        reason: "waiting".to_string(),
        locator_payload,
        variable_snapshot,
        external_ref_payload: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn typed_trace(
    node_id: &str,
    node_type: &str,
    output_payload: Value,
    error_payload: Option<Value>,
) -> NodeExecutionTrace {
    NodeExecutionTrace {
        node_id: node_id.to_string(),
        node_type: node_type.to_string(),
        node_alias: node_id.to_string(),
        input_payload: json!({}),
        output_payload,
        error_payload,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        provider_events: Vec::new(),
    }
}

fn trace(node_id: &str, output_payload: Value, error_payload: Option<Value>) -> NodeExecutionTrace {
    typed_trace(node_id, "llm", output_payload, error_payload)
}

#[test]
fn checkpoint_snapshot_from_record_reads_active_node_ids() {
    let checkpoint = checkpoint_record(
        json!({
            "node_id": "node-human",
            "next_node_index": 3,
            "active_node_ids": ["node-answer", "node-followup"]
        }),
        json!({ "node-human": { "answer": "ok" } }),
    );

    let snapshot = checkpoint_snapshot_from_record(&checkpoint).unwrap();

    assert_eq!(snapshot.next_node_index, 3);
    assert_eq!(
        snapshot.active_node_ids,
        vec!["node-answer".to_string(), "node-followup".to_string()]
    );
}

#[test]
fn checkpoint_snapshot_from_record_requires_active_node_ids() {
    let checkpoint = checkpoint_record(
        json!({
            "node_id": "node-human",
            "next_node_index": 3
        }),
        json!({}),
    );

    let error = checkpoint_snapshot_from_record(&checkpoint).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("checkpoint is missing active_node_ids"),
        "{error}"
    );
}

#[test]
fn checkpoint_locator_payload_round_trips_snapshot_fields() {
    let snapshot = orchestration_runtime::execution_state::CheckpointSnapshot {
        next_node_index: 3,
        variable_pool: Map::from_iter([("node-human".to_string(), json!({ "answer": "ok" }))]),
        active_node_ids: vec!["node-answer".to_string(), "node-followup".to_string()],
    };
    let locator_payload =
        CheckpointLocatorPayload::from_snapshot("node-human", &snapshot).into_json();
    let checkpoint = checkpoint_record(
        locator_payload,
        Value::Object(snapshot.variable_pool.clone()),
    );

    let locator = CheckpointLocatorPayload::from_record(&checkpoint).unwrap();
    let restored = locator
        .into_checkpoint_snapshot(&checkpoint.variable_snapshot)
        .unwrap();

    assert_eq!(restored, snapshot);
}

#[test]
fn checkpoint_locator_payload_from_runtime_position_preserves_branch_state() {
    let checkpoint = checkpoint_record(
        CheckpointLocatorPayload::from_runtime_position(
            "node-tool",
            2,
            vec!["node-answer".to_string(), "node-cleanup".to_string()],
        )
        .into_json(),
        json!({ "node-tool": { "output": "waiting" } }),
    );

    let snapshot = checkpoint_snapshot_from_record(&checkpoint).unwrap();

    assert_eq!(checkpoint_node_id(&checkpoint).unwrap(), "node-tool");
    assert_eq!(snapshot.next_node_index, 2);
    assert_eq!(
        snapshot.active_node_ids,
        vec!["node-answer".to_string(), "node-cleanup".to_string()]
    );
}

#[test]
fn failed_flow_output_keeps_last_successful_node_payload() {
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
            node_id: "llm-2".to_string(),
            node_alias: "LLM2".to_string(),
            error_payload: json!({ "message": "provider worker ended without result line" }),
        }),
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        node_traces: vec![
            trace("start", json!({}), None),
            trace("llm-1", json!({ "text": "first answer" }), None),
            trace(
                "llm-2",
                json!({}),
                Some(json!({ "message": "provider worker ended without result line" })),
            ),
        ],
    };

    assert_eq!(
        final_flow_output_payload(&outcome),
        json!({ "text": "first answer" })
    );
}

#[test]
fn completed_flow_output_uses_terminal_node_payload() {
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        node_traces: vec![
            trace("llm-1", json!({ "text": "first answer" }), None),
            trace("answer", json!({ "answer": "final answer" }), None),
        ],
    };

    assert_eq!(
        final_flow_output_payload(&outcome),
        json!({ "answer": "final answer" })
    );
}

#[test]
fn failed_flow_output_uses_terminal_answer_payload_even_when_answer_has_error() {
    let answer_error = json!({
        "error_kind": "prompt_template_unresolved",
        "message": "Answer node rendered with unresolved template selectors",
    });
    let answer_output = json!({
        "answer": "partial final answer",
        "error": answer_error.clone(),
    });
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
            node_id: "answer".to_string(),
            node_alias: "Answer".to_string(),
            error_payload: answer_error.clone(),
        }),
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        node_traces: vec![
            trace("llm-1", json!({ "text": "partial final answer" }), None),
            typed_trace(
                "answer",
                "answer",
                answer_output.clone(),
                Some(answer_error),
            ),
        ],
    };

    assert_eq!(final_flow_output_payload(&outcome), answer_output);
}

#[test]
fn ac_015_provider_request_log_task_projects_empty_response_and_attempt_usage() {
    let attempt_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let scope_id = Uuid::now_v7();
    let started_at = OffsetDateTime::UNIX_EPOCH;
    let attempt = json!({
        "attempt_index": 0,
        "provider_instance_id": Uuid::nil(),
        "provider_instance_display_name": "Gemini 01",
        "provider_code": "gemini",
        "protocol": "google_genai",
        "upstream_model_id": "gemini-3-flash",
        "status": "succeeded",
        "event_count": 0,
        "usage": {"input_tokens": 12, "output_tokens": 0, "total_tokens": 12}
    });

    let task = super::model_attempts::provider_request_log_task_from_attempt(
        scope_id,
        attempt_id,
        flow_run_id,
        "应用快照",
        started_at,
        started_at,
        &attempt,
    );

    assert_eq!(task.application_name, "应用快照");
    assert_eq!(task.attempt_index, 1);
    assert_eq!(
        task.provider_instance_display_name.as_deref(),
        Some("Gemini 01")
    );
    assert_eq!(task.status, "empty_response");
    assert_eq!(task.input_tokens, Some(12));
    assert_eq!(task.output_tokens, Some(0));
    assert_eq!(task.total_tokens, Some(12));
    serde_json::to_value(task).unwrap();
}
