use super::*;
use control_plane::ports::{LlmInputRepresentation, LlmInputSelection};

fn select(
    projector: &mut ResponsesWebSocketProjector,
    run: &NativeRunResult,
    sequence: i64,
    node_id: &str,
    node_run_id: Uuid,
    representation: LlmInputRepresentation,
) {
    assert!(projector
        .project(
            run,
            RuntimeEventEnvelope::new(
                run.id,
                sequence,
                LlmInputSelection {
                    node_id: node_id.into(),
                    node_run_id,
                    representation,
                }
                .runtime_event(),
            ),
        )
        .unwrap()
        .is_empty());
}

fn waiting_for_node(
    run: &NativeRunResult,
    sequence: i64,
    node_id: &str,
    node_run_id: Uuid,
) -> RuntimeEventEnvelope {
    let mut event = waiting_tool(run, sequence);
    event.node_run_id = Some(node_run_id);
    event.payload["node_run_id"] = json!(node_run_id);
    event.payload["node_id"] = json!(node_id);
    event
}

fn preferred_projector() -> ResponsesWebSocketProjector {
    ResponsesWebSocketProjector::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::AwaitingHostSelection,
    )
}

#[test]
fn same_node_native_to_semantic_callback_projects_function_call() {
    let mut run = native_run(2100);
    run.status = NativeRunStatus::Waiting;
    let node = Uuid::now_v7();
    let mut projector = preferred_projector();
    select(
        &mut projector,
        &run,
        1,
        "llm",
        node,
        LlmInputRepresentation::NativeOpaque,
    );
    select(
        &mut projector,
        &run,
        2,
        "llm",
        node,
        LlmInputRepresentation::SemanticMapped,
    );
    let frames = decoded(
        projector
            .project(&run, waiting_for_node(&run, 3, "llm", node))
            .unwrap(),
    );
    let output = &frames.last().unwrap()["response"]["output"];
    assert_eq!(frames.last().unwrap()["type"], "response.completed");
    assert_eq!(output.as_array().unwrap().len(), 1);
    assert_eq!(output[0]["type"], "function_call");
    assert_eq!(output[0]["call_id"], "call_weather");
    assert_eq!(output[0]["name"], "weather");
}

#[test]
fn same_node_semantic_to_native_callback_preserves_one_formal_item() {
    for mode in [
        ResponsesProjectionMode::AwaitingHostSelection,
        ResponsesProjectionMode::TransparentProviderResponses,
    ] {
        let mut run = native_run(2101);
        run.status = NativeRunStatus::Waiting;
        let node = Uuid::now_v7();
        let mut projector = ResponsesWebSocketProjector::with_mode("fixture".into(), None, mode);
        select(
            &mut projector,
            &run,
            1,
            "llm",
            node,
            LlmInputRepresentation::SemanticMapped,
        );
        select(
            &mut projector,
            &run,
            2,
            "llm",
            node,
            LlmInputRepresentation::NativeOpaque,
        );
        let item = json!({"type":"function_call","id":"fc_native_original","call_id":"call_native_original","name":"native_weather","arguments":"{}","status":"completed"});
        let mut frames = decoded(
            projector
                .project(
                    &run,
                    RuntimeEventEnvelope::new(
                        run.id,
                        3,
                        committed_provider_output_item_done("llm", node, 0, item.clone()),
                    ),
                )
                .unwrap(),
        );
        frames.extend(decoded(
            projector
                .project(&run, waiting_for_node(&run, 4, "llm", node))
                .unwrap(),
        ));
        assert_eq!(frames.last().unwrap()["type"], "response.completed");
        assert_eq!(frames.last().unwrap()["response"]["output"], json!([item]));
        assert_eq!(
            frames
                .iter()
                .filter(|event| event["type"] == "response.output_item.done")
                .count(),
            1
        );
    }
}

#[test]
fn another_native_node_does_not_suppress_semantic_callback() {
    let mut run = native_run(2102);
    run.status = NativeRunStatus::Waiting;
    let native_node = Uuid::now_v7();
    let semantic_node = Uuid::now_v7();
    let mut projector = preferred_projector();
    select(
        &mut projector,
        &run,
        1,
        "native",
        native_node,
        LlmInputRepresentation::NativeOpaque,
    );
    select(
        &mut projector,
        &run,
        2,
        "semantic",
        semantic_node,
        LlmInputRepresentation::SemanticMapped,
    );
    let frames = decoded(
        projector
            .project(&run, waiting_for_node(&run, 3, "semantic", semantic_node))
            .unwrap(),
    );
    let output = &frames.last().unwrap()["response"]["output"];
    assert_eq!(frames.last().unwrap()["type"], "response.completed");
    assert_eq!(output.as_array().unwrap().len(), 1);
    assert_eq!(output[0]["type"], "function_call");
    assert_eq!(output[0]["call_id"], "call_weather");
}

#[test]
fn static_presentation_resumes_after_current_selection_becomes_semantic() {
    let mut run = native_run(2103);
    run.status = NativeRunStatus::Succeeded;
    let node = Uuid::now_v7();
    let mut projector = preferred_projector();
    select(
        &mut projector,
        &run,
        1,
        "llm",
        node,
        LlmInputRepresentation::NativeOpaque,
    );
    select(
        &mut projector,
        &run,
        2,
        "llm",
        node,
        LlmInputRepresentation::SemanticMapped,
    );
    let mut frames = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    3,
                    debug_stream_events::answer_text_delta(
                        "answer",
                        "static marker".into(),
                        0,
                        None,
                        None,
                        None,
                    ),
                ),
            )
            .unwrap(),
    );
    frames.extend(decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    4,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            )
            .unwrap(),
    ));
    let deltas = frames
        .iter()
        .filter(|event| event["type"] == "response.output_text.delta")
        .collect::<Vec<_>>();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0]["delta"], "static marker");
    let output = &frames.last().unwrap()["response"]["output"];
    assert_eq!(output.as_array().unwrap().len(), 1);
    assert_eq!(output[0]["content"][0]["text"], "static marker");
}
