mod native_order;
use super::*;
use control_plane_contracts::ports::{
    AgentLogEventKind as Kind, AgentLogsBatch, AGENT_LOGS_SCHEMA_VERSION,
};
fn run() -> domain::FlowRunRecord {
    let id = Uuid::from_u128(1);
    let time = OffsetDateTime::parse("2026-10-07T08:00:00Z", &Rfc3339).unwrap();
    domain::FlowRunRecord {
        id,
        application_id: Uuid::from_u128(2),
        flow_id: Uuid::from_u128(3),
        draft_id: Uuid::from_u128(4),
        compiled_plan_id: None,
        debug_session_id: "native-source".into(),
        flow_schema_version: "v1".into(),
        document_hash: "original".into(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "source title".into(),
        status: domain::FlowRunStatus::Running,
        input_payload: json!({"prompt":"complete user text"}),
        output_payload: Value::Null,
        error_payload: None,
        created_by: Uuid::from_u128(5),
        authorized_account: None,
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: Some("conversation".into()),
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: time,
        finished_at: None,
        created_at: time,
        updated_at: time,
    }
}
#[test]
fn native_standard_exchange_preserves_full_units_tools_and_stable_identities_without_terminal_guess(
) {
    let run = run();
    let full = "long original body\0\n".repeat(5000);
    let archive = json!({"flow_run_fact":run,"canonical_contents":[{"content":{"original":full}}],"export_warnings":[]});
    let messages = vec![
        json!({"id":"system","display_sequence":-1000000000,"native_message":{"role":"system","content":"real system"}}),
        json!({"id":"user","display_sequence":0,"native_message":{"role":"user","content":"user text"}}),
        json!({"id":"call","display_sequence":1,"native_message":{"role":"assistant","content":"short summary","_source_item":{"type":"function_call","call_id":"call-1","name":"tool","arguments":full}}}),
        json!({"id":"result","display_sequence":2,"native_message":{"role":"tool","_source_item":{"type":"function_call_output","call_id":"call-1","output":"full tool result"}}}),
        json!({"id":"answer","display_sequence":3,"native_message":{"role":"assistant","_source_item":{"type":"message","phase":"final_answer","content":[{"type":"output_text","text":"full answer"}]}}}),
    ];
    let events = native::normalize(
        &run,
        "conversation",
        archive.clone(),
        messages.clone(),
        vec![],
    )
    .unwrap();
    assert_eq!(events.len(), 6);
    assert_eq!(events[1].kind, Kind::System);
    assert_eq!(events[2].kind, Kind::User);
    assert_eq!(events[3].kind, Kind::ToolCall);
    assert_eq!(events[3].content.as_deref(), Some(full.as_str()));
    assert_eq!(events[4].kind, Kind::ToolResult);
    assert_eq!(events[3].call_id, events[4].call_id);
    assert_eq!(events[5].phase.as_deref(), Some("final_answer"));
    assert!(events.iter().all(|event| event.kind != Kind::TaskEnd));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.raw.get("canonical_contents").is_some())
            .count(),
        1
    );
    let make_batch = |mut events: Vec<control_plane_contracts::ports::AgentLogEvent>| {
        events.sort_by(|left, right| {
            (left.sequence, &left.event_id).cmp(&(right.sequence, &right.event_id))
        });
        AgentLogsBatch {
            schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
            source_id: "native-source".into(),
            source_client: "1flowbase".into(),
            events,
        }
    };
    let batch = make_batch(events);
    let first = protocol::batches_to_traces(std::slice::from_ref(&batch)).unwrap();
    let second = protocol::batches_to_traces(&[make_batch(
        native::normalize(&run, "conversation", archive, messages, vec![]).unwrap(),
    )])
    .unwrap();
    assert_eq!(
        first, second,
        "unchanged source export must have stable IDs and payloads"
    );
    assert_eq!(
        serde_json::to_value(protocol::traces_to_batches(&first).unwrap()).unwrap(),
        serde_json::to_value([batch]).unwrap()
    );
    assert!(first
        .resource_spans
        .iter()
        .flat_map(|r| &r.scope_spans)
        .flat_map(|s| &s.spans)
        .flat_map(|s| &s.attributes)
        .all(|a| !a.key.starts_with("langfuse.")));
}
#[test]
fn native_conflicting_and_missing_facts_are_explicit_without_success_or_final_answer_inference() {
    let mut run = run();
    run.status = domain::FlowRunStatus::Failed;
    let events = native::normalize(&run,"session",json!({}),vec![json!({"id":"conflict","display_sequence":0,"native_message":{"role":"assistant","_log_conflicting":true,"_source_item":{"phase":"final_answer","content":"disputed"}}})],vec![]).unwrap();
    assert_eq!(events[1].kind, Kind::Context);
    assert_eq!(events[1].phase.as_deref(), Some("source_conflict"));
    assert_eq!(
        events[0].raw["exchange_diagnostics"],
        json!(["terminal_finished_at_missing"])
    );
    assert!(events.iter().all(|event| event.kind != Kind::TaskEnd));
    assert!(native::normalize(
        &run,
        "session",
        json!({}),
        vec![json!({"id":"missing","display_sequence":0,"native_message":null})],
        vec![]
    )
    .is_err());
    run.finished_at = Some(run.started_at + time::Duration::seconds(1));
    let events = native::normalize(&run, "session", json!({}), vec![], vec![]).unwrap();
    assert_eq!(events.last().unwrap().kind, Kind::TaskEnd);
    assert_eq!(events.last().unwrap().phase.as_deref(), Some("failed"));
    assert_eq!(
        events[0].raw["exchange_diagnostics"],
        json!(["canonical_messages_missing"])
    );
    run.status = domain::FlowRunStatus::Succeeded;
    let completed = native::normalize(&run, "session", json!({}), vec![], vec![]).unwrap();
    assert_eq!(
        completed.last().unwrap().phase.as_deref(),
        Some("completed")
    );
    assert_eq!(completed.last().unwrap().raw["status"], "succeeded");
}
#[test]
fn source_date_selection_is_started_at_inclusive_exclusive_and_rejects_invalid_ranges() {
    let from = "2026-10-01T00:00:00Z";
    let to = "2026-11-01T00:00:00Z";
    assert_eq!(
        date_filter(Some(from.into()), Some(to.into())).unwrap(),
        Filter::All(vec![
            Filter::Field {
                field: "started_at".into(),
                operator: Operator::Gte,
                value: json!(from)
            },
            Filter::Field {
                field: "started_at".into(),
                operator: Operator::Lt,
                value: json!(to)
            }
        ])
    );
    assert!(date_filter(Some(to.into()), Some(from.into())).is_err());
    assert!(date_filter(Some(from.into()), Some(from.into())).is_err());
    assert!(date_filter(Some("yesterday".into()), None).is_err());
}
