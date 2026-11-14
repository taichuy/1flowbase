use super::*;

fn canonical_messages() -> Vec<Value> {
    // Real MD5(conversation:source_key) identities from QA1's counterexample.
    // UUID order is answer, result, call, prompt, opposite the source order.
    [
        (0, "c59f4c66-1268-6f3d-5edc-e03c1bf3a92a", "prompt:00000000-0000-0000-0000-000000000001", json!({"role":"user","content":"prompt"})),
        (1, "a8ac9109-c1e1-4ee4-e3f9-add15b817a3a", "output:call-1", json!({"role":"assistant","_source_item":{"type":"function_call","call_id":"tool-1","name":"tool","arguments":"exact arguments"}})),
        (2, "90025379-0f39-cc88-2760-2f6edbd2a04a", "result:tool-1", json!({"role":"tool","_source_item":{"type":"function_call_output","call_id":"tool-1","output":"exact result"}})),
        (3, "7ce7b484-03eb-f242-b050-18fd91ad1da6", "output:answer", json!({"role":"assistant","_source_item":{"type":"message","phase":"final_answer","content":"answer"}})),
    ].into_iter().map(|(sequence,id,key,message)| json!({"display_sequence":sequence,"id":id,"source_item_key":key,"started_at":"2026-10-07T08:00:00Z","native_message":message})).collect()
}
fn encode(
    events: Vec<control_plane_contracts::ports::AgentLogEvent>,
) -> protocol::ExportTraceServiceRequest {
    protocol::batches_to_traces(&[AgentLogsBatch {
        schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
        source_id: "native-source".into(),
        source_client: "1flowbase".into(),
        events,
    }])
    .unwrap()
}
#[test]
fn native_equal_timestamp_md5_ids_preserve_canonical_order_and_call_result_on_otlp_roundtrip() {
    let run = run();
    let messages = canonical_messages();
    let mut id_order = messages.clone();
    id_order.sort_by_key(|unit| unit["id"].as_str().unwrap().to_owned());
    assert_ne!(
        id_order, messages,
        "adversarial fixture must reject UUID surrogate ordering"
    );
    // Deliberately supply UUID order: normalization must use the retained sequence.
    let facts = native::normalize(&run, "conversation", json!({}), id_order, vec![]).unwrap();
    assert_eq!(
        facts.iter().map(|fact| fact.sequence).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    assert_eq!(
        facts[1..]
            .iter()
            .map(|fact| fact.raw["display_sequence"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(facts
        .iter()
        .all(|fact| fact.occurred_at == "2026-10-07T08:00:00Z"));
    assert_eq!(facts[2].kind, Kind::ToolCall);
    assert_eq!(facts[3].kind, Kind::ToolResult);
    assert_eq!(facts[2].call_id, facts[3].call_id);
    let first = encode(facts.clone());
    assert_eq!(
        first,
        encode(native::normalize(&run, "conversation", json!({}), messages, vec![]).unwrap())
    );
    let restored = protocol::traces_to_batches(&first).unwrap();
    assert_eq!(
        serde_json::to_value(&restored[0].events).unwrap(),
        serde_json::to_value(facts).unwrap()
    );
}
#[test]
fn native_corrected_source_sequence_and_occurrence_time_get_new_immutable_versions() {
    let run = run();
    let messages = canonical_messages();
    let original =
        native::normalize(&run, "conversation", json!({}), messages.clone(), vec![]).unwrap();
    let mut corrected_order = messages.clone();
    // Preserve relative order but change an actual source sequence, not a hash/time surrogate.
    corrected_order[3]["display_sequence"] = json!(i64::MAX);
    let changed = native::normalize(
        &run,
        "conversation",
        json!({}),
        corrected_order.clone(),
        vec![],
    )
    .unwrap();
    assert_eq!(original[4].sequence, changed[4].sequence);
    assert_ne!(original[4].event_id, changed[4].event_id);
    assert_eq!(
        encode(changed),
        encode(
            native::normalize(&run, "conversation", json!({}), corrected_order, vec![]).unwrap()
        )
    );
    let mut corrected_time = messages;
    corrected_time[3]["started_at"] = json!("2026-10-07T08:00:01Z");
    let changed =
        native::normalize(&run, "conversation", json!({}), corrected_time, vec![]).unwrap();
    assert_eq!(original[4].sequence, changed[4].sequence);
    assert_ne!(original[4].occurred_at, changed[4].occurred_at);
    assert_ne!(original[4].event_id, changed[4].event_id);
}
#[test]
fn native_corrected_finished_at_versions_terminal_without_changing_status_or_body() {
    let mut run = run();
    run.status = domain::FlowRunStatus::Succeeded;
    run.finished_at = Some(run.started_at + time::Duration::seconds(1));
    let first = native::normalize(&run, "conversation", json!({}), vec![], vec![]).unwrap();
    run.finished_at = Some(run.started_at + time::Duration::seconds(2));
    let second = native::normalize(&run, "conversation", json!({}), vec![], vec![]).unwrap();
    let before = first.last().unwrap();
    let after = second.last().unwrap();
    assert_eq!(before.kind, Kind::TaskEnd);
    assert_eq!(before.raw, after.raw);
    assert_eq!(before.phase, after.phase);
    assert_eq!(before.sequence, after.sequence);
    assert_ne!(before.occurred_at, after.occurred_at);
    assert_ne!(before.event_id, after.event_id);
    assert_ne!(encode(first), encode(second.clone()));
    assert_eq!(
        encode(second),
        encode(native::normalize(&run, "conversation", json!({}), vec![], vec![]).unwrap())
    );
}
#[test]
fn native_message_order_rejects_missing_or_conflicting_source_sequences() {
    let run = run();
    let mut missing = canonical_messages();
    missing[0]
        .as_object_mut()
        .unwrap()
        .remove("display_sequence");
    assert!(native::normalize(&run, "conversation", json!({}), missing, vec![]).is_err());
    let mut conflicting = canonical_messages();
    conflicting[1]["display_sequence"] = conflicting[0]["display_sequence"].clone();
    assert!(native::normalize(&run, "conversation", json!({}), conflicting, vec![]).is_err());
}

#[test]
fn native_multiple_task_roundtrip_preserves_each_source_order_times_and_parent_relation() {
    let parent = run();
    let mut child = parent.clone();
    child.id = Uuid::from_u128(11);
    child.started_at = parent.started_at + time::Duration::seconds(1);
    let mut child_messages = canonical_messages();
    for message in &mut child_messages {
        message["started_at"] = json!("2026-10-07T08:00:01Z");
    }
    let mut events = native::normalize(
        &parent,
        "conversation",
        json!({}),
        canonical_messages(),
        vec![],
    )
    .unwrap();
    let mut child_events =
        native::normalize(&child, "conversation", json!({}), child_messages, vec![]).unwrap();
    for fact in &mut child_events {
        fact.parent_source_task_id = Some(parent.id.to_string());
        fact.event_id.push_str(&format!(":parent:{}", parent.id));
    }
    events.extend(child_events);
    let restored = protocol::traces_to_batches(&encode(events)).unwrap();
    for (task, time, parent_id) in [
        (&parent, "2026-10-07T08:00:00Z", None),
        (&child, "2026-10-07T08:00:01Z", Some(parent.id.to_string())),
    ] {
        let task_events: Vec<_> = restored[0]
            .events
            .iter()
            .filter(|fact| fact.source_task_id == task.id.to_string())
            .collect();
        assert_eq!(task_events.len(), 5);
        assert!(task_events
            .iter()
            .all(|fact| fact.occurred_at == time && fact.parent_source_task_id == parent_id));
        assert_eq!(
            task_events[1..]
                .iter()
                .map(|fact| fact.raw["display_sequence"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(task_events[2].kind, Kind::ToolCall);
        assert_eq!(task_events[3].kind, Kind::ToolResult);
        assert_eq!(task_events[2].call_id, task_events[3].call_id);
    }
}
