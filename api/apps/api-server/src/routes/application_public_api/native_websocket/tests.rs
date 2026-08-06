use axum::extract::ws::Message;
use control_plane::{
    application_public_api::native::{NativeRunResult, NativeRunStatus},
    orchestration_runtime::debug_stream_events,
    ports::RuntimeEventEnvelope,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    actor::{NativeConnectionActor, NativeConnectionState},
    projector::NativeWebSocketProjector,
    require_native_websocket_protocol,
    schema::{decode_client_message, sequence_from_event_id, NativeWebSocketClientCommand},
};
use crate::routes::application_public_api::sse::IncludeWorkflowEvents;

fn native_run() -> NativeRunResult {
    NativeRunResult {
        id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        api_key_id: Uuid::now_v7(),
        publication_version_id: Uuid::now_v7(),
        status: NativeRunStatus::Running,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn issue_1601_native_websocket_decodes_typed_commands() {
    let command = decode_client_message(Message::Text(
        json!({
            "type": "run.create",
            "request_id": "req-1",
            "request": {"query": "hello"}
        })
        .to_string(),
    ))
    .expect("command should decode")
    .expect("text should contain a command");
    assert!(matches!(
        command,
        NativeWebSocketClientCommand::Create { .. }
    ));
}

#[test]
fn issue_1601_native_attach_keeps_workflow_visibility_explicit() {
    let command = decode_client_message(Message::Text(
        json!({
            "type": "run.attach",
            "request_id": "req-attach",
            "run_id": Uuid::now_v7(),
            "after_event_id": null,
            "stream_options": {"include_workflow_events": "public"}
        })
        .to_string(),
    ))
    .unwrap()
    .unwrap();
    assert!(matches!(
        command,
        NativeWebSocketClientCommand::Attach { stream_options, .. }
            if stream_options.include_workflow_events
                == control_plane::application_public_api::native::NativeWorkflowEventVisibility::Public
    ));
}

#[test]
fn issue_1601_native_websocket_projects_deltas_before_one_terminal() {
    let mut run = native_run();
    let mut projector =
        NativeWebSocketProjector::new("req-stream".to_string(), IncludeWorkflowEvents::Public);
    let first = projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                1,
                debug_stream_events::answer_text_delta(
                    "answer",
                    "Hel".to_string(),
                    1,
                    Some("llm"),
                    None,
                    Some("text"),
                ),
            ),
        )
        .unwrap()
        .unwrap();
    let second = projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                2,
                debug_stream_events::answer_text_delta(
                    "answer",
                    "lo".to_string(),
                    2,
                    Some("llm"),
                    None,
                    Some("text"),
                ),
            ),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&first).unwrap()["type"],
        "message.delta"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&second).unwrap()["type"],
        "message.delta"
    );
    assert!(!projector.has_terminal());

    run.status = NativeRunStatus::Succeeded;
    run.answer = Some("Hello".to_string());
    let terminal = projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                3,
                debug_stream_events::flow_finished(run.id, json!({"answer": "Hello"})),
            ),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&terminal).unwrap()["type"],
        "run.completed"
    );
    assert!(projector.has_terminal());
    assert!(projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                4,
                debug_stream_events::answer_text_delta(
                    "answer",
                    "Hello".to_string(),
                    4,
                    Some("llm"),
                    None,
                    Some("text"),
                ),
            ),
        )
        .unwrap()
        .is_none());
}

#[test]
fn issue_1601_native_websocket_rejects_cross_run_cursor() {
    let run_id = Uuid::now_v7();
    let other = Uuid::now_v7();
    assert!(sequence_from_event_id(run_id, Some(&format!("{other}:7"))).is_err());
    assert_eq!(
        sequence_from_event_id(run_id, Some(&format!("{run_id}:7"))).unwrap(),
        Some(7)
    );
}

#[test]
fn issue_1601_native_websocket_allows_only_one_active_turn() {
    let mut actor = NativeConnectionActor::new();
    let first = actor.start_turn().expect("first turn should start");
    assert_eq!(actor.state(), NativeConnectionState::Active);
    assert_eq!(actor.start_turn(), Err("active_run_exists"));
    actor.complete_turn(first, first);
    assert_eq!(actor.state(), NativeConnectionState::Idle);
    assert!(actor.start_turn().is_ok());
}

#[test]
fn issue_1601_native_websocket_requires_versioned_subprotocol() {
    let mut headers = axum::http::HeaderMap::new();
    assert!(require_native_websocket_protocol(&headers).is_err());
    headers.insert(
        axum::http::header::SEC_WEBSOCKET_PROTOCOL,
        "other, 1flowbase.native.v1".parse().unwrap(),
    );
    assert!(require_native_websocket_protocol(&headers).is_ok());
}
