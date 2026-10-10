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

#[test]
fn agent_logs_delete_requires_explicit_mode_and_strict_rfc3339_range() {
    for bad in [
        json!({}),
        json!({"mode":"everything"}),
        json!({"mode":"all_time","started_at_from":"2026-10-07T00:00:00Z"}),
        json!({"mode":"time_range"}),
        json!({"mode":"time_range","started_at_from":"2026-10-07T00:00:00Z"}),
    ] {
        assert!(serde_json::from_value::<AgentLogsDeleteScope>(bad).is_err());
    }
    assert!(
        serde_json::from_value::<AgentLogsDeleteScope>(json!({"mode":"all_time"}))
            .unwrap()
            .bounds()
            .unwrap()
            .is_none()
    );
    for (from, to) in [
        ("bad", "2026-10-08T00:00:00Z"),
        ("2026-10-07", "2026-10-08T00:00:00Z"),
        ("2026-10-07T00:00:00Z", "2026-10-07T00:00:00Z"),
        ("2026-10-08T00:00:00Z", "2026-10-07T00:00:00Z"),
    ] {
        assert!(AgentLogsDeleteScope::TimeRange {
            started_at_from: from.into(),
            started_at_to: to.into(),
            batch_size: None,
            ingested_at_before: None,
        }
        .bounds()
        .is_err());
    }
    let (from, to) = AgentLogsDeleteScope::TimeRange {
        started_at_from: "2026-10-07T08:00:00+08:00".into(),
        started_at_to: "2026-10-08T00:00:00Z".into(),
        batch_size: None,
        ingested_at_before: None,
    }
    .bounds()
    .unwrap()
    .unwrap();
    assert!(from < to);
    assert_eq!(
        serde_json::to_value(AgentLogsDeleteReceipt {
            deleted_records: 3,
            has_more: false,
            ingested_at_before: "2026-10-09T00:00:00Z".into()
        })
        .unwrap(),
        json!({"deleted_records":3,"has_more":false,"ingested_at_before":"2026-10-09T00:00:00Z"})
    );
}

#[test]
fn agent_logs_delete_batch_size_and_ingestion_boundary_are_strict() {
    for size in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!("100"),
        json!(4294967296u64),
    ] {
        assert!(serde_json::from_value::<AgentLogsDeleteScope>(
            json!({"mode":"all_time","batch_size":size})
        )
        .is_err());
    }
    let scope: AgentLogsDeleteScope = serde_json::from_value(json!({"mode":"all_time","batch_size":100,"ingested_at_before":"2026-10-09T12:00:00+08:00"})).unwrap();
    assert_eq!(scope.batch_size().unwrap().get(), 100);
    assert!(scope.ingested_at_before().unwrap().is_some());
    let bad: AgentLogsDeleteScope =
        serde_json::from_value(json!({"mode":"all_time","ingested_at_before":"yesterday"}))
            .unwrap();
    assert!(bad.ingested_at_before().is_err());
}

#[test]
fn agent_logs_delete_job_state_machine_never_claims_uncommitted_or_stopped_completion() {
    assert_eq!(
        AgentLogsDeleteJobStatus::after_batch(0, 0, false),
        AgentLogsDeleteJobStatus::Succeeded
    );
    assert_eq!(
        AgentLogsDeleteJobStatus::after_batch(100, 205, false),
        AgentLogsDeleteJobStatus::Running
    );
    assert_eq!(
        AgentLogsDeleteJobStatus::after_batch(100, 205, true),
        AgentLogsDeleteJobStatus::Stopped
    );
    assert_eq!(
        AgentLogsDeleteJobStatus::after_batch(205, 205, true),
        AgentLogsDeleteJobStatus::Succeeded
    );
    for value in [
        json!({"scope":{"mode":"all_time"}}),
        json!({"job_id":"bad", "scope":{"mode":"all_time"}}),
        json!({"job_id":"00000000-0000-0000-0000-000000000001", "scope":{"mode":"all_time"}, "unexpected":true}),
    ] {
        assert!(serde_json::from_value::<AgentLogsDeleteJobCreate>(value).is_err());
    }
}
