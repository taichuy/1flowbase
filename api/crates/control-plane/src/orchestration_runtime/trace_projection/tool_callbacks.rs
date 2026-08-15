use super::*;

pub fn merge_trace_node_run_detail(
    node_runs: &[domain::NodeRunRecord],
) -> Option<domain::NodeRunRecord> {
    if node_runs.is_empty() {
        return None;
    }

    Some(merge_node_run_group(node_runs))
}

pub(super) fn callback_tasks_for_node_run_ids(
    detail: &domain::ApplicationRunDetail,
    node_run_ids: &HashSet<Uuid>,
) -> Vec<domain::CallbackTaskRecord> {
    detail
        .callback_tasks
        .iter()
        .filter(|task| node_run_ids.contains(&task.node_run_id))
        .cloned()
        .collect()
}

pub(super) fn synthetic_tool_calls_not_in_callback_tasks(
    node_runs: &[domain::NodeRunRecord],
    tool_tasks: &[&domain::CallbackTaskRecord],
) -> Vec<serde_json::Value> {
    if tool_tasks.is_empty() {
        return tool_calls_from_node_runs(node_runs);
    }

    let callback_tool_call_keys = tool_tasks
        .iter()
        .flat_map(|task| tool_calls_from_callback_task(task))
        .map(|tool_call| tool_call_dedup_key(&tool_call))
        .collect::<HashSet<_>>();

    tool_calls_from_node_runs(node_runs)
        .into_iter()
        .filter(|tool_call| !callback_tool_call_keys.contains(&tool_call_dedup_key(tool_call)))
        .collect()
}

fn tool_call_dedup_key(tool_call: &serde_json::Value) -> String {
    tool_call
        .get("id")
        .or_else(|| tool_call.get("tool_call_id"))
        .or_else(|| tool_call.get("call_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| tool_call.to_string())
}

pub(super) fn tool_calls_from_callback_task(
    task: &domain::CallbackTaskRecord,
) -> Vec<serde_json::Value> {
    task.request_payload
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn tool_calls_from_node_runs(node_runs: &[domain::NodeRunRecord]) -> Vec<serde_json::Value> {
    let mut tool_calls = Vec::new();
    let mut seen_tool_call_ids = HashSet::<String>::new();

    for node_run in node_runs {
        for tool_call in tool_calls_from_node_payload(&node_run.output_payload)
            .into_iter()
            .chain(tool_calls_from_node_debug_payload(&node_run.debug_payload))
            .chain(tool_calls_from_visible_internal_route_traces(
                &node_run.debug_payload,
            ))
        {
            let tool_call_id = tool_call
                .get("id")
                .or_else(|| tool_call.get("tool_call_id"))
                .or_else(|| tool_call.get("call_id"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    legacy_locator_component(
                        "node_run_tool_call",
                        &node_run.id.to_string(),
                        &tool_call,
                    )
                });

            if seen_tool_call_ids.insert(tool_call_id) {
                tool_calls.push(tool_call);
            }
        }
    }

    tool_calls
}

fn tool_calls_from_node_payload(payload: &serde_json::Value) -> Vec<serde_json::Value> {
    payload
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn tool_calls_from_node_debug_payload(payload: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(llm_rounds) = payload.get("llm_rounds") else {
        return Vec::new();
    };
    if let Some(rounds) = llm_rounds.as_array() {
        return rounds
            .iter()
            .filter_map(|round| round.get("assistant"))
            .filter_map(|assistant| assistant.get("tool_calls"))
            .filter_map(serde_json::Value::as_array)
            .flatten()
            .cloned()
            .collect();
    }

    llm_rounds
        .get("tool_callbacks")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn tool_calls_from_visible_internal_route_traces(
    payload: &serde_json::Value,
) -> Vec<serde_json::Value> {
    payload
        .get("visible_internal_llm_tool_trace")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|trace| {
            let tool_call_id = trace
                .get("tool_call_id")
                .or_else(|| trace.get("id"))
                .or_else(|| trace.get("call_id"))
                .and_then(serde_json::Value::as_str)?;
            let mut tool_call = serde_json::Map::new();
            tool_call.insert("id".to_string(), serde_json::json!(tool_call_id));
            tool_call.insert("tool_call_id".to_string(), serde_json::json!(tool_call_id));
            if let Some(tool_name) = trace
                .get("tool_name")
                .or_else(|| trace.get("name"))
                .or_else(|| trace.get("route_alias"))
                .or_else(|| trace.get("fusion_alias"))
                .and_then(serde_json::Value::as_str)
            {
                tool_call.insert("name".to_string(), serde_json::json!(tool_name));
            }
            if let Some(arguments) = trace.get("arguments") {
                tool_call.insert("arguments".to_string(), arguments.clone());
            }
            tool_call.insert(
                "source_kind".to_string(),
                serde_json::json!("visible_internal_llm_tool_trace"),
            );

            Some(serde_json::Value::Object(tool_call))
        })
        .collect()
}

pub(super) fn tool_result_for_call(
    task: &domain::CallbackTaskRecord,
    tool_call_id: &str,
) -> Option<serde_json::Value> {
    task.response_payload
        .as_ref()
        .and_then(|payload| payload.get("tool_results"))
        .and_then(serde_json::Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| tool_payload_matches_call_id(item, tool_call_id))
                .cloned()
        })
}

pub(super) fn tool_result_for_call_from_node_runs(
    node_runs: &[domain::NodeRunRecord],
    tool_call_id: &str,
) -> Option<serde_json::Value> {
    node_runs.iter().rev().find_map(|node_run| {
        let result = tool_result_for_call_from_debug_payload(&node_run.debug_payload, tool_call_id);
        let event = runtime_internal_tool_event_for_call(&node_run.debug_payload, tool_call_id);
        merge_tool_result_and_runtime_event(result, event)
    })
}

fn tool_result_for_call_from_debug_payload(
    debug_payload: &serde_json::Value,
    tool_call_id: &str,
) -> Option<serde_json::Value> {
    let llm_rounds = debug_payload.get("llm_rounds")?;
    if let Some(callbacks) = llm_rounds
        .get("tool_callbacks")
        .and_then(serde_json::Value::as_array)
    {
        return callbacks
            .iter()
            .find(|callback| tool_payload_matches_call_id(callback, tool_call_id))
            .cloned();
    }
    let rounds = llm_rounds.as_array()?;

    for round in rounds.iter().rev() {
        let Some(tool_results) = round
            .get("tool_results")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };

        for tool_result in tool_results.iter().rev() {
            if tool_payload_matches_call_id(tool_result, tool_call_id) {
                return Some(tool_result.clone());
            }
        }
    }

    None
}

fn runtime_internal_tool_event_for_call(
    debug_payload: &serde_json::Value,
    tool_call_id: &str,
) -> Option<serde_json::Value> {
    debug_payload
        .get("runtime_internal_tool_events")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|event| tool_payload_matches_call_id(event, tool_call_id))
        .cloned()
}

fn merge_tool_result_and_runtime_event(
    result: Option<serde_json::Value>,
    event: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match (result, event) {
        (Some(mut result), Some(event)) => {
            if let (Some(result), Some(event)) = (result.as_object_mut(), event.as_object()) {
                for (key, value) in event {
                    if key == "execution_status"
                        && result.get(key).and_then(serde_json::Value::as_str) == Some("unknown")
                    {
                        result.insert(key.clone(), value.clone());
                    } else {
                        result.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                }
            }
            Some(result)
        }
        (Some(result), None) => Some(result),
        (None, Some(event)) => Some(event),
        (None, None) => None,
    }
}

fn tool_payload_matches_call_id(payload: &serde_json::Value, tool_call_id: &str) -> bool {
    payload
        .get("tool_call_id")
        .or_else(|| payload.get("id"))
        .or_else(|| payload.get("call_id"))
        .and_then(serde_json::Value::as_str)
        == Some(tool_call_id)
}

pub(super) fn route_trace_for_tool_call(
    parent_node_runs: &[domain::NodeRunRecord],
    tool_call_id: &str,
) -> Option<serde_json::Value> {
    parent_node_runs
        .iter()
        .filter_map(|node_run| {
            node_run
                .debug_payload
                .get("visible_internal_llm_tool_trace")
                .and_then(serde_json::Value::as_array)
        })
        .flatten()
        .find(|trace| tool_payload_matches_call_id(trace, tool_call_id))
        .cloned()
}

pub(super) fn route_trace_node_kind(route_trace: &serde_json::Value) -> &'static str {
    match route_trace
        .get("route_kind")
        .or_else(|| route_trace.get("kind"))
        .and_then(serde_json::Value::as_str)
    {
        Some("fusion") | Some("visible_internal_llm_tool_fusion") => "fusion",
        _ => "route",
    }
}

pub(super) fn route_trace_locator_component(
    route_trace: &serde_json::Value,
    node_kind: &str,
    order_key: &str,
) -> String {
    route_trace
        .get("route_ref")
        .or_else(|| route_trace.get("route_id"))
        .or_else(|| route_trace.get("fusion_ref"))
        .or_else(|| route_trace.get("fusion_id"))
        .or_else(|| route_trace.get("tool_call_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| legacy_locator_component(node_kind, order_key, route_trace))
}

pub(super) fn route_trace_branch_traces(route_trace: &serde_json::Value) -> Vec<serde_json::Value> {
    route_trace
        .get("branch_traces")
        .or_else(|| route_trace.get("branch_summaries"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn route_trace_node_alias(route_trace: &serde_json::Value, node_kind: &str) -> String {
    route_trace
        .get("route_alias")
        .or_else(|| route_trace.get("fusion_alias"))
        .or_else(|| route_trace.get("tool_name"))
        .or_else(|| route_trace.get("route_model"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            if node_kind == "fusion" {
                "Fusion".to_string()
            } else {
                "Route".to_string()
            }
        })
}

pub(super) fn branch_locator_component(
    branch_trace: &serde_json::Value,
    order_key: &str,
) -> String {
    branch_trace
        .get("branch_ref")
        .or_else(|| branch_trace.get("branch_id"))
        .or_else(|| branch_trace.get("node_run_id"))
        .or_else(|| branch_trace.get("node_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| legacy_locator_component("branch", order_key, branch_trace))
}

pub(super) fn branch_trace_node_alias(branch_trace: &serde_json::Value) -> String {
    branch_trace
        .get("node_alias")
        .or_else(|| branch_trace.get("branch_alias"))
        .or_else(|| branch_trace.get("node_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Branch".to_string())
}

pub(super) fn route_trace_status(route_trace: &serde_json::Value) -> String {
    let status = route_trace
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("completed")
        .to_string();

    if status == "failed" && route_trace_has_interception_error(route_trace) {
        return "intercepted".to_string();
    }

    status
}

fn route_trace_has_interception_error(route_trace: &serde_json::Value) -> bool {
    if route_trace
        .get("status")
        .and_then(serde_json::Value::as_str)
        == Some("intercepted")
    {
        return true;
    }

    route_trace
        .get("events")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|event| event.get("error_payload"))
        .any(error_payload_is_interception)
}

fn error_payload_is_interception(error_payload: &serde_json::Value) -> bool {
    let Some(error_object) = error_payload.as_object() else {
        return false;
    };
    if error_object
        .get("error_code")
        .and_then(serde_json::Value::as_str)
        .is_some_and(is_visible_internal_llm_tool_interception_error_code)
    {
        return true;
    }

    ["details", "error_payload"]
        .into_iter()
        .filter_map(|key| error_object.get(key))
        .any(error_payload_is_interception)
}

fn is_visible_internal_llm_tool_interception_error_code(error_code: &str) -> bool {
    matches!(
        error_code,
        "visible_internal_llm_tool_media_unavailable"
            | "visible_internal_llm_tool_mixed_round_callback_unavailable"
            | "visible_internal_llm_tool_external_callback_forbidden"
    )
}

pub(super) fn branch_trace_status(branch_trace: &serde_json::Value) -> String {
    branch_trace
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("completed")
        .to_string()
}

pub(super) fn route_trace_metrics_payload(route_trace: &serde_json::Value) -> serde_json::Value {
    route_trace
        .get("metrics_payload")
        .or_else(|| route_trace.get("usage"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

fn payload_object_field<'a>(
    payload: &'a serde_json::Value,
    field_name: &str,
) -> Option<&'a serde_json::Value> {
    payload
        .get(field_name)
        .filter(|field| field.as_object().is_some())
}

fn route_trace_usage_payload(route_trace: &serde_json::Value) -> Option<&serde_json::Value> {
    payload_object_field(route_trace, "call_usage")
        .or_else(|| payload_object_field(route_trace, "usage"))
        .or_else(|| {
            let metrics_payload = route_trace
                .get("metrics_payload")
                .filter(|field| field.as_object().is_some())?;

            payload_object_field(metrics_payload, "usage").or(Some(metrics_payload))
        })
}

fn tool_callback_call_usage_payload<'a>(
    tool_call: &'a serde_json::Value,
    tool_result: Option<&'a serde_json::Value>,
    route_trace: Option<&'a serde_json::Value>,
) -> Option<&'a serde_json::Value> {
    payload_object_field(tool_call, "call_usage")
        .or_else(|| tool_result.and_then(|result| payload_object_field(result, "call_usage")))
        .or_else(|| route_trace.and_then(route_trace_usage_payload))
}

pub(super) fn tool_callback_metrics_payload(
    tool_call: &serde_json::Value,
    tool_result: Option<&serde_json::Value>,
    route_trace: Option<&serde_json::Value>,
) -> serde_json::Value {
    tool_callback_call_usage_payload(tool_call, tool_result, route_trace)
        .map(|usage| serde_json::json!({ "usage": usage }))
        .unwrap_or_else(|| serde_json::json!({}))
}

fn callback_status(task: &domain::CallbackTaskRecord) -> &'static str {
    if task.response_payload.is_some() {
        "returned"
    } else {
        "waiting_callback"
    }
}

pub(super) fn tool_result_execution_status(
    tool_result: Option<&serde_json::Value>,
) -> Option<String> {
    tool_result
        .and_then(|value| value.get("execution_status"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

pub(super) fn tool_result_duration_ms(tool_result: Option<&serde_json::Value>) -> Option<i64> {
    tool_result
        .and_then(|value| value.get("duration_ms"))
        .and_then(serde_json::Value::as_i64)
}

pub(super) fn tool_result_timestamp(
    tool_result: Option<&serde_json::Value>,
    field: &str,
) -> Option<OffsetDateTime> {
    serde_json::from_value(tool_result?.get(field)?.clone()).ok()
}

fn route_trace_execution_status(route_trace: Option<&serde_json::Value>) -> Option<String> {
    match route_trace_status(route_trace?).as_str() {
        "intercepted" => Some("intercepted".to_string()),
        "failed" => Some("failed".to_string()),
        "succeeded" | "returned_to_main" | "route_completed" => Some("succeeded".to_string()),
        _ => None,
    }
}

pub(super) fn route_trace_tool_callback_status(
    route_trace: Option<&serde_json::Value>,
) -> Option<String> {
    match route_trace_status(route_trace?).as_str() {
        "intercepted" => Some("intercepted".to_string()),
        "failed" => Some("failed".to_string()),
        _ => None,
    }
}

pub(super) fn tool_callback_content_payload(
    task: Option<&domain::CallbackTaskRecord>,
    tool_call_id: &str,
    tool_name: &str,
    tool_call: &serde_json::Value,
    tool_result: Option<&serde_json::Value>,
    route_trace: Option<&serde_json::Value>,
) -> serde_json::Value {
    let call_usage = tool_callback_call_usage_payload(tool_call, tool_result, route_trace).cloned();
    let result_context_usage = tool_result
        .and_then(|result| result.get("result_context_usage"))
        .cloned();
    let callback_status = task.map(callback_status).unwrap_or_else(|| {
        if tool_result.is_some() {
            "returned"
        } else {
            "waiting_callback"
        }
    });

    serde_json::json!({
        "id": tool_call_id,
        "name": tool_name,
        "callback_task_id": task.map(|task| task.id),
        "tool_call_id": tool_call_id,
        "callback_status": callback_status,
        "execution_status": route_trace_execution_status(route_trace)
            .or_else(|| tool_result_execution_status(tool_result)),
        "request_payload": tool_call,
        "callback_payload": tool_result,
        "parsed_result": tool_result,
        "call_usage": call_usage,
        "result_context_usage": result_context_usage,
        "duration_ms": task.and_then(|task| trace_node_duration_ms(task.created_at, task.completed_at)),
        "route_trace": route_trace,
        "tool_call": tool_call,
        "tool_result": tool_result,
    })
}

pub(super) fn callback_task_trace_node_status(task: &domain::CallbackTaskRecord) -> String {
    match task.status {
        domain::CallbackTaskStatus::Pending => domain::NodeRunStatus::WaitingCallback,
        domain::CallbackTaskStatus::Completed => domain::NodeRunStatus::Succeeded,
        domain::CallbackTaskStatus::Cancelled => domain::NodeRunStatus::Cancelled,
    }
    .as_str()
    .to_string()
}

pub(super) fn tool_group_status(tool_tasks: &[&domain::CallbackTaskRecord]) -> String {
    if tool_tasks
        .iter()
        .any(|task| task.status == domain::CallbackTaskStatus::Pending)
    {
        return domain::NodeRunStatus::WaitingCallback.as_str().to_string();
    }

    if tool_tasks
        .iter()
        .any(|task| task.status == domain::CallbackTaskStatus::Cancelled)
    {
        return domain::NodeRunStatus::Cancelled.as_str().to_string();
    }

    domain::NodeRunStatus::Succeeded.as_str().to_string()
}

pub(super) fn node_run_group_content(
    trace_node_id: Uuid,
    node_runs: &[domain::NodeRunRecord],
    detail: &domain::ApplicationRunDetail,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let node_run_ids = node_runs
        .iter()
        .map(|node_run| node_run.id)
        .collect::<HashSet<_>>();
    let checkpoints: Vec<&domain::CheckpointRecord> = detail
        .checkpoints
        .iter()
        .filter(|checkpoint| {
            checkpoint
                .node_run_id
                .is_some_and(|node_run_id| node_run_ids.contains(&node_run_id))
        })
        .collect();
    let events: Vec<&domain::RunEventRecord> = detail
        .events
        .iter()
        .filter(|event| {
            event
                .node_run_id
                .is_some_and(|node_run_id| node_run_ids.contains(&node_run_id))
        })
        .collect();
    let primary_node_run = &node_runs[0];
    let source_ref_values = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "source_kind": "node_run",
                "source_locator": node_run.id
            })
        })
        .collect::<Vec<_>>();
    let source_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "source_kind": "node_run",
                "source_locator": node_run.id
            })
        })
        .collect::<Vec<_>>();
    let node_run_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "detail_kind": "node_run",
                "source_kind": "node_run",
                "source_locator": node_run.id,
                "count": 1
            })
        })
        .collect::<Vec<_>>();
    let detail_refs = serde_json::json!([
        {
            "detail_ref_id": "node_run",
            "detail_kind": "node_run",
            "source_kind": "node_run",
            "source_locator": primary_node_run.id,
            "count": node_runs.len()
        },
        {
            "detail_ref_id": "checkpoints",
            "detail_kind": "checkpoints",
            "source_kind": "flow_run_checkpoints",
            "source_locator": trace_node_id,
            "count": checkpoints.len()
        },
        {
            "detail_ref_id": "events",
            "detail_kind": "events",
            "source_kind": "flow_run_events",
            "source_locator": trace_node_id,
            "count": events.len()
        }
    ]);

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": node_runs.len(),
                "checkpoint_count": checkpoints.len(),
                "event_count": events.len(),
                "node_run_ids": node_runs.iter().map(|node_run| node_run.id).collect::<Vec<_>>()
            },
            "source_refs": source_ref_values,
            "detail_refs": detail_refs,
            "node_run_refs": node_run_refs
        }),
        source_refs: serde_json::Value::Array(source_refs),
    })
}
