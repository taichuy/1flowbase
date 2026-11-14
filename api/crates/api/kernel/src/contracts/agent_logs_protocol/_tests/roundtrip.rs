use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};

#[test]
fn self_export_is_one_observation_per_source_fact_with_stable_identity_and_parent_links() {
    let batches = source_batches();
    let request = batches_to_traces(&batches).unwrap();
    let spans = &request.resource_spans[0].scope_spans[0].spans;
    assert_eq!(spans.len(), batches[0].events.len());
    assert_eq!(spans[0].trace_id, spans[1].trace_id);
    assert_ne!(spans[0].trace_id, spans[3].trace_id);
    assert_eq!(spans[1].parent_span_id, spans[0].span_id);
    assert_eq!(spans[2].parent_span_id, spans[1].span_id);
    assert_eq!(batches_to_traces(&batches).unwrap(), request);
    let metadata = spans[0]
        .attributes
        .iter()
        .find(|attr| attr.key == "1flowbase.agent_log.source_fact")
        .unwrap();
    let Some(any_value::Value::StringValue(metadata)) = &metadata.value.as_ref().unwrap().value
    else {
        panic!("metadata JSON blob");
    };
    let encoded = metadata;
    let payload = encoded
        .strip_prefix("1flowbase.agent-logs.source-fact/v1;base64,")
        .unwrap();
    let fact: Value = serde_json::from_slice(&STANDARD.decode(payload).unwrap()).unwrap();
    assert_eq!(fact["schema_version"], AGENT_LOGS_SCHEMA_VERSION);
    assert_eq!(fact["event"]["raw"], batches[0].events[0].raw);
    for (mime, body) in [
        ("application/json", encode_json(&request).unwrap()),
        ("application/x-protobuf", encode_protobuf(&request)),
    ] {
        let imported =
            traces_to_batches(&decode(mime, Some("gzip"), &encode_gzip(&body).unwrap()).unwrap())
                .unwrap();
        assert_eq!(
            serde_json::to_value(&imported).unwrap(),
            serde_json::to_value(&batches).unwrap()
        );
    }
}

#[test]
fn legacy_nested_otlp_file_still_replays_all_original_source_facts() {
    let batches = source_batches();
    let mut request = batches_to_traces(&batches).unwrap();
    for (span, event) in request.resource_spans[0].scope_spans[0]
        .spans
        .iter_mut()
        .zip(&batches[0].events)
    {
        let legacy = json!({"1flowbase_agent_log":{"schema_version":batches[0].schema_version,"source_id":batches[0].source_id,"source_client":batches[0].source_client,"event":event}});
        span.attributes
            .retain(|attr| attr.key != "1flowbase.agent_log.source_fact");
        string_attr(span, "langfuse.observation.metadata", &legacy.to_string());
    }
    for (mime, body) in [
        ("application/json", encode_json(&request).unwrap()),
        ("application/x-protobuf", encode_protobuf(&request)),
    ] {
        let decoded = decode(mime, None, &body).unwrap();
        assert_eq!(
            serde_json::to_value(traces_to_batches(&decoded).unwrap()).unwrap(),
            serde_json::to_value(&batches).unwrap()
        );
    }
}

#[test]
fn malformed_self_metadata_is_rejected_without_generic_fallback() {
    for metadata in [
        r#"{"1flowbase_agent_log":null}"#,
        r#"{"1flowbase_agent_log":{"schema_version":"unknown"}}"#,
        r#"{"1flowbase_agent_log":"lost-source"}"#,
        r#"{"1flowbase_agent_log":broken"#,
    ] {
        let mut request = batches_to_traces(&source_batches()).unwrap();
        let span = &mut request.resource_spans[0].scope_spans[0].spans[0];
        span.attributes
            .retain(|attr| attr.key != "1flowbase.agent_log.source_fact");
        string_attr(span, "langfuse.observation.metadata", metadata);
        assert!(
            traces_to_batches(&request).is_err(),
            "must reject {metadata}"
        );
    }
    let mut request = batches_to_traces(&source_batches()).unwrap();
    let span = &mut request.resource_spans[0].scope_spans[0].spans[0];
    span.attributes
        .retain(|attr| attr.key != "1flowbase.agent_log.source_fact");
    let mut fact = json!({"schema_version": AGENT_LOGS_SCHEMA_VERSION,"source_id":"collector-a","source_client":"codex","event":source_batches()[0].events[0]});
    fact["event"]["source_task_id"] = Value::String(String::new());
    string_attr(
        span,
        "langfuse.observation.metadata",
        &json!({"1flowbase_agent_log":fact}).to_string(),
    );
    assert!(traces_to_batches(&request).is_err());
}

#[test]
fn standard_usage_matches_canonical_delta_precedence_and_cumulative_ownership() {
    let template = source_batches()[0].events[0].clone();
    let mut events = Vec::new();
    // First record: only deltas (3 + 7) count, not the superseding
    // cumulative snapshot, an inherited delta or an unowned cumulative.
    // Second record: the last owned cumulative (12) contributes once.
    for (id, task, sequence, basis, response, inherited, total) in [
        (
            "delta-a",
            "task-one",
            1,
            AgentLogUsageBasis::Delta,
            Some("response-one"),
            false,
            3,
        ),
        (
            "delta-b",
            "task-one",
            2,
            AgentLogUsageBasis::Delta,
            Some("response-one"),
            false,
            7,
        ),
        (
            "redundant-cumulative",
            "task-one",
            3,
            AgentLogUsageBasis::Cumulative,
            Some("response-one"),
            false,
            100,
        ),
        (
            "inherited",
            "task-one",
            4,
            AgentLogUsageBasis::Delta,
            Some("response-one"),
            true,
            999,
        ),
        (
            "unowned-cumulative",
            "task-one",
            5,
            AgentLogUsageBasis::Cumulative,
            None,
            false,
            999,
        ),
        (
            "old-cumulative",
            "task-two",
            1,
            AgentLogUsageBasis::Cumulative,
            Some("response-two"),
            false,
            5,
        ),
        (
            "last-cumulative",
            "task-two",
            2,
            AgentLogUsageBasis::Cumulative,
            Some("response-two"),
            false,
            12,
        ),
    ] {
        let mut event = template.clone();
        event.event_id = id.into();
        event.source_task_id = task.into();
        event.sequence = sequence;
        event.inherited = inherited;
        event.usage = Some(AgentLogUsage {
            basis,
            response_id: response.map(str::to_owned),
            input_tokens: Some(total),
            output_tokens: Some(0),
            total_tokens: Some(total),
            input_cache_hit_tokens: None,
            cache_write_tokens: None,
        });
        events.push(event);
    }
    let mut batch = source_batches().remove(0);
    batch.events = events;
    let request = batches_to_traces(&[batch.clone()]).unwrap();
    let spans = &request.resource_spans[0].scope_spans[0].spans;
    assert_eq!(spans.len(), 7);
    let contributions: Vec<i64> = spans
        .iter()
        .map(|span| {
            let Some(attr) = span
                .attributes
                .iter()
                .find(|attr| attr.key == "gen_ai.usage.total_tokens")
            else {
                assert!(span
                    .attributes
                    .iter()
                    .all(|attr| !attr.key.starts_with("gen_ai.usage.")));
                return 0;
            };
            let Some(any_value::Value::IntValue(total)) = &attr.value.as_ref().unwrap().value
            else {
                panic!("integer usage");
            };
            *total
        })
        .collect();
    assert_eq!(contributions, [3, 7, 0, 0, 0, 0, 12]);
    assert_eq!(contributions.iter().sum::<i64>(), 22);
    let mut expected = batch;
    expected.events.sort_by(|left, right| {
        (left.sequence, &left.event_id).cmp(&(right.sequence, &right.event_id))
    });
    assert_eq!(
        serde_json::to_value(traces_to_batches(&request).unwrap()).unwrap(),
        serde_json::to_value([expected]).unwrap()
    );
}

#[test]
fn neutral_capsule_rejects_malformed_ambiguous_or_flattened_authority() {
    let original = batches_to_traces(&source_batches()).unwrap();
    for malformed in [
        "",
        "{}",
        "1flowbase.agent-logs.source-fact/v1;base64,not-base64",
    ] {
        let mut request = original.clone();
        string_attr(
            &mut request.resource_spans[0].scope_spans[0].spans[0],
            "1flowbase.agent_log.source_fact",
            malformed,
        );
        assert!(traces_to_batches(&request).is_err());
    }
    let mut ambiguous = original.clone();
    string_attr(
        &mut ambiguous.resource_spans[0].scope_spans[0].spans[0],
        "langfuse.observation.metadata",
        "{}",
    );
    assert!(traces_to_batches(&ambiguous).is_err());
    let mut flattened = original;
    let span = &mut flattened.resource_spans[0].scope_spans[0].spans[0];
    span.attributes
        .retain(|attr| attr.key != "1flowbase.agent_log.source_fact");
    string_attr(
        span,
        "langfuse.observation.metadata",
        r#"{"1flowbase_agent_log.event":{"content":"ambiguous"}}"#,
    );
    assert!(traces_to_batches(&flattened).is_err());
}
