use super::*;
use serde_json::json;
use uuid::Uuid;

pub(super) fn batch_without_task_end() -> AgentLogsBatch {
    serde_json::from_value(json!({
        "schema_version":"1flowbase.agent-logs/v1",
        "source_id":"collector",
        "source_client":"codex",
        "events":[{
            "event_id":"event-1","source_session_id":"session-1","source_task_id":"task-1",
            "sequence":7,"occurred_at":"2026-11-07T01:00:00Z","kind":"assistant",
            "content":"business answer","phase":"analysis","model_id":"model-1",
            "reasoning_effort":"high","provider_code":"provider-1",
            "usage":{"basis":"delta","response_id":"response-1","input_tokens":5,"output_tokens":8},
            "raw":{"authorization":"RAW_SECRET","unregistered":{"blob":"opaque"}}
        }]
    }))
    .unwrap()
}

#[test]
fn agent_logs_event_projection_accepts_business_facts_without_task_end_and_excludes_raw() {
    let batch = batch_without_task_end();
    batch.validate().unwrap();
    let input = Input(batch);
    let value = input.project_for_event().unwrap();
    let validator = jsonschema::validator_for(&Input::event_projection_schema().unwrap()).unwrap();
    assert!(validator.is_valid(&value));
    assert_eq!(value["events"][0]["content"], "business answer");
    assert_eq!(value["events"][0]["kind"], "assistant");
    assert_eq!(value["events"][0]["usage"]["output_tokens"], 8);
    assert_eq!(value["source_id"], "collector");
    assert!(value["events"][0].get("raw").is_none());
    assert!(!value.to_string().contains("RAW_SECRET"));
    let mut leaked = value;
    leaked["events"][0]["raw"] = json!({"secret":"private"});
    assert!(!validator.is_valid(&leaked));
    assert_ne!(
        Input::event_projection_schema(),
        Input::managed_projection_schema()
    );
    assert_eq!(
        input.project_for_managed_hook(),
        Some(json!({"kind":Input::CONTRACT_ID}))
    );
}

#[test]
fn agent_logs_receipt_projection_preserves_duplicates_and_record_identity_without_task_status() {
    let record_id = Uuid::now_v7();
    for (accepted, duplicate) in [(2, 0), (0, 2), (1, 1)] {
        let output = Output(AgentLogsReceipt {
            accepted_events: accepted,
            duplicate_events: duplicate,
            record_ids: vec![record_id],
        });
        let projection = output.project_for_event().unwrap();
        assert_eq!(
            projection,
            json!({"accepted_events":accepted,"duplicate_events":duplicate,"record_ids":[record_id]})
        );
        let schema = Output::event_projection_schema().unwrap();
        assert!(jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&projection));
        assert!(projection.get("flow_run_id").is_none());
        assert!(projection.get("task_status").is_none());
    }
    assert_eq!(TargetError::event_projection_schema(), None);
    assert!(event_projection::OPERATIONS.contains(&"application.logs.events.ingest"));
    assert!(event_projection::OPERATIONS.contains(&"application.logs.otlp.traces.ingest"));
    assert!(event_projection::OPERATIONS.contains(&"applications.logs.otlp.import"));
    assert!(!event_projection::OPERATIONS.contains(&"applications.logs.otlp.export"));
}
