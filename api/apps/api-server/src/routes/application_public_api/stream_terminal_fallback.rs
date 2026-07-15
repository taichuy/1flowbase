use control_plane::{
    application_public_api::{
        native::{
            answer_segments_from_text, answer_segments_from_value, answer_segments_value,
            AnswerProjectionSegment, AnswerProjectionSegmentKind, NativeRunResult, NativeRunStatus,
            ANSWER_SEGMENTS_KEY,
        },
        run_service::{native_result_from_run_detail, ApplicationPublishedRunControlRepository},
    },
    orchestration_runtime::{
        debug_artifacts::is_runtime_debug_artifact_preview, debug_stream_events,
    },
    ports::{
        FileManagementRepository, GetRuntimeDebugArtifactInput, OrchestrationRuntimeRepository,
        RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventSource,
    },
};
use serde_json::{json, Value};
use tracing::warn;
use uuid::Uuid;

use crate::app_state::ApiState;

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

pub(crate) async fn load_latest_native_run_for_terminal_fallback(
    state: &ApiState,
    initial_run: &NativeRunResult,
) -> NativeRunResult {
    match state
        .store
        .get_published_run_detail(initial_run.application_id, initial_run.id)
        .await
    {
        Ok(Some(detail)) => native_result_from_run_detail(&detail, initial_run.metadata.clone()),
        Ok(None) => {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "compatible/native stream closed without terminal event and no durable run detail was found"
            );
            initial_run.clone()
        }
        Err(error) => {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                error = %error,
                "failed to load durable run detail for stream terminal fallback"
            );
            initial_run.clone()
        }
    }
}

pub(crate) fn terminal_runtime_event_from_native_run(
    run: &NativeRunResult,
) -> Option<RuntimeEventEnvelope> {
    let payload = match run.status {
        NativeRunStatus::Succeeded => {
            debug_stream_events::flow_finished(run.id, terminal_output_payload(run))
        }
        NativeRunStatus::Failed => {
            debug_stream_events::flow_failed(run.id, terminal_error_payload(run))
        }
        NativeRunStatus::Cancelled => debug_stream_events::flow_cancelled(run.id),
        NativeRunStatus::Waiting => waiting_callback_payload(run)?,
        NativeRunStatus::Created | NativeRunStatus::Queued | NativeRunStatus::Running => {
            return None;
        }
    };
    Some(RuntimeEventEnvelope::new(run.id, 0, payload))
}

pub(crate) async fn enrich_terminal_runtime_event_with_durable_answer(
    state: &ApiState,
    run: &NativeRunResult,
    mut event: RuntimeEventEnvelope,
) -> RuntimeEventEnvelope {
    if !matches!(event.event_type.as_str(), "flow_finished" | "flow_failed") {
        return event;
    }
    if !terminal_answer_deltas_from_payload(&event.payload).is_empty()
        || run
            .answer
            .as_deref()
            .is_some_and(|answer| !answer.is_empty())
    {
        return event;
    }

    let deltas = recover_terminal_answer_deltas_from_durable_runtime_events(state, run).await;
    if deltas.is_empty() {
        return event;
    }

    put_terminal_answer_in_payload(
        event.event_type.as_str(),
        &mut event.payload,
        terminal_answer_deltas_to_answer_text(&deltas),
    );
    event
}

pub(crate) async fn recover_terminal_answer_deltas_from_durable_runtime_events(
    state: &ApiState,
    run: &NativeRunResult,
) -> Vec<TerminalAnswerDelta> {
    let records = match state.store.list_runtime_events(run.id, 0).await {
        Ok(records) => records,
        Err(error) => {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                error = %error,
                "failed to load durable runtime events for terminal answer fallback"
            );
            return Vec::new();
        }
    };

    let presentation_deltas = terminal_answer_deltas_from_runtime_records(&records);
    if !presentation_deltas.is_empty() {
        return presentation_deltas;
    }

    for record in records.iter().rev() {
        if !matches!(record.event_type.as_str(), "flow_finished" | "flow_failed") {
            continue;
        }
        let deltas =
            terminal_answer_deltas_from_payload_resolving_artifacts(state, run, &record.payload)
                .await;
        if !deltas.is_empty() {
            return deltas;
        }
    }

    Vec::new()
}

fn terminal_answer_deltas_from_runtime_records(
    records: &[domain::RuntimeEventRecord],
) -> Vec<TerminalAnswerDelta> {
    records
        .iter()
        .filter(|record| {
            matches!(record.event_type.as_str(), "text_delta" | "reasoning_delta")
                && debug_stream_events::is_answer_presentation_delta_payload(&record.payload)
        })
        .filter_map(|record| {
            let text = record
                .payload
                .get("text")
                .or_else(|| record.payload.get("delta"))
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())?;
            let kind = if record.event_type == "reasoning_delta" {
                TerminalAnswerDeltaKind::Reasoning
            } else {
                TerminalAnswerDeltaKind::Text
            };
            Some(TerminalAnswerDelta {
                kind,
                text: text.to_string(),
            })
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

fn waiting_callback_payload(run: &NativeRunResult) -> Option<RuntimeEventPayload> {
    let action = run.required_action.as_ref()?;
    let callback_task_id = action
        .payload
        .get("callback_task_id")
        .cloned()
        .unwrap_or(Value::Null);
    let callback_kind = action
        .payload
        .get("callback_kind")
        .cloned()
        .unwrap_or(Value::Null);
    let tool_calls = run
        .tool_calls
        .clone()
        .or_else(|| action.payload.get("tool_calls").cloned())
        .unwrap_or(Value::Null);

    Some(RuntimeEventPayload {
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
            "request_payload": action
                .payload
                .get("request_payload")
                .cloned()
                .unwrap_or(Value::Null),
            "tool_calls": tool_calls,
            "required_action": action,
        }),
    })
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

async fn terminal_answer_deltas_from_payload_resolving_artifacts(
    state: &ApiState,
    run: &NativeRunResult,
    payload: &Value,
) -> Vec<TerminalAnswerDelta> {
    let direct_deltas = terminal_answer_deltas_from_payload(payload);
    if !direct_deltas.is_empty() {
        return direct_deltas;
    }

    for artifact_id in terminal_answer_artifact_ids_from_payload(payload) {
        let Some(value) = load_runtime_debug_artifact_json_value(state, run, artifact_id).await
        else {
            continue;
        };
        let deltas = terminal_answer_deltas_from_payload(&value);
        if !deltas.is_empty() {
            return deltas;
        }
        let Some(answer) = terminal_answer_text_from_value(&value, 0) else {
            continue;
        };
        let deltas = split_terminal_answer_deltas(&answer);
        if !deltas.is_empty() {
            return deltas;
        }
    }

    Vec::new()
}

async fn load_runtime_debug_artifact_json_value(
    state: &ApiState,
    run: &NativeRunResult,
    artifact_id: Uuid,
) -> Option<Value> {
    let workspace_id = runtime_debug_artifact_workspace_id(state, run, artifact_id).await?;
    let artifact = match state
        .store
        .get_runtime_debug_artifact(&GetRuntimeDebugArtifactInput {
            workspace_id,
            application_id: run.application_id,
            artifact_id,
        })
        .await
    {
        Ok(Some(artifact)) => artifact,
        Ok(None) => return None,
        Err(error) => {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                artifact_id = %artifact_id,
                error = %error,
                "failed to load runtime debug artifact metadata for terminal answer fallback"
            );
            return None;
        }
    };

    let storage = match state.store.get_file_storage(artifact.storage_id).await {
        Ok(Some(storage)) => storage,
        Ok(None) => return None,
        Err(error) => {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                artifact_id = %artifact_id,
                error = %error,
                "failed to load runtime debug artifact storage for terminal answer fallback"
            );
            return None;
        }
    };
    if !storage.enabled {
        warn!(
            flow_run_id = %run.id,
            application_id = %run.application_id,
            artifact_id = %artifact_id,
            storage_id = %storage.id,
            "runtime debug artifact storage is disabled for terminal answer fallback"
        );
        return None;
    }
    let Some(driver) = state.file_storage_registry.get(&storage.driver_type) else {
        warn!(
            flow_run_id = %run.id,
            application_id = %run.application_id,
            artifact_id = %artifact_id,
            storage_id = %storage.id,
            driver_type = %storage.driver_type,
            "runtime debug artifact storage driver is not registered for terminal answer fallback"
        );
        return None;
    };
    let object = match driver
        .open_read(storage_object::OpenReadInput {
            config_json: &storage.config_json,
            object_path: &artifact.storage_ref,
        })
        .await
    {
        Ok(object) => object,
        Err(error) => {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                artifact_id = %artifact_id,
                error = %error,
                "failed to read runtime debug artifact object for terminal answer fallback"
            );
            return None;
        }
    };

    match serde_json::from_slice(&object.bytes) {
        Ok(value) => Some(value),
        Err(error) => {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                artifact_id = %artifact_id,
                error = %error,
                "runtime debug artifact object is not JSON for terminal answer fallback"
            );
            None
        }
    }
}

async fn runtime_debug_artifact_workspace_id(
    state: &ApiState,
    run: &NativeRunResult,
    artifact_id: Uuid,
) -> Option<Uuid> {
    match sqlx::query_scalar::<_, Uuid>(
        r#"
        select workspace_id
        from runtime_debug_artifacts
        where id = $1
          and application_id = $2
          and (flow_run_id = $3 or flow_run_id is null)
        "#,
    )
    .bind(artifact_id)
    .bind(run.application_id)
    .bind(run.id)
    .fetch_optional(state.store.pool())
    .await
    {
        Ok(workspace_id) => workspace_id,
        Err(error) => {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                artifact_id = %artifact_id,
                error = %error,
                "failed to resolve runtime debug artifact workspace for terminal answer fallback"
            );
            None
        }
    }
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

fn terminal_answer_artifact_ids_from_payload(payload: &Value) -> Vec<Uuid> {
    let mut artifact_ids = Vec::new();
    if let Some(answer) = payload
        .get("output")
        .and_then(|output| output.get("answer"))
    {
        collect_terminal_answer_artifact_ids(answer, &mut artifact_ids, 0);
    }
    if let Some(answer) = payload.get("answer") {
        collect_terminal_answer_artifact_ids(answer, &mut artifact_ids, 0);
    }
    if let Some(output) = payload.get("output") {
        collect_terminal_answer_artifact_ids(output, &mut artifact_ids, 0);
    }
    artifact_ids.dedup();
    artifact_ids
}

fn collect_terminal_answer_artifact_ids(value: &Value, artifact_ids: &mut Vec<Uuid>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Some(object) = value.as_object() else {
        return;
    };
    if is_runtime_debug_artifact_preview(value) {
        if let Some(artifact_id) = object
            .get("artifact_ref")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        {
            artifact_ids.push(artifact_id);
        }
        return;
    }
    for key in ["answer", "text", "output"] {
        if let Some(value) = object.get(key) {
            collect_terminal_answer_artifact_ids(value, artifact_ids, depth + 1);
        }
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

fn terminal_answer_deltas_to_answer_text(deltas: &[TerminalAnswerDelta]) -> String {
    let mut answer = String::new();
    for delta in deltas {
        match delta.kind {
            TerminalAnswerDeltaKind::Reasoning => {
                answer.push_str("<think>");
                answer.push_str(&delta.text);
                answer.push_str("</think>");
            }
            TerminalAnswerDeltaKind::Text => answer.push_str(&delta.text),
        }
    }
    answer
}

fn put_terminal_answer_in_payload(event_type: &str, payload: &mut Value, answer: String) {
    let answer_segments = answer_segments_value(&answer_segments_from_text(&answer));
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    if event_type == "flow_finished" {
        let output = object.entry("output").or_insert_with(|| json!({}));
        if !output.is_object() {
            *output = json!({});
        }
        if let Some(output) = output.as_object_mut() {
            output.insert("answer".to_string(), Value::String(answer));
            if let Some(answer_segments) = answer_segments {
                output.insert(ANSWER_SEGMENTS_KEY.to_string(), answer_segments);
            }
        }
    } else {
        object.insert("answer".to_string(), Value::String(answer));
        if let Some(answer_segments) = answer_segments {
            object.insert(ANSWER_SEGMENTS_KEY.to_string(), answer_segments);
        }
    }
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
        split_terminal_answer_deltas, terminal_answer_deltas_from_payload,
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
    fn terminal_fallback_ignores_non_terminal_native_run() {
        assert!(
            terminal_runtime_event_from_native_run(&native_run(NativeRunStatus::Running)).is_none()
        );
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
        assert_eq!(event.payload["tool_calls"][0]["name"], json!("Read"));
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
