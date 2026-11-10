// API compatibility shims retain the existing test roots. Concrete projection
// bodies and state live in application-public-protocol.
use super::*;

mod anthropic_stream;
mod openai_chat;
mod openai_responses;

pub(super) use anthropic_stream::AnthropicStreamMapper;
pub(super) use openai_chat::OpenAiChatStreamMapper;
pub(super) use openai_responses::OpenAiResponseStreamMapper;

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(super) use application_public_protocol::protocol_mappers::{
    anthropic_tool_use_blocks_from_waiting_payload, openai_response_function_call_output_items,
};

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use crate::routes::application_public_api::stream_terminal_fallback::{
    terminal_answer_deltas_from_payload, terminal_answer_text_from_payload, TerminalAnswerDelta,
    TerminalAnswerDeltaKind,
};

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
fn terminal_answer_text(run: &NativeRunResult, payload: &Value) -> Option<String> {
    terminal_answer_text_from_payload(payload).or_else(|| {
        run.answer
            .as_ref()
            .filter(|answer| !answer.is_empty())
            .cloned()
    })
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(super) fn terminal_answer_deltas_from_run_or_payload(
    run: &NativeRunResult,
    payload: &Value,
) -> Vec<TerminalAnswerDelta> {
    let payload_deltas = terminal_answer_deltas_from_payload(payload);
    if !payload_deltas.is_empty() {
        return payload_deltas;
    }

    if let Some(answer_segments) = run
        .answer_segments
        .as_ref()
        .filter(|segments| !segments.is_empty())
    {
        return terminal_answer_deltas_from_payload(&json!({
            "answer_segments": answer_segments
        }));
    }

    terminal_answer_text(run, payload)
        .map(|answer| terminal_answer_deltas_from_payload(&json!({ "answer": answer })))
        .unwrap_or_default()
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(super) fn anthropic_completed_run_to_sse(
    run: &NativeRunResult,
    model: &str,
) -> Vec<Result<Event, Infallible>> {
    let mut mapper = AnthropicStreamMapper::new(model.to_string());
    let mut events = mapper.runtime_event_to_sse(
        run,
        RuntimeEventEnvelope::new(run.id, 0, debug_stream_events::flow_started(run.id)),
    );
    if let Some(payload) = waiting_payload_from_run(run) {
        if let Some(tool_events) = mapper.anthropic_tool_use_events(&payload, run.usage.as_ref()) {
            events.extend(tool_events);
            return events;
        }
    }
    for (index, delta) in terminal_answer_deltas_from_run_or_payload(run, &json!({}))
        .into_iter()
        .enumerate()
    {
        let event = terminal_answer_delta_to_runtime_event(run, index as i64 + 1, delta);
        events.extend(mapper.runtime_event_to_sse(run, event));
    }
    events.extend(mapper.anthropic_stop_events(run.usage.as_ref()));
    events
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
fn terminal_answer_delta_to_runtime_event(
    run: &NativeRunResult,
    sequence: i64,
    delta: TerminalAnswerDelta,
) -> RuntimeEventEnvelope {
    let payload = match delta.kind {
        TerminalAnswerDeltaKind::Reasoning => debug_stream_events::answer_reasoning_delta(
            "assistant",
            delta.text,
            sequence as usize,
            None,
            None,
            None,
        ),
        TerminalAnswerDeltaKind::Text => debug_stream_events::answer_text_delta(
            "assistant",
            delta.text,
            sequence as usize,
            None,
            None,
            None,
        ),
    };
    RuntimeEventEnvelope::new(run.id, sequence, payload)
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
fn waiting_payload_from_run(run: &NativeRunResult) -> Option<Value> {
    let action = run.required_action.as_ref()?;
    Some(json!({
        "callback_kind": action.payload.get("callback_kind").cloned().unwrap_or(Value::Null),
        "callback_task_id": action.payload.get("callback_task_id").cloned().unwrap_or(Value::Null),
        "tool_calls": run.tool_calls.clone().unwrap_or(Value::Null),
    }))
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(super) use anthropic_stream::anthropic_delta_payload;

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
pub(super) use openai_chat::{
    openai_delta_chunk_payload, openai_finish_chunk_payload, openai_tool_call_chunk_payload,
};
