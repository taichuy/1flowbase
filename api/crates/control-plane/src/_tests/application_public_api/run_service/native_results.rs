use control_plane::application_public_api::{
    native::{NativeRunStatus, NativeUsage},
    run_service::{
        native_result_from_flow_run, native_result_from_run_stream_state, PublishedRunNodeUsage,
        PublishedRunPendingCallback, PublishedRunStreamState,
    },
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

fn uuid(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn failed_published_flow_run(error_payload: serde_json::Value) -> domain::FlowRunRecord {
    let now = OffsetDateTime::now_utc();
    domain::FlowRunRecord {
        id: uuid(1),
        application_id: uuid(2),
        flow_id: uuid(3),
        draft_id: uuid(4),
        compiled_plan_id: None,
        debug_session_id: String::new(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "Published run".to_string(),
        status: domain::FlowRunStatus::Failed,
        input_payload: json!({}),
        output_payload: json!({}),
        error_payload: Some(error_payload),
        created_by: uuid(5),
        authorized_account: None,
        api_key_id: Some(uuid(6)),
        publication_version_id: Some(uuid(7)),
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: now,
        finished_at: Some(now),
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn native_result_omits_provider_upstream_raw_details_from_public_error() {
    let flow_run = failed_published_flow_run(json!({
        "error_code": "provider_upstream_error",
        "message": "400 Bad Request: missing instructions",
        "provider_summary": "[REDACTED]",
        "provider_details": {
            "status": 400,
            "content_type": "application/json",
            "headers": {
                "x-request-id": "req_123"
            },
            "raw_body": "{\"error\":{\"message\":\"missing instructions\"}}\n"
        }
    }));

    let result = native_result_from_flow_run(&flow_run, json!({}));
    let error = result
        .error
        .expect("failed native run should expose an error");

    assert_eq!(error.code, "provider_upstream_error");
    assert_eq!(error.message, "provider upstream request failed");
    assert_eq!(error.details["status_code"], json!(400));
    assert!(error.details.get("provider_summary").is_none());
    assert!(error.details.get("provider_details").is_none());
}

#[test]
fn native_result_sanitizes_legacy_provider_upstream_raw_body_from_public_error() {
    let raw_body = "plain upstream failure body with request payload";
    let flow_run = failed_published_flow_run(json!({
        "error_code": "provider_upstream_error",
        "message": format!("400 Bad Request: {raw_body}"),
        "provider_summary": raw_body,
        "provider_details": {
            "status": 400,
            "content_type": "text/plain",
            "headers": {
                "x-request-id": "req_123"
            },
            "raw_body": raw_body
        }
    }));

    let result = native_result_from_flow_run(&flow_run, json!({}));
    let error = result
        .error
        .expect("failed native run should expose an error");

    assert_eq!(error.code, "provider_upstream_error");
    assert_eq!(error.message, "provider upstream request failed");
    assert_eq!(error.details["status_code"], json!(400));
    assert!(error.details.get("provider_summary").is_none());
    assert!(error.details.get("provider_details").is_none());
    assert!(!error.message.contains(raw_body));
    assert!(!error.details.to_string().contains(raw_body));
}

#[test]
fn d1_ac_001_failed_native_result_never_projects_an_answer_or_success_artifact() {
    let raw_provider_body = "429 rate limit: upstream diagnostic body";
    let mut flow_run = failed_published_flow_run(json!({
        "error_code": "provider_upstream_error",
        "message": raw_provider_body,
        "provider_details": { "raw_body": raw_provider_body }
    }));
    flow_run.output_payload = json!({
        "answer": raw_provider_body,
        "answer_segments": [{ "kind": "message", "text": raw_provider_body }],
        "finish_reason": "stop"
    });

    let result = native_result_from_flow_run(&flow_run, json!({}));

    assert_eq!(result.status, NativeRunStatus::Failed);
    assert!(
        result.answer.is_none(),
        "D1-AC-001: a failed run cannot expose provider error text as an Answer"
    );
    assert!(
        result.answer_segments.is_none(),
        "D1-AC-001: a failed run cannot expose a successful answer artifact"
    );
    let error = result
        .error
        .expect("failed run should expose a sanitized error");
    assert!(!error.message.contains(raw_provider_body));
    assert!(!error.details.to_string().contains(raw_provider_body));
}

#[test]
fn d1_ac_007_durable_incomplete_run_projects_the_same_non_success_terminal() {
    let mut flow_run = failed_published_flow_run(json!({}));
    flow_run.status = domain::FlowRunStatus::Incomplete;
    flow_run.error_payload = None;
    flow_run.output_payload = json!({ "answer": "partial output at the limit" });

    let initial = native_result_from_flow_run(&flow_run, json!({}));

    assert_eq!(initial.status, NativeRunStatus::Incomplete);
    assert_eq!(
        initial.answer.as_deref(),
        Some("partial output at the limit")
    );
    assert!(initial.error.is_none());

    let replay = native_result_from_run_stream_state(
        &initial,
        &PublishedRunStreamState {
            status: domain::FlowRunStatus::Incomplete,
            output_payload: flow_run.output_payload.clone(),
            error_payload: None,
            node_usages: Vec::new(),
            latest_pending_callback: None,
        },
    );

    assert_eq!(replay.status, NativeRunStatus::Incomplete);
    assert_eq!(replay.answer, initial.answer);
    assert!(replay.error.is_none());
}

#[test]
fn native_result_from_stream_state_preserves_usage_and_pending_tool_callback_contract() {
    let mut flow_run = failed_published_flow_run(json!({}));
    flow_run.status = domain::FlowRunStatus::Running;
    flow_run.input_payload = json!({ "node-start": { "query": "hello" } });
    flow_run.error_payload = None;
    let initial = native_result_from_flow_run(&flow_run, json!({ "request_id": "req-1" }));
    let callback_task = domain::CallbackTaskRecord {
        id: uuid(8),
        flow_run_id: flow_run.id,
        node_run_id: uuid(9),
        callback_kind: "llm_tool_calls".to_string(),
        status: domain::CallbackTaskStatus::Pending,
        request_payload: json!({
            "tool_calls": [{
                "id": "toolu_latest",
                "name": "Read",
                "arguments": { "path": "README.md" }
            }]
        }),
        response_payload: None,
        external_ref_payload: None,
        created_at: OffsetDateTime::now_utc(),
        completed_at: None,
    };
    let stream_state = PublishedRunStreamState {
        status: domain::FlowRunStatus::WaitingCallback,
        output_payload: json!({}),
        error_payload: None,
        node_usages: vec![PublishedRunNodeUsage {
            metrics_usage: Some(json!({ "input_tokens": 21, "output_tokens": 8 })),
            output_usage: Some(json!({ "prompt_tokens": 999, "completion_tokens": 999 })),
        }],
        latest_pending_callback: Some(PublishedRunPendingCallback {
            id: callback_task.id,
            flow_run_id: callback_task.flow_run_id,
            node_run_id: callback_task.node_run_id,
            callback_kind: callback_task.callback_kind.clone(),
            request_payload: None,
            tool_calls: callback_task.request_payload.get("tool_calls").cloned(),
        }),
    };

    let result = native_result_from_run_stream_state(&initial, &stream_state);

    assert_eq!(result.id, initial.id);
    assert_eq!(result.node_input_payload, initial.node_input_payload);
    assert_eq!(result.metadata, initial.metadata);
    assert_eq!(result.status, NativeRunStatus::Waiting);
    assert_eq!(
        result.usage,
        Some(NativeUsage {
            prompt_tokens: Some(21),
            completion_tokens: Some(8),
            total_tokens: Some(29),
            ..NativeUsage::default()
        })
    );
    assert_eq!(
        result
            .required_action
            .as_ref()
            .map(|action| action.action_type.as_str()),
        Some("submit_tool_outputs")
    );
    assert_eq!(
        result.required_action.as_ref().unwrap().payload["callback_task_id"],
        json!(callback_task.id)
    );
    assert_eq!(
        result.tool_calls.as_ref().unwrap()[0]["id"],
        json!("toolu_latest")
    );
}
