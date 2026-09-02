use super::*;

#[derive(Clone)]
pub(super) struct LlmToolCallbackRuntimeFacts {
    callback_payload: Value,
    duration_ms: Option<i64>,
}

#[derive(Clone)]
pub(super) struct LlmToolCallbackArtifact {
    id: String,
    name: String,
    request_payload: Value,
    callback_payload: Option<Value>,
    request_round_index: Option<i64>,
    result_round_index: Option<i64>,
    call_usage: Option<Value>,
    result_context_usage: Option<Value>,
    duration_ms: Option<i64>,
    route_trace: Option<Value>,
}

impl LlmToolCallbackArtifact {
    fn callback_status(&self) -> &'static str {
        if self.callback_payload.is_some() {
            "returned"
        } else {
            "waiting_callback"
        }
    }

    pub(super) fn detail_payload(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "callback_status": self.callback_status(),
            "execution_status": execution_status_from_callback_payload_and_route_trace(
                self.callback_payload.as_ref(),
                self.route_trace.as_ref()
            ),
            "request_payload": self.request_payload,
            "callback_payload": self.callback_payload,
            "parsed_result": self.callback_payload.as_ref().map(parsed_tool_callback_payload),
            "request_round_index": self.request_round_index,
            "result_round_index": self.result_round_index,
            "call_usage": self.call_usage,
            "result_context_usage": self.result_context_usage,
            "duration_ms": self.duration_ms,
            "route_trace": self.route_trace,
        })
    }

    fn summary_payload_base(&self) -> Map<String, Value> {
        let Some(object) = json!({
            "id": self.id,
            "name": self.name,
            "callback_status": self.callback_status(),
            "execution_status": execution_status_from_callback_payload_and_route_trace(
                self.callback_payload.as_ref(),
                self.route_trace.as_ref()
            ),
            "request_round_index": self.request_round_index,
            "result_round_index": self.result_round_index,
            "call_usage": self.call_usage,
            "result_context_usage": self.result_context_usage,
            "duration_ms": self.duration_ms,
            "route_trace": self.route_trace.as_ref().map(lightweight_route_trace_summary),
        })
        .as_object()
        .cloned() else {
            return Map::new();
        };

        object
    }

    pub(super) fn summary_payload(&self, artifact_id: Uuid) -> Value {
        let mut object = self.summary_payload_base();
        object.insert("artifact_ref".to_string(), json!(artifact_id.to_string()));

        Value::Object(object)
    }
}

fn lightweight_route_trace_summary(route_trace: &Value) -> Value {
    let Some(route_trace_object) = route_trace.as_object() else {
        return route_trace.clone();
    };
    let mut summary = route_trace_object.clone();
    for field in [
        "arguments",
        "branch_traces",
        "callback_requests",
        "events",
        "final_output",
        "main_resume_output",
        "route_output",
        "tool_call",
        "tool_result",
    ] {
        summary.remove(field);
    }

    Value::Object(summary)
}

fn inline_route_trace_tool_call_id(route_trace: &Value) -> Option<String> {
    route_trace
        .as_object()
        .and_then(|object| record_string_field(object, &["tool_call_id", "id", "call_id"]))
}

fn inline_route_traces_by_tool_call_id(debug_payloads: &[Value]) -> HashMap<String, Value> {
    let mut route_traces = HashMap::new();

    for debug_payload in debug_payloads {
        let Some(traces) = debug_payload
            .get("visible_internal_llm_tool_trace")
            .and_then(Value::as_array)
        else {
            continue;
        };

        for trace in traces {
            let Some(tool_call_id) = inline_route_trace_tool_call_id(trace) else {
                continue;
            };
            route_traces.insert(tool_call_id, trace.clone());
        }
    }

    route_traces
}

pub(super) fn attach_inline_route_traces(
    callbacks: &mut [LlmToolCallbackArtifact],
    debug_payloads: &[Value],
) {
    let route_traces = inline_route_traces_by_tool_call_id(debug_payloads);
    if route_traces.is_empty() {
        return;
    }

    for callback in callbacks {
        if callback.route_trace.is_none() {
            callback.route_trace = route_traces.get(&callback.id).cloned();
        }
    }
}

pub(super) fn is_llm_rounds_field_path(field_path: &[String]) -> bool {
    field_path.len() == 1 && field_path[0] == "llm_rounds"
}

pub(super) fn is_tool_calls_field_path(field_path: &[String]) -> bool {
    field_path.last().is_some_and(|key| key == "tool_calls")
}

pub(super) fn with_array_item_count(
    mut payload: Value,
    full_value: &Value,
    field_name: &str,
) -> Value {
    let Some(count) = full_value.as_array().map(|items| items.len() as i64) else {
        return payload;
    };
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    object.insert(field_name.to_string(), json!(count));
    payload
}

pub(super) fn is_llm_rounds_debug_artifact_missing_tool_index(value: &Value) -> bool {
    is_runtime_debug_artifact_payload(value) && value.get("tool_callbacks").is_none()
}

fn value_object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

fn record_field<'a>(record: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| record.get(*key))
}

pub(super) fn record_string_field(record: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        record
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

pub(super) fn record_value_field(record: &Map<String, Value>, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| record.get(*key).cloned())
}

fn record_i64_field(record: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        let value = record.get(*key)?;
        if let Some(value) = value.as_i64() {
            return Some(value);
        }

        value.as_u64().and_then(|value| i64::try_from(value).ok())
    })
}

fn round_index(round: &Map<String, Value>, fallback_index: usize) -> i64 {
    round
        .get("round_index")
        .and_then(Value::as_i64)
        .unwrap_or(fallback_index as i64)
}

fn read_round_tool_calls(round: &Map<String, Value>) -> Vec<Value> {
    let assistant_tool_calls = record_field(round, &["assistant", "assistant_message"])
        .and_then(value_object)
        .and_then(|assistant| assistant.get("tool_calls"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if !assistant_tool_calls.is_empty() {
        return assistant_tool_calls;
    }

    round
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn read_round_tool_results(round: &Map<String, Value>) -> Vec<Value> {
    round
        .get("tool_results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn tool_call_id(tool_call: &Map<String, Value>, round_number: i64, index: usize) -> String {
    record_string_field(tool_call, &["id", "tool_call_id", "call_id"])
        .unwrap_or_else(|| format!("tool-{}-{}", round_number + 1, index + 1))
}

fn tool_result_id(tool_result: &Map<String, Value>, round_number: i64, index: usize) -> String {
    record_string_field(tool_result, &["tool_call_id", "id", "call_id"])
        .unwrap_or_else(|| format!("tool-result-{}-{}", round_number + 1, index + 1))
}

fn callback_duration_ms(task: &domain::CallbackTaskRecord) -> Option<i64> {
    let completed_at = task.completed_at?;
    let duration = completed_at - task.created_at;
    if duration < time::Duration::ZERO {
        return None;
    }

    i64::try_from(duration.whole_milliseconds()).ok()
}

pub(super) fn collect_llm_tool_callback_runtime_facts(
    callback_tasks: &[domain::CallbackTaskRecord],
) -> HashMap<String, LlmToolCallbackRuntimeFacts> {
    let mut facts_by_tool_call_id = HashMap::new();

    for task in callback_tasks {
        if task.callback_kind != "llm_tool_calls" {
            continue;
        }

        let Some(response_payload) = task.response_payload.as_ref() else {
            continue;
        };
        let duration_ms = callback_duration_ms(task);

        for callback_payload in read_callback_response_tool_payloads(response_payload) {
            let Some(callback_payload_object) = callback_payload.as_object() else {
                continue;
            };
            let Some(tool_call_id) =
                record_string_field(callback_payload_object, &["tool_call_id", "id", "call_id"])
            else {
                continue;
            };

            facts_by_tool_call_id.insert(
                tool_call_id,
                LlmToolCallbackRuntimeFacts {
                    callback_payload,
                    duration_ms,
                },
            );
        }
    }

    facts_by_tool_call_id
}

fn read_callback_response_tool_payloads(response_payload: &Value) -> Vec<Value> {
    if let Some(tool_results) = response_payload
        .get("tool_results")
        .and_then(Value::as_array)
        .cloned()
    {
        return tool_results;
    }

    response_payload
        .as_object()
        .and_then(|object| {
            record_string_field(object, &["tool_call_id", "id", "call_id"])
                .map(|_| vec![response_payload.clone()])
        })
        .unwrap_or_default()
}

fn read_callback_request_tool_payloads(request_payload: &Value) -> Vec<Value> {
    if let Some(tool_calls) = request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
    {
        return tool_calls;
    }

    request_payload
        .as_object()
        .and_then(|object| {
            record_string_field(object, &["id", "tool_call_id", "call_id"])
                .map(|_| vec![request_payload.clone()])
        })
        .unwrap_or_default()
}

pub(super) fn execution_status_from_callback_payload(
    callback_payload: Option<&Value>,
) -> &'static str {
    let Some(callback_payload) = callback_payload else {
        return "unknown";
    };
    let Some(callback_payload_object) = callback_payload.as_object() else {
        return "unknown";
    };

    if let Some(status) = callback_payload_object
        .get("execution")
        .and_then(Value::as_object)
        .and_then(|execution| execution.get("status"))
        .and_then(Value::as_str)
        .and_then(normalized_execution_status)
    {
        return status;
    }
    if let Some(status) = callback_payload_object
        .get("execution_status")
        .and_then(Value::as_str)
        .and_then(normalized_execution_status)
    {
        return status;
    }
    if callback_payload_object
        .get("timed_out")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return "timed_out";
    }
    if callback_payload_object
        .get("cancelled")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return "cancelled";
    }
    if let Some(exit_code) = callback_payload_object
        .get("exit_code")
        .and_then(Value::as_i64)
    {
        return if exit_code == 0 {
            "succeeded"
        } else {
            "failed"
        };
    }
    if let Some(http_status) = callback_payload_object
        .get("http_status")
        .and_then(Value::as_i64)
    {
        return if (200..300).contains(&http_status) {
            "succeeded"
        } else {
            "failed"
        };
    }
    if callback_payload_object
        .get("is_error")
        .and_then(Value::as_bool)
        == Some(true)
        || callback_payload_object
            .get("error")
            .is_some_and(|value| !value.is_null())
    {
        return "failed";
    }

    "unknown"
}

pub(super) fn execution_status_from_callback_payload_and_route_trace(
    callback_payload: Option<&Value>,
    route_trace: Option<&Value>,
) -> &'static str {
    execution_status_from_route_trace(route_trace)
        .unwrap_or_else(|| execution_status_from_callback_payload(callback_payload))
}

fn execution_status_from_route_trace(route_trace: Option<&Value>) -> Option<&'static str> {
    let status = route_trace?
        .get("status")
        .and_then(Value::as_str)
        .and_then(normalized_route_trace_execution_status)?;

    Some(status)
}

fn normalized_route_trace_execution_status(status: &str) -> Option<&'static str> {
    match status {
        "intercepted" => Some("intercepted"),
        "failed" => Some("failed"),
        "succeeded" | "returned_to_main" | "route_completed" => Some("succeeded"),
        _ => None,
    }
}

fn normalized_execution_status(status: &str) -> Option<&'static str> {
    match status {
        "succeeded" => Some("succeeded"),
        "intercepted" => Some("intercepted"),
        "failed" => Some("failed"),
        "timed_out" => Some("timed_out"),
        "cancelled" | "canceled" => Some("cancelled"),
        "unknown" => Some("unknown"),
        _ => None,
    }
}

fn parsed_tool_callback_payload(callback_payload: &Value) -> Value {
    let Some(callback_payload_object) = callback_payload.as_object() else {
        return json!({ "raw": callback_payload });
    };

    let mut parsed_payload = Map::new();
    for key in [
        "tool_call_id",
        "id",
        "call_id",
        "name",
        "content",
        "stdout",
        "stderr",
        "error",
        "exit_code",
        "http_status",
        "is_error",
        "timed_out",
        "cancelled",
        "execution",
        "execution_status",
    ] {
        if let Some(value) = callback_payload_object.get(key) {
            parsed_payload.insert(key.to_string(), value.clone());
        }
    }

    Value::Object(parsed_payload)
}

pub(super) fn collect_llm_tool_callbacks(
    llm_rounds: &Value,
    runtime_facts: &HashMap<String, LlmToolCallbackRuntimeFacts>,
) -> Vec<LlmToolCallbackArtifact> {
    let Some(rounds) = llm_rounds.as_array() else {
        return Vec::new();
    };
    let mut callbacks: Vec<LlmToolCallbackArtifact> = Vec::new();
    let mut index_by_id = std::collections::HashMap::<String, usize>::new();

    for (fallback_round_index, round) in rounds.iter().enumerate() {
        let Some(round) = round.as_object() else {
            continue;
        };
        let current_round_index = round_index(round, fallback_round_index);
        let current_usage = round.get("usage").cloned();
        let next_usage = rounds
            .get(fallback_round_index + 1)
            .and_then(Value::as_object)
            .and_then(|round| round.get("usage"))
            .cloned();

        for (tool_call_index, tool_call) in read_round_tool_calls(round).into_iter().enumerate() {
            let Some(tool_call_object) = tool_call.as_object() else {
                continue;
            };
            let id = tool_call_id(tool_call_object, current_round_index, tool_call_index);
            let name =
                record_string_field(tool_call_object, &["name"]).unwrap_or_else(|| "Tool".into());

            upsert_llm_tool_callback(
                &mut callbacks,
                &mut index_by_id,
                LlmToolCallbackArtifact {
                    callback_payload: runtime_facts
                        .get(&id)
                        .map(|facts| facts.callback_payload.clone()),
                    duration_ms: runtime_facts.get(&id).and_then(|facts| facts.duration_ms),
                    id,
                    name,
                    call_usage: record_value_field(tool_call_object, &["call_usage"])
                        .or_else(|| current_usage.clone()),
                    result_context_usage: None,
                    request_payload: tool_call,
                    request_round_index: Some(current_round_index),
                    result_round_index: None,
                    route_trace: None,
                },
            );
        }

        for (tool_result_index, tool_result) in
            read_round_tool_results(round).into_iter().enumerate()
        {
            let Some(tool_result_object) = tool_result.as_object() else {
                continue;
            };
            let id = tool_result_id(tool_result_object, current_round_index, tool_result_index);
            let name =
                record_string_field(tool_result_object, &["name"]).unwrap_or_else(|| "Tool".into());

            upsert_llm_tool_callback(
                &mut callbacks,
                &mut index_by_id,
                LlmToolCallbackArtifact {
                    callback_payload: runtime_facts
                        .get(&id)
                        .map(|facts| facts.callback_payload.clone())
                        .or_else(|| Some(tool_result.clone())),
                    duration_ms: record_i64_field(tool_result_object, &["duration_ms"])
                        .or_else(|| runtime_facts.get(&id).and_then(|facts| facts.duration_ms)),
                    id,
                    name,
                    call_usage: record_value_field(tool_result_object, &["call_usage"]),
                    result_context_usage: record_value_field(
                        tool_result_object,
                        &["result_context_usage"],
                    )
                    .or_else(|| next_usage.clone()),
                    request_payload: json!({}),
                    request_round_index: None,
                    result_round_index: Some(current_round_index),
                    route_trace: None,
                },
            );
        }
    }

    callbacks
}

fn collect_llm_tool_callbacks_from_callback_tasks(
    callback_tasks: &[domain::CallbackTaskRecord],
    runtime_facts: &HashMap<String, LlmToolCallbackRuntimeFacts>,
) -> Vec<LlmToolCallbackArtifact> {
    let mut callbacks: Vec<LlmToolCallbackArtifact> = Vec::new();
    let mut index_by_id = std::collections::HashMap::<String, usize>::new();

    for task in callback_tasks {
        if task.callback_kind != "llm_tool_calls" {
            continue;
        }

        for (tool_call_index, tool_call) in
            read_callback_request_tool_payloads(&task.request_payload)
                .into_iter()
                .enumerate()
        {
            let Some(tool_call_object) = tool_call.as_object() else {
                continue;
            };
            let id = tool_call_id(tool_call_object, 0, tool_call_index);
            let name =
                record_string_field(tool_call_object, &["name"]).unwrap_or_else(|| "Tool".into());
            let call_usage = record_value_field(tool_call_object, &["call_usage"]);

            upsert_llm_tool_callback(
                &mut callbacks,
                &mut index_by_id,
                LlmToolCallbackArtifact {
                    callback_payload: runtime_facts
                        .get(&id)
                        .map(|facts| facts.callback_payload.clone()),
                    duration_ms: runtime_facts.get(&id).and_then(|facts| facts.duration_ms),
                    id,
                    name,
                    request_payload: tool_call,
                    request_round_index: None,
                    result_round_index: None,
                    call_usage,
                    result_context_usage: None,
                    route_trace: None,
                },
            );
        }

        let Some(response_payload) = task.response_payload.as_ref() else {
            continue;
        };
        for (tool_result_index, tool_result) in
            read_callback_response_tool_payloads(response_payload)
                .into_iter()
                .enumerate()
        {
            let Some(tool_result_object) = tool_result.as_object() else {
                continue;
            };
            let id = tool_result_id(tool_result_object, 0, tool_result_index);
            let name =
                record_string_field(tool_result_object, &["name"]).unwrap_or_else(|| "Tool".into());
            let duration_ms = record_i64_field(tool_result_object, &["duration_ms"])
                .or_else(|| runtime_facts.get(&id).and_then(|facts| facts.duration_ms))
                .or_else(|| callback_duration_ms(task));
            let call_usage = record_value_field(tool_result_object, &["call_usage"]);
            let result_context_usage =
                record_value_field(tool_result_object, &["result_context_usage"]);

            upsert_llm_tool_callback(
                &mut callbacks,
                &mut index_by_id,
                LlmToolCallbackArtifact {
                    callback_payload: runtime_facts
                        .get(&id)
                        .map(|facts| facts.callback_payload.clone())
                        .or_else(|| Some(tool_result.clone())),
                    duration_ms,
                    id,
                    name,
                    request_payload: json!({}),
                    request_round_index: None,
                    result_round_index: None,
                    call_usage,
                    result_context_usage,
                    route_trace: None,
                },
            );
        }
    }

    callbacks
}

pub(super) fn count_llm_tool_callback_trace_items(
    debug_payloads: &[Value],
    callback_tasks: &[domain::CallbackTaskRecord],
) -> usize {
    let runtime_facts = collect_llm_tool_callback_runtime_facts(callback_tasks);
    let mut callbacks = Vec::<LlmToolCallbackArtifact>::new();
    let mut index_by_id = std::collections::HashMap::<String, usize>::new();

    for callback in debug_payloads
        .iter()
        .filter_map(|payload| payload.get("llm_rounds"))
        .flat_map(|llm_rounds| collect_llm_tool_callbacks(llm_rounds, &runtime_facts))
    {
        upsert_llm_tool_callback(&mut callbacks, &mut index_by_id, callback);
    }

    for callback in collect_llm_tool_callbacks_from_callback_tasks(callback_tasks, &runtime_facts) {
        upsert_llm_tool_callback(&mut callbacks, &mut index_by_id, callback);
    }
    attach_inline_route_traces(&mut callbacks, debug_payloads);

    let mut callback_ids = callbacks
        .into_iter()
        .map(|callback| callback.id)
        .collect::<std::collections::HashSet<_>>();
    callback_ids.extend(
        debug_payloads
            .iter()
            .filter_map(|payload| payload.get("llm_rounds"))
            .filter_map(|llm_rounds| llm_rounds.get("tool_callbacks"))
            .filter_map(Value::as_array)
            .flatten()
            .enumerate()
            .map(|(index, callback)| {
                callback
                    .get("id")
                    .or_else(|| callback.get("tool_call_id"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| format!("artifact_tool_callback_{index}"))
            }),
    );

    callback_ids.len()
}

fn upsert_llm_tool_callback(
    callbacks: &mut Vec<LlmToolCallbackArtifact>,
    index_by_id: &mut std::collections::HashMap<String, usize>,
    next: LlmToolCallbackArtifact,
) {
    let Some(index) = index_by_id.get(&next.id).copied() else {
        index_by_id.insert(next.id.clone(), callbacks.len());
        callbacks.push(next);
        return;
    };

    let current = &mut callbacks[index];
    if next
        .request_payload
        .as_object()
        .is_some_and(|object| !object.is_empty())
    {
        current.request_payload = next.request_payload;
    }
    if next.callback_payload.is_some() {
        current.callback_payload = next.callback_payload;
    }
    if current.name == "Tool" && next.name != "Tool" {
        current.name = next.name;
    }
    if next.request_round_index.is_some() {
        current.request_round_index = next.request_round_index;
    }
    if next.result_round_index.is_some() {
        current.result_round_index = next.result_round_index;
    }
    if next.call_usage.is_some() {
        current.call_usage = next.call_usage;
    }
    if next.result_context_usage.is_some() {
        current.result_context_usage = next.result_context_usage;
    }
    if next.duration_ms.is_some() {
        current.duration_ms = next.duration_ms;
    }
    if next.route_trace.is_some() {
        current.route_trace = next.route_trace;
    }
}

pub(super) fn with_llm_tool_callback_runtime_facts(
    llm_rounds: Value,
    runtime_facts: &HashMap<String, LlmToolCallbackRuntimeFacts>,
) -> (Value, bool) {
    let Value::Array(rounds) = llm_rounds else {
        return (llm_rounds, false);
    };
    let mut changed = false;
    let rounds = rounds
        .into_iter()
        .enumerate()
        .map(|(fallback_round_index, round)| {
            let Some(mut round_object) = round.as_object().cloned() else {
                return round;
            };
            let current_round_index = round_index(&round_object, fallback_round_index);
            let Some(tool_results) = round_object
                .get_mut("tool_results")
                .and_then(Value::as_array_mut)
            else {
                return Value::Object(round_object);
            };

            for (tool_result_index, tool_result) in tool_results.iter_mut().enumerate() {
                let Some(tool_result_object) = tool_result.as_object_mut() else {
                    continue;
                };
                if tool_result_object.contains_key("duration_ms") {
                    continue;
                }
                let id = tool_result_id(tool_result_object, current_round_index, tool_result_index);
                let Some(duration_ms) = runtime_facts.get(&id).and_then(|facts| facts.duration_ms)
                else {
                    continue;
                };

                tool_result_object.insert("duration_ms".to_string(), json!(duration_ms));
                changed = true;
            }

            Value::Object(round_object)
        })
        .collect();

    (Value::Array(rounds), changed)
}
