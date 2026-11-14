use super::*;

#[test]
fn openlit_history_tool_and_session_summary_preserve_source_without_invented_terminal() {
    let request = traces_fixture();
    let batches = traces_to_batches(&request).unwrap();
    assert_eq!(batches.len(), 2); // coding_agent.client=codex and SDK scope=openlit.
    let events: Vec<_> = batches.iter().flat_map(|batch| &batch.events).collect();
    assert!(events
        .iter()
        .any(|event| event.source_session_id == "session-one"));
    assert!(events
        .iter()
        .any(|event| event.source_session_id == "session-two"));
    assert!(events
        .iter()
        .all(|event| event.phase.is_none() && event.kind != AgentLogEventKind::TaskEnd));
    assert!(events
        .iter()
        .any(|event| event.kind == AgentLogEventKind::Context
            && event.content.as_deref() == Some("historical question")));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == AgentLogEventKind::User)
            .count(),
        1
    );
    let summary: Vec<_> = events
        .iter()
        .filter(|event| {
            event.raw.pointer("/otlp/span/name").and_then(Value::as_str)
                == Some("coding_agent.session")
        })
        .collect();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].kind, AgentLogEventKind::Context);
    assert!(summary[0].usage.is_none() && summary[0].content.is_none());
    let usage: Vec<_> = events
        .iter()
        .filter_map(|event| event.usage.as_ref())
        .collect();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].basis, AgentLogUsageBasis::Cumulative);
    assert_eq!(usage[0].total_tokens, Some(20));
    assert_eq!(usage[0].input_cache_hit_tokens, Some(2));
    assert_eq!(usage[0].cache_write_tokens, Some(3));
    assert_eq!(
        usage[0].response_id.as_deref(),
        Some("112233445566778899aabbccddeeff00/1112131415161718")
    );
    assert!(events
        .iter()
        .any(|event| event.kind == AgentLogEventKind::ToolCall
            && event.name.as_deref() == Some("weather")
            && event.call_id.as_deref() == Some("call-weather")));
    assert!(events
        .iter()
        .any(|event| event.kind == AgentLogEventKind::ToolResult
            && event.content.as_deref() == Some("{\"temp\":20}")));
    assert_eq!(
        summary[0]
            .raw
            .pointer("/otlp/resource/attributes/1/value/stringValue")
            .and_then(Value::as_str),
        Some("original-resource")
    );
    assert_eq!(
        summary[0]
            .raw
            .pointer("/otlp/scope/version")
            .and_then(Value::as_str),
        Some("source-version")
    );
}

#[test]
fn duplicate_snapshot_is_stable_and_updated_snapshot_has_cumulative_execution_identity() {
    let original = traces_fixture();
    let expected = traces_to_batches(&original).unwrap();
    assert_eq!(
        serde_json::to_value(traces_to_batches(&original).unwrap()).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let mut duplicate = original.clone();
    let snapshot = duplicate.resource_spans[0].scope_spans[0].spans[1].clone();
    duplicate.resource_spans[0].scope_spans[0]
        .spans
        .push(snapshot);
    assert_eq!(
        serde_json::to_value(traces_to_batches(&duplicate).unwrap()).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let mut updated = original.clone();
    first_span(&mut updated).end_time_unix_nano += 10;
    string_attr(first_span(&mut updated), "gen_ai.usage.input_tokens", "14");
    // The TypeScript SDK uses the alternate cache_* semantic convention keys.
    for attr in &mut first_span(&mut updated).attributes {
        if attr.key == "gen_ai.usage.cache.read_input_tokens" {
            attr.key = "gen_ai.usage.cache_read.input_tokens".into();
        }
        if attr.key == "gen_ai.usage.cache.creation_input_tokens" {
            attr.key = "gen_ai.usage.cache_creation.input_tokens".into();
        }
    }
    let new = traces_to_batches(&updated).unwrap();
    let old = expected
        .iter()
        .flat_map(|batch| &batch.events)
        .find(|event| event.usage.is_some())
        .unwrap();
    let new = new
        .iter()
        .flat_map(|batch| &batch.events)
        .find(|event| event.usage.is_some())
        .unwrap();
    assert_ne!(old.event_id, new.event_id);
    assert_eq!(old.source_task_id, new.source_task_id);
    assert_eq!(
        old.usage.as_ref().unwrap().response_id,
        new.usage.as_ref().unwrap().response_id
    );
    assert_eq!(
        new.usage.as_ref().unwrap().basis,
        AgentLogUsageBasis::Cumulative
    );
    assert!(new.sequence > old.sequence);
    assert_eq!(new.usage.as_ref().unwrap().total_tokens, Some(21));
    assert_eq!(new.usage.as_ref().unwrap().input_cache_hit_tokens, Some(2));
    assert_eq!(new.usage.as_ref().unwrap().cache_write_tokens, Some(3));
}
