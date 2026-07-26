use control_plane::{
    application_public_api::{
        native::{
            answer_segments_from_value, answer_segments_value, AnswerProjectionSegment,
            AnswerProjectionSegmentKind, NativeRunResult, NativeRunStatus, ANSWER_SEGMENTS_KEY,
        },
        run_service::{
            native_result_from_run_stream_state, ApplicationPublishedRunControlRepository,
        },
    },
    orchestration_runtime::{
        debug_artifacts::is_runtime_debug_artifact_preview, debug_stream_events,
        FinalizePublishedRunMissingStreamTerminalCommand, OrchestrationRuntimeService,
    },
    ports::{
        RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventSource,
    },
};
use serde_json::{json, Value};
use tracing::warn;

use crate::{app_state::ApiState, provider_runtime::ApiProviderRuntime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalAnswerDeltaKind {
    Reasoning,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalAnswerDelta {
    pub kind: TerminalAnswerDeltaKind,
    pub text: String,
}

/// A public terminal may only be projected from a durable run snapshot. A load
/// failure is therefore a barrier failure, not permission to reuse the stale
/// pre-execution snapshot.
pub(crate) async fn load_durable_native_run_for_terminal_projection(
    state: &ApiState,
    initial_run: &NativeRunResult,
) -> anyhow::Result<NativeRunResult> {
    load_latest_native_run_strict(state, initial_run).await
}

pub(crate) fn durable_native_run_matches_terminal(run: &NativeRunResult, event_type: &str) -> bool {
    matches!(
        (run.status, event_type),
        (NativeRunStatus::Succeeded, "flow_finished")
            | (NativeRunStatus::Incomplete, "flow_incomplete")
            | (NativeRunStatus::Failed, "flow_failed")
            | (NativeRunStatus::Cancelled, "flow_cancelled")
            | (
                NativeRunStatus::Waiting,
                "waiting_human" | "waiting_callback"
            )
    )
}

/// Resolves a confirmed producer EOF to the durable winner before an API adapter projects it.
/// Callers must use this only after execution has ended or a runtime stream closed without a
/// terminal event; transport failures and client disconnects are not EOF evidence.
pub(crate) async fn recover_missing_stream_terminal_winner(
    state: &ApiState,
    initial_run: &NativeRunResult,
) -> anyhow::Result<NativeRunResult> {
    let current = load_latest_native_run_strict(state, initial_run).await?;
    if !matches!(
        current.status,
        NativeRunStatus::Queued | NativeRunStatus::Running
    ) {
        return Ok(current);
    }

    let recovery_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let recovery_result = recovery_service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: initial_run.application_id,
                flow_run_id: initial_run.id,
            },
        )
        .await;

    // Live publication can fail after the durable transaction commits (for example a stream
    // already closed). The fresh durable winner, rather than that delivery error, is the source
    // of truth for the fallback projection.
    let winner = load_latest_native_run_strict(state, initial_run).await?;
    if !matches!(
        winner.status,
        NativeRunStatus::Queued | NativeRunStatus::Running
    ) {
        if let Err(error) = recovery_result {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                error = %error,
                "published stream EOF recovery committed a durable winner but could not publish its live terminal"
            );
        }
        return Ok(winner);
    }

    match recovery_result {
        Ok(_) => Err(anyhow::anyhow!(
            "published stream EOF recovery returned a nonterminal durable winner"
        )),
        Err(error) => Err(error),
    }
}

async fn load_latest_native_run_strict(
    state: &ApiState,
    initial_run: &NativeRunResult,
) -> anyhow::Result<NativeRunResult> {
    let stream_state = state
        .store
        .get_published_run_stream_state(initial_run.application_id, initial_run.id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("published run stream state not found"))?;
    Ok(native_result_from_run_stream_state(
        initial_run,
        &stream_state,
    ))
}

pub(crate) fn terminal_runtime_event_from_native_run(
    run: &NativeRunResult,
) -> Option<RuntimeEventEnvelope> {
    let payload = match run.status {
        NativeRunStatus::Succeeded => {
            debug_stream_events::flow_finished(run.id, terminal_output_payload(run))
        }
        NativeRunStatus::Incomplete => {
            debug_stream_events::flow_incomplete(run.id, terminal_output_payload(run))
        }
        NativeRunStatus::Failed => {
            debug_stream_events::flow_failed(run.id, terminal_error_payload(run))
        }
        NativeRunStatus::Cancelled => debug_stream_events::flow_cancelled(run.id),
        NativeRunStatus::Waiting => waiting_terminal_payload(run),
        NativeRunStatus::Created | NativeRunStatus::Queued | NativeRunStatus::Running => {
            return None;
        }
    };
    Some(RuntimeEventEnvelope::new(run.id, 0, payload))
}

pub(crate) fn terminal_answer_runtime_events_from_native_run(
    run: &NativeRunResult,
) -> Vec<RuntimeEventEnvelope> {
    terminal_answer_deltas_from_payload(&terminal_output_payload(run))
        .into_iter()
        .enumerate()
        .map(|(index, delta)| {
            let payload = match delta.kind {
                TerminalAnswerDeltaKind::Reasoning => debug_stream_events::answer_reasoning_delta(
                    "assistant",
                    delta.text,
                    index,
                    None,
                    None,
                    None,
                ),
                TerminalAnswerDeltaKind::Text => debug_stream_events::answer_text_delta(
                    "assistant",
                    delta.text,
                    index,
                    None,
                    None,
                    None,
                ),
            };
            RuntimeEventEnvelope::new(run.id, index as i64 + 1, payload)
        })
        .collect()
}

fn terminal_output_payload(run: &NativeRunResult) -> Value {
    let mut payload = json!({
        "answer": run.answer,
        "tool_calls": run.tool_calls,
        "usage": run.usage,
    });
    if let (Some(output), Some(answer_segments)) = (
        payload.as_object_mut(),
        run.answer_segments
            .as_deref()
            .and_then(answer_segments_value),
    ) {
        output.insert(ANSWER_SEGMENTS_KEY.to_string(), answer_segments);
    }
    payload
}

fn terminal_error_payload(run: &NativeRunResult) -> Value {
    run.error
        .as_ref()
        .and_then(|error| serde_json::to_value(error).ok())
        .unwrap_or_else(|| json!({ "message": "published run failed" }))
}

fn waiting_terminal_payload(run: &NativeRunResult) -> RuntimeEventPayload {
    let Some(action) = run.required_action.as_ref() else {
        return RuntimeEventPayload {
            event_type: "waiting_human".to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({
                "type": "waiting_human",
                "run_id": run.id,
                "status": "waiting_human",
            }),
        };
    };
    let Some(callback_task_id) = action
        .payload
        .get("callback_task_id")
        .cloned()
        .filter(|value| !value.is_null())
    else {
        return RuntimeEventPayload {
            event_type: "waiting_human".to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({
                "type": "waiting_human",
                "run_id": run.id,
                "status": "waiting_human",
                "required_action": action,
            }),
        };
    };
    let callback_kind = action
        .payload
        .get("callback_kind")
        .cloned()
        .unwrap_or(Value::Null);
    RuntimeEventPayload {
        event_type: "waiting_callback".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "waiting_callback",
            "run_id": run.id,
            "status": "waiting_callback",
            "callback_task_id": callback_task_id,
            "callback_kind": callback_kind,
            "node_run_id": action
                .payload
                .get("node_run_id")
                .cloned()
                .unwrap_or(Value::Null),
            "required_action": action,
        }),
    }
}

pub(crate) fn terminal_answer_deltas_from_payload(payload: &Value) -> Vec<TerminalAnswerDelta> {
    let structured_deltas = terminal_answer_segments_from_payload(payload)
        .into_iter()
        .map(terminal_answer_delta_from_segment)
        .collect::<Vec<_>>();
    if !structured_deltas.is_empty() {
        return structured_deltas;
    }

    terminal_answer_text_from_payload(payload)
        .as_deref()
        .map(split_terminal_answer_deltas)
        .unwrap_or_default()
}

fn terminal_answer_segments_from_payload(payload: &Value) -> Vec<AnswerProjectionSegment> {
    payload
        .get("output")
        .and_then(|output| output.get(ANSWER_SEGMENTS_KEY))
        .map(answer_segments_from_value)
        .filter(|segments| !segments.is_empty())
        .or_else(|| {
            payload
                .get(ANSWER_SEGMENTS_KEY)
                .map(answer_segments_from_value)
                .filter(|segments| !segments.is_empty())
        })
        .unwrap_or_default()
}

fn terminal_answer_delta_from_segment(segment: AnswerProjectionSegment) -> TerminalAnswerDelta {
    TerminalAnswerDelta {
        kind: match segment.kind {
            AnswerProjectionSegmentKind::Reasoning => TerminalAnswerDeltaKind::Reasoning,
            AnswerProjectionSegmentKind::Message => TerminalAnswerDeltaKind::Text,
        },
        text: segment.text,
    }
}

pub(crate) fn terminal_answer_text_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("output")
        .and_then(|output| output.get("answer"))
        .and_then(|value| terminal_answer_text_from_value(value, 0))
        .or_else(|| {
            payload
                .get("answer")
                .and_then(|value| terminal_answer_text_from_value(value, 0))
        })
        .or_else(|| {
            payload
                .get("output")
                .and_then(|value| terminal_answer_text_from_value(value, 0))
        })
}

pub(crate) fn split_terminal_answer_deltas(answer: &str) -> Vec<TerminalAnswerDelta> {
    let mut remaining = answer;
    let mut inside_think = false;
    let mut deltas = Vec::new();

    while !remaining.is_empty() {
        let tag = if inside_think { "</think>" } else { "<think>" };
        let Some(tag_index) = remaining.find(tag) else {
            push_terminal_answer_delta(&mut deltas, inside_think, remaining);
            break;
        };

        push_terminal_answer_delta(&mut deltas, inside_think, &remaining[..tag_index]);
        remaining = &remaining[tag_index + tag.len()..];
        inside_think = !inside_think;
    }

    deltas
}

fn push_terminal_answer_delta(deltas: &mut Vec<TerminalAnswerDelta>, reasoning: bool, text: &str) {
    if text.is_empty() {
        return;
    }
    deltas.push(TerminalAnswerDelta {
        kind: if reasoning {
            TerminalAnswerDeltaKind::Reasoning
        } else {
            TerminalAnswerDeltaKind::Text
        },
        text: text.to_string(),
    });
}

fn terminal_answer_text_from_value(value: &Value, depth: usize) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Object(object) => {
            if is_runtime_debug_artifact_preview(value) {
                let decoded = decode_runtime_debug_artifact_preview(value)?;
                return terminal_answer_text_from_value(&decoded, depth + 1);
            }
            object
                .get("answer")
                .and_then(|value| terminal_answer_text_from_value(value, depth + 1))
                .or_else(|| {
                    object
                        .get("text")
                        .and_then(|value| terminal_answer_text_from_value(value, depth + 1))
                })
                .or_else(|| {
                    object
                        .get("output")
                        .and_then(|value| terminal_answer_text_from_value(value, depth + 1))
                })
        }
        _ => None,
    }
}

fn decode_runtime_debug_artifact_preview(payload: &Value) -> Option<Value> {
    if !is_runtime_debug_artifact_preview(payload) {
        return None;
    }
    let preview = payload.get("preview").and_then(Value::as_str)?;
    serde_json::from_str(preview).ok().or_else(|| {
        let is_truncated = payload
            .get("is_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        (!is_truncated && !preview.is_empty()).then(|| Value::String(preview.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use control_plane::application_public_api::native::{
        NativeRequiredAction, NativeRunResult, NativeRunStatus,
    };
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::{
        durable_native_run_matches_terminal, split_terminal_answer_deltas,
        terminal_answer_deltas_from_payload, terminal_answer_runtime_events_from_native_run,
        terminal_runtime_event_from_native_run, TerminalAnswerDeltaKind,
    };

    fn native_run(status: NativeRunStatus) -> NativeRunResult {
        NativeRunResult {
            id: Uuid::from_u128(0x11111111111111111111111111111111),
            application_id: Uuid::from_u128(0x22222222222222222222222222222222),
            api_key_id: Uuid::from_u128(0x33333333333333333333333333333333),
            publication_version_id: Uuid::from_u128(0x44444444444444444444444444444444),
            status,
            node_input_payload: json!({}),
            metadata: json!({}),
            answer: Some("done".to_string()),
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
    fn terminal_fallback_maps_succeeded_native_run_to_flow_finished() {
        let event = terminal_runtime_event_from_native_run(&native_run(NativeRunStatus::Succeeded))
            .expect("succeeded run should synthesize a terminal runtime event");

        assert_eq!(event.event_type, "flow_finished");
        assert_eq!(event.payload["output"]["answer"], json!("done"));
    }

    #[test]
    fn d1_ac_007_terminal_fallback_maps_incomplete_native_run_to_flow_incomplete() {
        let event =
            terminal_runtime_event_from_native_run(&native_run(NativeRunStatus::Incomplete))
                .expect("incomplete run should synthesize a terminal runtime event");

        assert_eq!(event.event_type, "flow_incomplete");
        assert_eq!(event.payload["status"], json!("incomplete"));
        assert_ne!(event.event_type, "flow_finished");
    }

    #[test]
    fn terminal_fallback_ignores_non_terminal_native_run() {
        assert!(
            terminal_runtime_event_from_native_run(&native_run(NativeRunStatus::Running)).is_none()
        );
    }

    #[test]
    fn success_terminal_requires_a_matching_durable_success_status() {
        assert!(durable_native_run_matches_terminal(
            &native_run(NativeRunStatus::Succeeded),
            "flow_finished"
        ));
        assert!(!durable_native_run_matches_terminal(
            &native_run(NativeRunStatus::Running),
            "flow_finished"
        ));
        assert!(!durable_native_run_matches_terminal(
            &native_run(NativeRunStatus::Failed),
            "flow_finished"
        ));
    }

    #[test]
    fn terminal_fallback_maps_waiting_native_run_to_callback_event() {
        let mut run = native_run(NativeRunStatus::Waiting);
        run.required_action = Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({
                "callback_task_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "callback_kind": "llm_tool_calls",
                "node_run_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                "tool_calls": [{"id": "call_read", "name": "Read"}],
            }),
        });
        run.tool_calls = Some(json!([{"id": "call_read", "name": "Read"}]));

        let event = terminal_runtime_event_from_native_run(&run)
            .expect("waiting run should synthesize required-action terminal event");

        assert_eq!(event.event_type, "waiting_callback");
        assert_eq!(event.payload["callback_kind"], json!("llm_tool_calls"));
        assert_eq!(
            event.payload["required_action"]["payload"]["tool_calls"][0]["name"],
            json!("Read")
        );
    }

    #[test]
    fn terminal_fallback_maps_waiting_without_callback_to_human_terminal() {
        let event = terminal_runtime_event_from_native_run(&native_run(NativeRunStatus::Waiting))
            .expect("waiting human run should synthesize a terminal event");

        assert_eq!(event.event_type, "waiting_human");
        assert_eq!(event.payload["status"], json!("waiting_human"));
    }

    #[test]
    fn failed_cancelled_and_waiting_terminals_recover_canonical_partial_before_terminal() {
        for status in [
            NativeRunStatus::Failed,
            NativeRunStatus::Cancelled,
            NativeRunStatus::Waiting,
        ] {
            let events = terminal_answer_runtime_events_from_native_run(&native_run(status));
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].event_type, "text_delta");
            assert_eq!(events[0].payload["text"], json!("done"));
            assert_eq!(events[0].payload["presentation"]["kind"], json!("answer"));
        }
    }

    #[test]
    fn split_terminal_answer_deltas_recovers_native_reasoning_and_text() {
        let deltas = split_terminal_answer_deltas("开头<think>先分析</think>\n最终回答");

        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].kind, TerminalAnswerDeltaKind::Text);
        assert_eq!(deltas[0].text, "开头");
        assert_eq!(deltas[1].kind, TerminalAnswerDeltaKind::Reasoning);
        assert_eq!(deltas[1].text, "先分析");
        assert_eq!(deltas[2].kind, TerminalAnswerDeltaKind::Text);
        assert_eq!(deltas[2].text, "\n最终回答");
    }

    #[test]
    fn terminal_answer_deltas_decode_runtime_artifact_preview_string() {
        let deltas = terminal_answer_deltas_from_payload(&json!({
            "answer": {
                "__runtime_debug_artifact": true,
                "artifact_ref": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "is_truncated": false,
                "preview": "\"最终回答\""
            }
        }));

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].kind, TerminalAnswerDeltaKind::Text);
        assert_eq!(deltas[0].text, "最终回答");
    }

    #[test]
    fn terminal_answer_deltas_decode_runtime_artifact_preview_object_answer() {
        let deltas = terminal_answer_deltas_from_payload(&json!({
            "output": {
                "answer": {
                    "__runtime_debug_artifact": true,
                    "artifact_ref": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                    "is_truncated": false,
                    "preview": "{\"answer\":\"最终回答\"}"
                }
            }
        }));

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].kind, TerminalAnswerDeltaKind::Text);
        assert_eq!(deltas[0].text, "最终回答");
    }

    #[test]
    fn terminal_answer_deltas_prefer_structured_answer_segments() {
        let deltas = terminal_answer_deltas_from_payload(&json!({
            "output": {
                "answer": "<think>旧思考</think>旧回答",
                "answer_segments": [
                    { "kind": "reasoning", "text": "结构化思考" },
                    { "kind": "message", "text": "结构化回答" }
                ]
            }
        }));

        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].kind, TerminalAnswerDeltaKind::Reasoning);
        assert_eq!(deltas[0].text, "结构化思考");
        assert_eq!(deltas[1].kind, TerminalAnswerDeltaKind::Text);
        assert_eq!(deltas[1].text, "结构化回答");
    }
}
