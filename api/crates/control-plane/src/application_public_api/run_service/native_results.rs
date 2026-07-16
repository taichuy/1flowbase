use orchestration_runtime::answer_projection::{
    answer_segments_from_value, AnswerProjectionSegment, ANSWER_SEGMENTS_KEY,
};
use serde_json::{json, Value};

use crate::application_public_api::native::{self, NativeRunResult, NativeRunStatus};

use super::repository_contracts::PublishedRunStreamState;

pub fn native_result_from_flow_run(
    flow_run: &domain::FlowRunRecord,
    metadata: Value,
) -> NativeRunResult {
    NativeRunResult {
        id: flow_run.id,
        application_id: flow_run.application_id,
        api_key_id: flow_run.api_key_id.unwrap_or_default(),
        publication_version_id: flow_run.publication_version_id.unwrap_or_default(),
        status: native_status(flow_run.status),
        node_input_payload: flow_run.input_payload.clone(),
        metadata,
        answer: extract_answer(&flow_run.output_payload),
        answer_segments: extract_answer_segments(&flow_run.output_payload),
        required_action: None,
        tool_calls: extract_tool_calls(&flow_run.output_payload),
        usage: extract_usage(&flow_run.output_payload),
        error: native_error_from_payload(flow_run.error_payload.as_ref()),
        created_at: flow_run.created_at,
    }
}

fn native_error_from_payload(payload: Option<&Value>) -> Option<native::NativeError> {
    payload.map(|payload| {
        let public_details = public_native_error_details(payload);
        let message = payload
            .get("message")
            .or_else(|| payload.get("error"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| payload.to_string());
        native::NativeError {
            code: payload
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("runtime_error")
                .to_string(),
            message: public_native_error_message(payload, &message),
            details: public_details,
        }
    })
}

fn public_native_error_details(payload: &Value) -> Value {
    let mut details = payload.clone();
    if let Value::Object(object) = &mut details {
        if object.get("error_code").and_then(Value::as_str) == Some("provider_upstream_error") {
            if public_provider_upstream_message_contains_raw_body(payload, object.get("message")) {
                object.insert(
                    "message".to_string(),
                    Value::String("provider upstream request failed".to_string()),
                );
            }
            object.remove("provider_summary");
        }
        object.remove("provider_details");
    }
    details
}

fn public_native_error_message(payload: &Value, message: &str) -> String {
    if payload.get("error_code").and_then(Value::as_str) != Some("provider_upstream_error") {
        return message.to_string();
    }

    if public_provider_upstream_message_contains_raw_body(
        payload,
        Some(&Value::String(message.to_string())),
    ) {
        "provider upstream request failed".to_string()
    } else {
        message.to_string()
    }
}

fn public_provider_upstream_message_contains_raw_body(
    payload: &Value,
    message: Option<&Value>,
) -> bool {
    let Some(message) = message.and_then(Value::as_str) else {
        return false;
    };
    let Some(raw_body) = payload
        .get("provider_details")
        .and_then(|details| details.get("raw_body"))
        .and_then(Value::as_str)
        .filter(|raw_body| !raw_body.trim().is_empty())
    else {
        return false;
    };
    message.contains(raw_body)
}

pub fn native_result_from_run_detail(
    detail: &domain::ApplicationRunDetail,
    metadata: Value,
) -> NativeRunResult {
    let mut result = native_result_from_flow_run(&detail.flow_run, metadata);
    if result.usage.is_none() {
        result.usage = aggregate_usage_payloads(detail.node_runs.iter().map(|node_run| {
            (
                node_run.metrics_payload.get("usage"),
                node_run.output_payload.get("usage"),
            )
        }));
    }
    apply_pending_callback_task(
        &mut result,
        latest_pending_callback_task(&detail.callback_tasks),
    );
    result
}

pub fn native_result_from_run_stream_state(
    initial_run: &NativeRunResult,
    stream_state: &PublishedRunStreamState,
) -> NativeRunResult {
    let mut result = initial_run.clone();
    result.status = native_status(stream_state.status);
    result.answer = extract_answer(&stream_state.output_payload);
    result.answer_segments = extract_answer_segments(&stream_state.output_payload);
    result.required_action = None;
    result.tool_calls = extract_tool_calls(&stream_state.output_payload);
    result.usage = extract_usage(&stream_state.output_payload).or_else(|| {
        aggregate_usage_payloads(stream_state.node_usages.iter().map(|node_usage| {
            (
                node_usage.metrics_usage.as_ref(),
                node_usage.output_usage.as_ref(),
            )
        }))
    });
    result.error = native_error_from_payload(stream_state.error_payload.as_ref());
    apply_pending_callback_task(
        &mut result,
        stream_state.latest_pending_callback_task.as_ref(),
    );
    result
}

fn latest_pending_callback_task(
    tasks: &[domain::CallbackTaskRecord],
) -> Option<&domain::CallbackTaskRecord> {
    tasks
        .iter()
        .rev()
        .find(|task| task.status == domain::CallbackTaskStatus::Pending)
}

fn apply_pending_callback_task(
    result: &mut NativeRunResult,
    task: Option<&domain::CallbackTaskRecord>,
) {
    let Some(task) = task else {
        return;
    };
    result.required_action = Some(native_required_action_from_callback_task(task));
    if task.callback_kind == "llm_tool_calls" {
        result.tool_calls = task
            .request_payload
            .get("tool_calls")
            .filter(|value| value.is_array())
            .cloned();
    }
}

fn native_required_action_from_callback_task(
    task: &domain::CallbackTaskRecord,
) -> native::NativeRequiredAction {
    let action_type = if task.callback_kind == "llm_tool_calls" {
        "submit_tool_outputs"
    } else {
        "callback"
    };
    native::NativeRequiredAction {
        action_type: action_type.to_string(),
        payload: json!({
            "callback_task_id": task.id,
            "callback_kind": task.callback_kind,
            "flow_run_id": task.flow_run_id,
            "node_run_id": task.node_run_id,
            "request_payload": task.request_payload,
            "tool_calls": task
                .request_payload
                .get("tool_calls")
                .cloned()
                .unwrap_or(Value::Null),
        }),
    }
}

fn extract_answer(output_payload: &Value) -> Option<String> {
    output_payload
        .get("answer")
        .or_else(|| output_payload.get("text"))
        .or_else(|| output_payload.get("output"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extract_answer_segments(output_payload: &Value) -> Option<Vec<AnswerProjectionSegment>> {
    let segments = output_payload
        .get(ANSWER_SEGMENTS_KEY)
        .or_else(|| {
            output_payload
                .get("output")
                .and_then(|output| output.get(ANSWER_SEGMENTS_KEY))
        })
        .map(answer_segments_from_value)
        .unwrap_or_default();

    (!segments.is_empty()).then_some(segments)
}

fn extract_tool_calls(output_payload: &Value) -> Option<Value> {
    output_payload
        .get("tool_calls")
        .filter(|value| value.is_array())
        .cloned()
}

fn extract_usage(output_payload: &Value) -> Option<native::NativeUsage> {
    let usage = output_payload.get("usage")?;
    usage_from_payload(usage)
}

fn aggregate_usage_payloads<'a>(
    node_usages: impl IntoIterator<Item = (Option<&'a Value>, Option<&'a Value>)>,
) -> Option<native::NativeUsage> {
    let mut aggregate = native::NativeUsage::default();
    let mut saw_usage = false;

    for (metrics_usage, output_usage) in node_usages {
        let usage = metrics_usage
            .and_then(usage_from_payload)
            .or_else(|| output_usage.and_then(usage_from_payload));
        let Some(usage) = usage else {
            continue;
        };
        saw_usage = true;
        merge_usage(&mut aggregate, usage_with_total(usage));
    }

    saw_usage.then_some(aggregate)
}

fn usage_from_payload(usage: &Value) -> Option<native::NativeUsage> {
    let native_usage = native::NativeUsage {
        prompt_tokens: usage_number(usage, &["prompt_tokens", "input_tokens"]),
        completion_tokens: usage_number(usage, &["completion_tokens", "output_tokens"]),
        total_tokens: usage_number(usage, &["total_tokens"]),
        reasoning_tokens: usage_number(usage, &["reasoning_tokens"]),
        input_cache_hit_tokens: usage_number(usage, &["input_cache_hit_tokens"]),
        input_cache_miss_tokens: usage_number(usage, &["input_cache_miss_tokens"]),
        cache_read_tokens: usage_number(usage, &["cache_read_tokens", "cache_read_input_tokens"]),
        cache_write_tokens: usage_number(
            usage,
            &["cache_write_tokens", "cache_creation_input_tokens"],
        ),
    };

    native_usage_has_any_tokens(&native_usage).then_some(native_usage)
}

fn usage_number(usage: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| usage.get(*key).and_then(Value::as_u64))
}

fn usage_with_total(mut usage: native::NativeUsage) -> native::NativeUsage {
    if usage.total_tokens.is_none() {
        usage.total_tokens = match (usage.prompt_tokens, usage.completion_tokens) {
            (Some(prompt_tokens), Some(completion_tokens)) => {
                Some(prompt_tokens + completion_tokens)
            }
            _ => None,
        };
    }
    usage
}

fn native_usage_has_any_tokens(usage: &native::NativeUsage) -> bool {
    usage.prompt_tokens.is_some()
        || usage.completion_tokens.is_some()
        || usage.total_tokens.is_some()
        || usage.reasoning_tokens.is_some()
        || usage.input_cache_hit_tokens.is_some()
        || usage.input_cache_miss_tokens.is_some()
        || usage.cache_read_tokens.is_some()
        || usage.cache_write_tokens.is_some()
}

fn merge_usage(target: &mut native::NativeUsage, delta: native::NativeUsage) {
    add_usage_tokens(&mut target.prompt_tokens, delta.prompt_tokens);
    add_usage_tokens(&mut target.completion_tokens, delta.completion_tokens);
    add_usage_tokens(&mut target.total_tokens, delta.total_tokens);
    add_usage_tokens(&mut target.reasoning_tokens, delta.reasoning_tokens);
    add_usage_tokens(
        &mut target.input_cache_hit_tokens,
        delta.input_cache_hit_tokens,
    );
    add_usage_tokens(
        &mut target.input_cache_miss_tokens,
        delta.input_cache_miss_tokens,
    );
    add_usage_tokens(&mut target.cache_read_tokens, delta.cache_read_tokens);
    add_usage_tokens(&mut target.cache_write_tokens, delta.cache_write_tokens);
}

fn add_usage_tokens(target: &mut Option<u64>, delta: Option<u64>) {
    if let Some(delta) = delta {
        *target = Some(target.unwrap_or_default() + delta);
    }
}

fn native_status(status: domain::FlowRunStatus) -> NativeRunStatus {
    match status {
        domain::FlowRunStatus::Queued => NativeRunStatus::Queued,
        domain::FlowRunStatus::Running => NativeRunStatus::Running,
        domain::FlowRunStatus::WaitingCallback | domain::FlowRunStatus::WaitingHuman => {
            NativeRunStatus::Waiting
        }
        domain::FlowRunStatus::Paused => NativeRunStatus::Running,
        domain::FlowRunStatus::Succeeded => NativeRunStatus::Succeeded,
        domain::FlowRunStatus::Failed => NativeRunStatus::Failed,
        domain::FlowRunStatus::Cancelled => NativeRunStatus::Cancelled,
    }
}
