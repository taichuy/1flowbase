use super::*;

#[test]
fn failed_terminal_preserves_committed_output_and_absorbs_late_success() {
    let mut run = native_run(2085);
    let node = Uuid::now_v7();
    let item = json!({"id":"fc_committed","type":"function_call","call_id":"call_committed","name":"exec","arguments":"{}"});
    let mut projector = transparent_projector("fixture", None);
    let frames = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    1,
                    committed_provider_output_item_done("llm", node, 0, item.clone()),
                ),
            )
            .unwrap(),
    );
    assert_eq!(frames.len(), 1);
    run.status = NativeRunStatus::Failed;
    run.error = Some(NativeError {
        code: "provider_upstream_error".into(),
        message: "original provider failure".into(),
        details: json!({}),
    });
    let terminal = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    2,
                    debug_stream_events::flow_failed(run.id, json!({})),
                ),
            )
            .unwrap(),
    );
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["type"], "response.failed");
    assert_eq!(
        terminal[0]["response"]["error"]["message"],
        "original provider failure"
    );
    assert_eq!(terminal[0]["response"]["output"], json!([item]));
    assert!(projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                3,
                debug_stream_events::flow_finished(run.id, json!({})),
            )
        )
        .unwrap()
        .is_empty());
}

#[test]
fn responses_websocket_preserves_full_upstream_error_object() {
    for upstream in [
        json!({"message":"rejected\nnext line","type":"vendor_refusal","code":"unknown_vendor_code","param":null,"extra":{"opaque":[null,1]}}),
        json!({"message":null,"type":null,"code":null,"future":true}),
        json!({"extra":[]}),
        json!({"message":"","code":""}),
    ] {
        let mut run = native_run(2190);
        run.status = NativeRunStatus::Failed;
        run.error = Some(NativeError {
            code: "unknown_vendor_code".into(),
            message: "fallback".into(),
            details: json!({"upstream_error":upstream,"semantic_terminal":true}),
        });
        let mut projector = transparent_projector("fixture", None);
        let frames = decoded(
            projector
                .project(
                    &run,
                    RuntimeEventEnvelope::new(
                        run.id,
                        1,
                        debug_stream_events::flow_failed(run.id, json!({})),
                    ),
                )
                .unwrap(),
        );
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["response"]["error"], upstream);
    }
}

#[test]
fn preferred_websocket_uses_host_selection_without_duplicating_native_text() {
    use control_plane::ports::{LlmInputRepresentation, LlmInputSelection};
    for native in [false, true] {
        let mut run = native_run(0x20990000000000000000000000000099);
        run.status = NativeRunStatus::Succeeded;
        let node = Uuid::now_v7();
        let item = json!({"id":"msg_selected_ws","type":"message","role":"assistant","content":[{"type":"output_text","text":"selected ws marker"}]});
        let mut projector = ResponsesWebSocketProjector::with_mode(
            "fixture".into(),
            None,
            ResponsesProjectionMode::AwaitingHostSelection,
        );
        let mut facts = vec![
            debug_stream_events::flow_started(run.id),
            LlmInputSelection {
                node_id: "llm".into(),
                node_run_id: node,
                representation: if native {
                    LlmInputRepresentation::NativeOpaque
                } else {
                    LlmInputRepresentation::SemanticMapped
                },
            }
            .runtime_event(),
        ];
        if native {
            facts.push(debug_stream_events::provider_responses_output_delta(
                "llm",
                node,
                json!({"type":"response.output_text.delta","delta":"selected ws marker"}),
            ));
            facts.push(debug_stream_events::provider_output_item_added(
                "llm",
                node,
                0,
                item.clone(),
            ));
            facts.push(debug_stream_events::provider_output_item_done(
                "llm",
                node,
                0,
                item.clone(),
            ));
        }
        facts.push(debug_stream_events::answer_text_delta(
            "answer",
            "selected ws marker".into(),
            0,
            Some("llm"),
            Some(node),
            Some("text"),
        ));
        facts.push(debug_stream_events::flow_finished(run.id, json!({})));
        let mut frames = vec![];
        for (index, fact) in facts.into_iter().enumerate() {
            frames.extend(decoded(
                projector
                    .project(
                        &run,
                        RuntimeEventEnvelope::new(run.id, index as i64 + 1, fact),
                    )
                    .unwrap(),
            ));
        }
        let deltas = frames
            .iter()
            .filter(|event| event["type"] == "response.output_text.delta")
            .collect::<Vec<_>>();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0]["delta"], "selected ws marker");
        let output = &frames.last().unwrap()["response"]["output"];
        assert_eq!(output.as_array().unwrap().len(), 1);
        if native {
            assert_eq!(output, &json!([item]));
        } else {
            assert_eq!(output[0]["content"][0]["text"], "selected ws marker");
        }
        assert!(projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    100,
                    debug_stream_events::flow_finished(run.id, json!({}))
                )
            )
            .unwrap()
            .is_empty());
    }
}

#[test]
fn preferred_projection_rejects_forged_and_unrelated_selection_facts() {
    use crate::routes::application_public_api::compat_sse::ResponsesProjectionSelector;
    use control_plane::ports::{LlmInputRepresentation, LlmInputSelection};
    let run = Uuid::now_v7();
    let node = Uuid::now_v7();
    let selection = LlmInputSelection {
        node_id: "llm".into(),
        node_run_id: node,
        representation: LlmInputRepresentation::SemanticMapped,
    };
    let answer = RuntimeEventEnvelope::new(
        run,
        3,
        debug_stream_events::answer_text_delta(
            "answer",
            "marker".into(),
            0,
            Some("llm"),
            Some(node),
            Some("text"),
        ),
    );
    let mut selector =
        ResponsesProjectionSelector::new(ResponsesProjectionMode::AwaitingHostSelection);
    assert!(!selector.semantic_event(&answer));
    let mut forged = RuntimeEventEnvelope::new(run, 1, selection.runtime_event());
    forged.source = RuntimeEventSource::Provider;
    selector.observe(run, &forged);
    assert!(!selector.semantic_event(&answer));
    forged.source = RuntimeEventSource::Runtime;
    forged.node_run_id = Some(Uuid::now_v7());
    selector.observe(run, &forged);
    assert!(!selector.semantic_event(&answer));
    selector.observe(
        run,
        &RuntimeEventEnvelope::new(Uuid::now_v7(), 2, selection.runtime_event()),
    );
    assert!(!selector.semantic_event(&answer));
    selector.observe(
        run,
        &RuntimeEventEnvelope::new(run, 2, selection.runtime_event()),
    );
    assert!(selector.semantic_event(&answer));
    let another = RuntimeEventEnvelope::new(
        run,
        4,
        debug_stream_events::answer_text_delta(
            "answer",
            "other".into(),
            0,
            Some("llm"),
            Some(Uuid::now_v7()),
            Some("text"),
        ),
    );
    assert!(!selector.semantic_event(&another));
    let native_node = Uuid::now_v7();
    selector.observe(
        run,
        &RuntimeEventEnvelope::new(
            run,
            5,
            LlmInputSelection {
                node_id: "other".into(),
                node_run_id: native_node,
                representation: LlmInputRepresentation::NativeOpaque,
            }
            .runtime_event(),
        ),
    );
    assert!(
        selector.semantic_event(&answer),
        "another node's native selection suppresses semantic output"
    );
    assert!(!selector.native_event(&RuntimeEventEnvelope::new(
        run,
        6,
        debug_stream_events::provider_output_item_done("llm", node, 0, json!({"type":"message"}))
    )));
}

#[test]
fn cold_selection_replay_requires_host_fact_without_changing_other_event_sources() {
    use crate::routes::application_public_api::stream_terminal_fallback::durable_record_to_runtime_event_envelope;
    use control_plane::ports::{LlmInputRepresentation, LlmInputSelection};
    let node = Uuid::now_v7();
    let selection = LlmInputSelection {
        node_id: "llm".into(),
        node_run_id: node,
        representation: LlmInputRepresentation::NativeOpaque,
    };
    for (source, trust, accepted) in [
        (
            domain::RuntimeEventSource::Host,
            domain::RuntimeTrustLevel::HostFact,
            true,
        ),
        (
            domain::RuntimeEventSource::ProviderPlugin,
            domain::RuntimeTrustLevel::HostFact,
            false,
        ),
        (
            domain::RuntimeEventSource::ExternalAgent,
            domain::RuntimeTrustLevel::HostFact,
            false,
        ),
        (
            domain::RuntimeEventSource::Host,
            domain::RuntimeTrustLevel::AgentReported,
            false,
        ),
    ] {
        let record = domain::RuntimeEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::now_v7(),
            node_run_id: Some(node),
            span_id: None,
            parent_span_id: None,
            sequence: 1,
            event_type: LlmInputSelection::EVENT_TYPE.into(),
            layer: domain::RuntimeEventLayer::RuntimeItem,
            source,
            trust_level: trust,
            item_id: None,
            ledger_ref: None,
            payload: selection.runtime_event().payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
            created_at: OffsetDateTime::now_utc(),
        };
        let envelope = durable_record_to_runtime_event_envelope(record.clone());
        assert_eq!(
            LlmInputSelection::from_host_event(&envelope).is_some(),
            accepted
        );
        let mut ordinary = record;
        ordinary.event_type = "node_started".into();
        let ordinary = durable_record_to_runtime_event_envelope(ordinary);
        assert_eq!(
            ordinary.source,
            if source == domain::RuntimeEventSource::ProviderPlugin {
                RuntimeEventSource::Provider
            } else {
                RuntimeEventSource::Runtime
            }
        );
    }
}

#[test]
fn preferred_websocket_missing_selection_cannot_complete_empty() {
    let mut run = native_run(2099);
    run.status = NativeRunStatus::Succeeded;
    let mut projector = ResponsesWebSocketProjector::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::AwaitingHostSelection,
    );
    let frames = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    1,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            )
            .unwrap(),
    );
    assert_eq!(frames.last().unwrap()["type"], "response.failed");
    assert_eq!(
        frames.last().unwrap()["response"]["error"]["code"],
        "llm_input_selection_missing"
    );
}
