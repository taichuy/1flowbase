use crate::ports::*;
use serde_json::json;
#[test]
fn agent_logs_frozen_optional_usage_and_transport_contract() {
    let payload = json!({"schema_version":"1flowbase.agent-logs/v1","source_id":"collector","source_client":"codex","events":[{"event_id":"source-event","source_session_id":"session","source_task_id":"turn","parent_source_task_id":null,"sequence":0,"occurred_at":"2026-10-07T08:00:00Z","kind":"usage","content":null,"phase":null,"name":null,"call_id":null,"model_id":null,"provider_code":null,"usage":{"basis":"cumulative","response_id":null,"input_tokens":null,"output_tokens":null,"input_cache_hit_tokens":4,"cache_write_tokens":null,"total_tokens":20},"inherited":false,"raw":{}}]});
    let batch: AgentLogsBatch = serde_json::from_value(payload.clone()).unwrap();
    batch.validate().unwrap();
    assert_eq!(serde_json::to_value(batch).unwrap(), payload);
    assert_eq!(
        serde_json::to_value(ClientTrajectoryTransport::File).unwrap(),
        json!("file")
    );
    let bad = json!({"schema_version":"wrong","source_id":"collector","source_client":"codex","events":[]});
    assert!(serde_json::from_value::<AgentLogsBatch>(bad)
        .unwrap()
        .validate()
        .is_err());
}

#[test]
fn agent_logs_reasoning_effort_is_optional_without_changing_old_event_identity() {
    let payload = json!({"event_id":"e","source_session_id":"s","source_task_id":"t","parent_source_task_id":null,"sequence":0,"occurred_at":"2026-10-07T08:00:00Z","kind":"context","content":null,"phase":null,"name":null,"call_id":null,"model_id":"source-model","provider_code":null,"usage":null,"inherited":false,"raw":{}});
    let mut event: AgentLogEvent = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(event.reasoning_effort, None);
    assert_eq!(serde_json::to_value(&event).unwrap(), payload);
    event.reasoning_effort = Some("medium".into());
    let mut expected = payload;
    expected["reasoning_effort"] = json!("medium");
    assert_eq!(serde_json::to_value(&event).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<AgentLogEvent>(expected)
            .unwrap()
            .reasoning_effort
            .as_deref(),
        Some("medium")
    );
}
#[test]
fn optional_agent_usage_rejects_negative_counts_without_capacity_guesses() {
    let mut event:AgentLogEvent=serde_json::from_value(json!({"event_id":"e","source_session_id":"s","source_task_id":"t","sequence":1,"occurred_at":"2026-10-07T08:00:00Z","kind":"usage","inherited":false,"usage":{"basis":"delta","total_tokens":-1}})).unwrap();
    let mut batch = AgentLogsBatch {
        schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
        source_id: "s".repeat(1024),
        source_client: "codex".into(),
        events: vec![event.clone()],
    };
    assert!(batch.validate().is_err());
    event.usage.as_mut().unwrap().total_tokens = Some(0);
    batch.events = vec![event; 1001];
    assert!(batch.validate().is_ok());
}

#[test]
fn agent_logs_lossless_cursor_wire_preserves_native_numbers_and_imported_ties() {
    let id = uuid::Uuid::now_v7();
    let cursor = RecordClientTrajectoryCursor::imported(10, id);
    assert_eq!(
        serde_json::to_value(RecordClientTrajectoryCursor::Native(10)).unwrap(),
        json!(10)
    );
    let token = serde_json::to_value(cursor)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        RecordClientTrajectoryCursor::imported_position(&token).unwrap(),
        (10, id)
    );
    assert!(RecordClientTrajectoryCursor::imported_position("10").is_err());
    assert!(RecordClientTrajectoryCursor::imported_position(&format!("s1:10:{id}:extra")).is_err());
}
