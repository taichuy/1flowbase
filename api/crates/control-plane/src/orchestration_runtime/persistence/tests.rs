use orchestration_runtime::execution_state::{
    ExecutionStopReason, FlowDebugExecutionOutcome, NativeOperationTerminal, NodeExecutionFailure,
    NodeExecutionTrace,
};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    answer_presentation_terminal_events, canonical_terminal_output_payload, checkpoint_node_id,
    checkpoint_snapshot_from_record, CheckpointLocatorPayload,
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

fn answer_presentation_plan() -> orchestration_runtime::compiled_plan::CompiledPlan {
    serde_json::from_value(json!({
        "flow_id": Uuid::nil(),
        "source_draft_id": "draft-1",
        "schema_version": "1flowbase.flow/v2",
        "topological_order": ["node-llm", "node-answer"],
        "edges": [],
        "compile_issues": [],
        "nodes": {
            "node-llm": {
                "node_id": "node-llm",
                "node_type": "llm",
                "alias": "LLM",
                "container_id": null,
                "dependency_node_ids": [],
                "downstream_node_ids": ["node-answer"],
                "bindings": {},
                "outputs": [{
                    "key": "text",
                    "title": "Text",
                    "value_type": "string",
                    "selector": ["text"]
                }],
                "config": {}
            },
            "node-answer": {
                "node_id": "node-answer",
                "node_type": "answer",
                "alias": "Answer",
                "container_id": null,
                "dependency_node_ids": ["node-llm"],
                "downstream_node_ids": [],
                "bindings": {
                    "answer_template": {
                        "kind": "selector",
                        "raw_value": ["node-llm", "text"],
                        "selector_paths": [["node-llm", "text"]]
                    }
                },
                "outputs": [{
                    "key": "answer",
                    "title": "Answer",
                    "value_type": "string",
                    "selector": ["answer"]
                }],
                "config": {}
            }
        }
    }))
    .unwrap()
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
        operation_terminal: None,
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
        canonical_terminal_output_payload(None, &outcome).unwrap(),
        json!({ "text": "first answer" })
    );
}

#[test]
fn completed_flow_output_uses_terminal_node_payload() {
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![
            trace("llm-1", json!({ "text": "first answer" }), None),
            trace("answer", json!({ "answer": "final answer" }), None),
        ],
    };

    assert_eq!(
        canonical_terminal_output_payload(None, &outcome).unwrap(),
        json!({
            "answer": "final answer",
            "__canonical_answer_presentation": true
        })
    );
}

#[test]
fn ac_005_terminal_answer_presentation_is_materialized_from_canonical_provider_events() {
    use plugin_framework::provider_contract::{ProviderFinishReason, ProviderStreamEvent};

    let plan = answer_presentation_plan();
    let mut llm_trace = trace("node-llm", json!({ "text": "wrong fallback" }), None);
    llm_trace.provider_events = vec![
        ProviderStreamEvent::TextDelta {
            delta: "<think>reason</think>same  ".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "\n`code`same  ".to_string(),
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        },
    ];
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::from_iter([
            ("node-llm".to_string(), json!({ "text": "wrong fallback" })),
            (
                "node-answer".to_string(),
                json!({ "answer": "wrong fallback" }),
            ),
        ]),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![
            llm_trace,
            typed_trace(
                "node-answer",
                "answer",
                json!({ "answer": "wrong fallback" }),
                None,
            ),
        ],
    };

    let events = answer_presentation_terminal_events(
        Some(&plan),
        &outcome,
        None,
        "node-answer",
        &json!({ "answer": "wrong fallback" }),
    )
    .unwrap();
    let text = events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    let reasoning = events
        .iter()
        .filter(|event| event.event_type == "reasoning_delta")
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();

    assert_eq!(text, "same  \n`code`same  ");
    assert_eq!(reasoning, "reason");
    assert!(!text.contains("wrong fallback"));
    assert_eq!(
        canonical_terminal_output_payload(Some(&plan), &outcome).unwrap()["answer"],
        json!("same  \n`code`same  ")
    );

    let mut failed_outcome = outcome;
    failed_outcome.stop_reason = ExecutionStopReason::Failed(NodeExecutionFailure {
        node_id: "node-answer".to_string(),
        node_alias: "Answer".to_string(),
        error_payload: json!({"message": "failed after canonical partial"}),
    });
    let failed_partial = canonical_terminal_output_payload(Some(&plan), &failed_outcome)
        .expect("failed outcome should retain canonical partial output");
    assert_eq!(failed_partial["answer"], json!("same  \n`code`same  "));
    assert_eq!(
        failed_partial["__canonical_answer_presentation"],
        json!(true)
    );
}

#[test]
fn ac_001_answer_presentation_projects_multiple_strict_provider_rounds() {
    use plugin_framework::provider_contract::{ProviderFinishReason, ProviderStreamEvent};

    let plan = answer_presentation_plan();
    let mut llm_trace = trace("node-llm", json!({ "text": "wrong fallback" }), None);
    llm_trace.provider_events = vec![
        ProviderStreamEvent::TextDelta {
            delta: "<think>first round reasoning".to_string(),
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::ToolCall,
        },
        ProviderStreamEvent::ReasoningDelta {
            delta: "second round reasoning".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "second round answer".to_string(),
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::ToolCall,
        },
    ];
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::from_iter([
            ("node-llm".to_string(), json!({ "text": "wrong fallback" })),
            (
                "node-answer".to_string(),
                json!({ "answer": "wrong fallback" }),
            ),
        ]),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![
            llm_trace,
            typed_trace(
                "node-answer",
                "answer",
                json!({ "answer": "wrong fallback" }),
                None,
            ),
        ],
    };

    let events = answer_presentation_terminal_events(
        Some(&plan),
        &outcome,
        None,
        "node-answer",
        &json!({ "answer": "wrong fallback" }),
    )
    .expect("each Provider round should have an independent canonical terminal");
    let projected = events
        .iter()
        .filter_map(|event| {
            matches!(event.event_type.as_str(), "reasoning_delta" | "text_delta").then(|| {
                (
                    event.event_type.as_str(),
                    event.payload["text"].as_str().unwrap(),
                )
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        projected,
        vec![
            ("reasoning_delta", "first round reasoning"),
            ("reasoning_delta", "second round reasoning"),
            ("text_delta", "second round answer"),
        ]
    );
    assert_eq!(
        canonical_terminal_output_payload(Some(&plan), &outcome).unwrap()["answer"],
        json!("second round answer")
    );
}

#[test]
fn canonical_native_operation_terminal_wins_over_later_ordinary_node_payload() {
    for terminal in [
        json!({
            "semantic_terminal": "count_tokens",
            "result": { "operation": "count_tokens", "input_tokens": 41 }
        }),
        json!({
            "semantic_terminal": "compact",
            "result": {
                "result_type": "response_items",
                "operation": "compact",
                "profile": "responses_compact",
                "response_items": [{ "type": "message" }]
            }
        }),
    ] {
        let outcome = FlowDebugExecutionOutcome {
            stop_reason: ExecutionStopReason::Completed,
            variable_pool: Map::new(),
            checkpoint_snapshot: None,
            operation_terminal: NativeOperationTerminal::from_payload(&terminal).unwrap(),
            node_traces: vec![
                trace("llm-operation", terminal.clone(), None),
                trace("ordinary-tail", json!({ "text": "must not win" }), None),
            ],
        };
        assert_eq!(
            canonical_terminal_output_payload(None, &outcome).unwrap(),
            terminal
        );
    }
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
        operation_terminal: None,
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

    assert_eq!(
        canonical_terminal_output_payload(None, &outcome).unwrap(),
        json!({
            "answer": "partial final answer",
            "error": answer_output["error"].clone(),
            "__canonical_answer_presentation": true
        })
    );
}

#[test]
fn ac_015_provider_request_log_task_projects_empty_response_and_attempt_usage() {
    let attempt_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let scope_id = Uuid::now_v7();
    let started_at = OffsetDateTime::UNIX_EPOCH;
    let finished_at = started_at + time::Duration::milliseconds(7426);
    let attempt = json!({
        "attempt_index": 0,
        "provider_instance_id": Uuid::nil(),
        "provider_instance_display_name": "Gemini 01",
        "provider_code": "gemini",
        "protocol": "google_genai",
        "upstream_model_id": "gemini-3-flash",
        "status": "empty_response",
        "event_count": 2,
        "time_to_first_token_ms": null,
        "usage": {"input_tokens": 12, "output_tokens": 0, "total_tokens": 12}
    });

    let task = super::model_attempts::provider_request_log_task_from_attempt(
        scope_id,
        attempt_id,
        flow_run_id,
        node_run_id,
        Some(Uuid::nil()),
        Some("conversation-1"),
        "应用快照",
        started_at,
        finished_at,
        &attempt,
    );

    assert_eq!(task.application_name, "应用快照");
    assert_eq!(task.node_run_id, Some(node_run_id));
    assert_eq!(task.application_id, Some(Uuid::nil()));
    assert_eq!(task.conversation_id.as_deref(), Some("conversation-1"));
    assert_eq!(task.attempt_index, 1);
    assert_eq!(
        task.provider_instance_display_name.as_deref(),
        Some("Gemini 01")
    );
    assert_eq!(task.status, "empty_response");
    assert_eq!(task.input_tokens, Some(12));
    assert_eq!(task.output_tokens, Some(0));
    assert_eq!(task.total_tokens, Some(12));
    assert_eq!(task.time_to_first_token_ms, None);
    assert_eq!(task.total_duration_ms, Some(7426));
    serde_json::to_value(task).unwrap();
}

#[test]
fn provider_request_log_task_accepts_legacy_queue_payload_without_node_run_id_ac_003() {
    let task = super::model_attempts::provider_request_log_task_from_attempt(
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        None,
        "Legacy queue fixture",
        OffsetDateTime::UNIX_EPOCH,
        OffsetDateTime::UNIX_EPOCH,
        &json!({}),
    );
    let mut payload = serde_json::to_value(task).expect("serialize request log task");
    payload
        .as_object_mut()
        .expect("request log task object")
        .remove("node_run_id");

    let restored: crate::ports::ProviderRequestLogTask =
        serde_json::from_value(payload).expect("legacy request log task");
    assert_eq!(restored.node_run_id, None);
}
