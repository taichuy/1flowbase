use super::*;

pub(super) fn json_object_has_keys(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|object| !object.is_empty())
}

pub(super) fn first_non_empty_json(
    node_runs: &[domain::NodeRunRecord],
    selector: impl Fn(&domain::NodeRunRecord) -> &serde_json::Value,
) -> serde_json::Value {
    node_runs
        .iter()
        .find_map(|node_run| {
            let payload = selector(node_run);
            json_object_has_keys(payload).then(|| payload.clone())
        })
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(super) fn last_non_empty_json(
    node_runs: &[domain::NodeRunRecord],
    selector: impl Fn(&domain::NodeRunRecord) -> &serde_json::Value,
) -> serde_json::Value {
    node_runs
        .iter()
        .rev()
        .find_map(|node_run| {
            let payload = selector(node_run);
            json_object_has_keys(payload).then(|| payload.clone())
        })
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(super) fn trace_node_group_status(
    node_runs: &[domain::NodeRunRecord],
) -> domain::NodeRunStatus {
    if node_runs
        .iter()
        .any(|node_run| node_run.status == domain::NodeRunStatus::Failed)
    {
        return domain::NodeRunStatus::Failed;
    }

    if node_runs
        .iter()
        .any(|node_run| node_run.status == domain::NodeRunStatus::WaitingHuman)
    {
        return domain::NodeRunStatus::WaitingHuman;
    }

    if node_runs
        .iter()
        .any(|node_run| node_run.status == domain::NodeRunStatus::WaitingCallback)
    {
        return domain::NodeRunStatus::WaitingCallback;
    }

    if node_runs.iter().any(|node_run| {
        matches!(
            node_run.status,
            domain::NodeRunStatus::Running
                | domain::NodeRunStatus::Streaming
                | domain::NodeRunStatus::Retrying
                | domain::NodeRunStatus::WaitingTool
        )
    }) {
        return domain::NodeRunStatus::Running;
    }

    if node_runs
        .iter()
        .all(|node_run| node_run.status == domain::NodeRunStatus::Succeeded)
    {
        return domain::NodeRunStatus::Succeeded;
    }

    node_runs
        .last()
        .map(|node_run| node_run.status)
        .unwrap_or(domain::NodeRunStatus::Running)
}

pub(super) fn trace_node_group_finished_at(
    node_runs: &[domain::NodeRunRecord],
) -> Option<OffsetDateTime> {
    if node_runs
        .iter()
        .any(|node_run| node_run.finished_at.is_none())
    {
        return None;
    }

    node_runs.last().and_then(|node_run| node_run.finished_at)
}

pub(super) fn trace_node_group_duration_ms(node_runs: &[domain::NodeRunRecord]) -> Option<i64> {
    let durations: Vec<i64> = node_runs
        .iter()
        .filter_map(|node_run| trace_node_duration_ms(node_run.started_at, node_run.finished_at))
        .collect();

    if durations.is_empty() {
        return None;
    }

    Some(
        durations
            .into_iter()
            .fold(0_i64, |total, duration| total.saturating_add(duration)),
    )
}

pub(super) fn merge_debug_payloads(node_runs: &[domain::NodeRunRecord]) -> serde_json::Value {
    let mut merged = serde_json::Map::new();
    let mut llm_rounds = Vec::<serde_json::Value>::new();
    let mut visible_internal_route_traces = Vec::<serde_json::Value>::new();
    let mut visible_internal_route_events = Vec::<serde_json::Value>::new();

    for node_run in node_runs {
        let Some(debug_payload) = node_run.debug_payload.as_object() else {
            continue;
        };

        for (key, value) in debug_payload {
            match key.as_str() {
                "llm_rounds" => {
                    if let Some(items) = value.as_array() {
                        llm_rounds.extend(items.iter().cloned());
                    } else if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                "visible_internal_llm_tool_trace" => {
                    if let Some(items) = value.as_array() {
                        visible_internal_route_traces.extend(items.iter().cloned());
                    } else if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                "visible_internal_llm_tool_events" => {
                    if let Some(items) = value.as_array() {
                        visible_internal_route_events.extend(items.iter().cloned());
                    } else if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                _ => {
                    if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }

    if !llm_rounds.is_empty() {
        merged.insert(
            "llm_rounds".to_string(),
            serde_json::Value::Array(llm_rounds),
        );
    }
    if !visible_internal_route_traces.is_empty() {
        merged.insert(
            "visible_internal_llm_tool_trace".to_string(),
            serde_json::Value::Array(visible_internal_route_traces),
        );
    }
    if !visible_internal_route_events.is_empty() {
        merged.insert(
            "visible_internal_llm_tool_events".to_string(),
            serde_json::Value::Array(visible_internal_route_events),
        );
    }

    serde_json::Value::Object(merged)
}

pub(super) fn merge_metric_usage_value(
    node_runs: &[domain::NodeRunRecord],
    usage_key: &str,
) -> Option<i64> {
    let mut total = None;

    for node_run in node_runs {
        if let Some(value) = node_run
            .metrics_payload
            .get("usage")
            .and_then(|usage| usage.get(usage_key))
            .and_then(serde_json::Value::as_i64)
        {
            total = Some(total.unwrap_or(0_i64).saturating_add(value));
        }
    }

    total
}

pub(super) fn merge_metrics_payloads(node_runs: &[domain::NodeRunRecord]) -> serde_json::Value {
    let mut usage = serde_json::Map::new();

    for key in [
        "total_tokens",
        "input_tokens",
        "output_tokens",
        "input_cache_hit_tokens",
        "cache_read_tokens",
    ] {
        if let Some(value) = merge_metric_usage_value(node_runs, key) {
            usage.insert(key.to_string(), serde_json::json!(value));
        }
    }

    if usage.is_empty() {
        return last_non_empty_json(node_runs, |node_run| &node_run.metrics_payload);
    }

    serde_json::json!({ "usage": usage })
}

pub(super) fn merge_node_run_group(node_runs: &[domain::NodeRunRecord]) -> domain::NodeRunRecord {
    let mut merged = node_runs[0].clone();

    if node_runs.len() == 1 {
        return merged;
    }

    merged.status = trace_node_group_status(node_runs);
    merged.finished_at = trace_node_group_finished_at(node_runs);
    merged.input_payload = first_non_empty_json(node_runs, |node_run| &node_run.input_payload);
    merged.output_payload = last_non_empty_json(node_runs, |node_run| &node_run.output_payload);
    merged.error_payload = node_runs
        .iter()
        .rev()
        .find_map(|node_run| node_run.error_payload.clone());
    merged.metrics_payload = merge_metrics_payloads(node_runs);
    merged.debug_payload = merge_debug_payloads(node_runs);

    merged
}

pub(super) fn tool_call_id(tool_call: &serde_json::Value) -> Option<&str> {
    tool_call
        .get("id")
        .or_else(|| tool_call.get("tool_call_id"))
        .or_else(|| tool_call.get("call_id"))
        .and_then(serde_json::Value::as_str)
}

pub(super) fn subagent_trace_matches_tool_call(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    task: &domain::CallbackTaskRecord,
    tool_call: &serde_json::Value,
) -> bool {
    subagent_trace.parent_callback_task_id == task.id
        && tool_call_id(tool_call) == Some(subagent_trace.parent_tool_call_id.as_str())
}

pub(super) fn linked_subagent_trace_for_tool_call<'a>(
    detail: &'a domain::ApplicationRunDetail,
    task: &domain::CallbackTaskRecord,
    tool_call: &serde_json::Value,
) -> Option<&'a domain::ApplicationRunSubagentTrace> {
    detail
        .subagent_traces
        .iter()
        .find(|subagent_trace| subagent_trace_matches_tool_call(subagent_trace, task, tool_call))
}

pub(super) fn ordinary_tool_calls_not_linked_to_subagents<'a>(
    detail: &'a domain::ApplicationRunDetail,
    tool_tasks: &[&'a domain::CallbackTaskRecord],
) -> Vec<ToolCallProjection<'a>> {
    let mut tool_calls = Vec::new();

    for task in tool_tasks {
        for tool_call in tool_calls_from_callback_task(task) {
            if linked_subagent_trace_for_tool_call(detail, task, &tool_call).is_none() {
                tool_calls.push(ToolCallProjection { task, tool_call });
            }
        }
    }

    tool_calls
}

pub(super) fn linked_subagent_traces_for_tool_tasks<'a>(
    detail: &'a domain::ApplicationRunDetail,
    tool_tasks: &[&domain::CallbackTaskRecord],
) -> Vec<&'a domain::ApplicationRunSubagentTrace> {
    let mut subagent_traces = Vec::new();
    let mut seen_flow_runs = HashSet::new();

    for task in tool_tasks {
        for tool_call in tool_calls_from_callback_task(task) {
            if let Some(subagent_trace) =
                linked_subagent_trace_for_tool_call(detail, task, &tool_call)
            {
                if seen_flow_runs.insert(subagent_trace.source_flow_run.id) {
                    subagent_traces.push(subagent_trace);
                }
            }
        }
    }

    subagent_traces
}

pub(super) fn count_linked_subagent_tool_calls(
    detail: &domain::ApplicationRunDetail,
    tool_tasks: &[&domain::CallbackTaskRecord],
) -> usize {
    linked_subagent_traces_for_tool_tasks(detail, tool_tasks).len()
}

pub(super) fn subagent_primary_node_run_group(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
) -> Option<Vec<domain::NodeRunRecord>> {
    trace_visible_node_run_groups(&subagent_trace.node_runs)
        .into_iter()
        .find(|group| {
            group
                .first()
                .is_some_and(|node_run| node_run.node_type == "llm")
        })
}

pub(super) fn subagent_group_status(
    subagent_traces: &[&domain::ApplicationRunSubagentTrace],
) -> String {
    if subagent_traces.iter().any(|trace| {
        matches!(
            trace.source_flow_run.status,
            domain::FlowRunStatus::Failed | domain::FlowRunStatus::Cancelled
        )
    }) {
        return domain::NodeRunStatus::Failed.as_str().to_string();
    }

    if subagent_traces.iter().any(|trace| {
        matches!(
            trace.source_flow_run.status,
            domain::FlowRunStatus::Queued
                | domain::FlowRunStatus::Running
                | domain::FlowRunStatus::WaitingCallback
                | domain::FlowRunStatus::WaitingHuman
                | domain::FlowRunStatus::Paused
        )
    }) {
        return domain::NodeRunStatus::Running.as_str().to_string();
    }

    domain::NodeRunStatus::Succeeded.as_str().to_string()
}

pub(super) fn subagent_group_finished_at(
    subagent_traces: &[&domain::ApplicationRunSubagentTrace],
) -> Option<OffsetDateTime> {
    if subagent_traces
        .iter()
        .any(|trace| trace.source_flow_run.finished_at.is_none())
    {
        return None;
    }

    subagent_traces
        .iter()
        .filter_map(|trace| trace.source_flow_run.finished_at)
        .max()
}

pub(super) fn subagent_display_alias(parent_tool_call_description: Option<&str>) -> String {
    parent_tool_call_description
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Subagent".to_string())
}

pub(super) fn subagent_parent_tool_call_description(
    detail: &domain::ApplicationRunDetail,
    subagent_trace: &domain::ApplicationRunSubagentTrace,
) -> Option<String> {
    for task in &detail.callback_tasks {
        if task.id != subagent_trace.parent_callback_task_id {
            continue;
        }
        for tool_call in tool_calls_from_callback_task(task) {
            if tool_call_id(&tool_call) != Some(subagent_trace.parent_tool_call_id.as_str()) {
                continue;
            }

            return tool_call
                .get("arguments")
                .and_then(|arguments| arguments.get("description"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|description| !description.is_empty())
                .map(ToOwned::to_owned);
        }
    }

    None
}

pub(super) fn subagent_parent_agent_tool_call_debug_payload(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    description: Option<&str>,
) -> serde_json::Value {
    let mut parent_tool_call = serde_json::Map::new();
    parent_tool_call.insert(
        "callback_task_id".to_string(),
        serde_json::json!(subagent_trace.parent_callback_task_id),
    );
    parent_tool_call.insert(
        "tool_call_id".to_string(),
        serde_json::json!(subagent_trace.parent_tool_call_id.clone()),
    );
    if let Some(description) = description {
        parent_tool_call.insert("description".to_string(), serde_json::json!(description));
    }

    let mut debug_payload = serde_json::Map::new();
    debug_payload.insert(
        "parent_agent_tool_call".to_string(),
        serde_json::Value::Object(parent_tool_call),
    );

    serde_json::Value::Object(debug_payload)
}

pub(super) fn subagent_flow_run_fallback_debug_payload(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    description: Option<&str>,
) -> serde_json::Value {
    let mut debug_payload =
        subagent_parent_agent_tool_call_debug_payload(subagent_trace, description);
    let Some(debug_payload_object) = debug_payload.as_object_mut() else {
        return debug_payload;
    };
    debug_payload_object.insert(
        "source_flow_run_id".to_string(),
        serde_json::json!(subagent_trace.source_flow_run.id),
    );
    debug_payload_object.insert(
        "runtime_event_count".to_string(),
        serde_json::json!(subagent_trace.runtime_events.len()),
    );

    debug_payload
}

pub(super) fn subagent_flow_run_fallback_content(
    trace_node_id: Uuid,
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    parent_tool_call_description: Option<&str>,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let source_run = &subagent_trace.source_flow_run;
    let source_refs = serde_json::json!([{
        "source_kind": "subagent_flow_run",
        "source_locator": source_run.id,
        "source_flow_run_id": source_run.id,
        "parent_callback_task_id": subagent_trace.parent_callback_task_id,
        "parent_tool_call_id": subagent_trace.parent_tool_call_id.clone(),
    }]);
    let detail_refs = serde_json::Value::Array(Vec::new());

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": 0,
                "checkpoint_count": 0,
                "event_count": subagent_trace.events.len(),
                "node_run_ids": [],
                "source_flow_run_id": source_run.id
            },
            "source_refs": source_refs.clone(),
            "detail_refs": detail_refs,
            "input_payload": source_run.input_payload.clone(),
            "output_payload": source_run.output_payload.clone(),
            "error_payload": source_run.error_payload.clone(),
            "metrics_payload": {},
            "debug_payload": subagent_flow_run_fallback_debug_payload(
                subagent_trace,
                parent_tool_call_description
            )
        }),
        source_refs,
    })
}

pub(super) fn subagent_node_run_group_content(
    trace_node_id: Uuid,
    node_runs: &[domain::NodeRunRecord],
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    parent_tool_call_description: Option<&str>,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let primary_node_run = &node_runs[0];
    let source_ref_values = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "source_kind": "subagent_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": subagent_trace.source_flow_run.id,
                "parent_callback_task_id": subagent_trace.parent_callback_task_id,
                "parent_tool_call_id": subagent_trace.parent_tool_call_id,
            })
        })
        .collect::<Vec<_>>();
    let node_run_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "detail_kind": "node_run",
                "source_kind": "subagent_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": subagent_trace.source_flow_run.id,
                "count": 1
            })
        })
        .collect::<Vec<_>>();
    let detail_refs = serde_json::json!([
        {
            "detail_ref_id": "node_run",
            "detail_kind": "node_run",
            "source_kind": "subagent_node_run",
            "source_locator": primary_node_run.id,
            "source_flow_run_id": subagent_trace.source_flow_run.id,
            "count": node_runs.len()
        },
        {
            "detail_ref_id": "checkpoints",
            "detail_kind": "checkpoints",
            "source_kind": "subagent_flow_run_checkpoints",
            "source_locator": trace_node_id,
            "source_flow_run_id": subagent_trace.source_flow_run.id,
            "count": 0
        },
        {
            "detail_ref_id": "events",
            "detail_kind": "events",
            "source_kind": "subagent_flow_run_events",
            "source_locator": trace_node_id,
            "source_flow_run_id": subagent_trace.source_flow_run.id,
            "count": subagent_trace.events.len()
        }
    ]);

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": node_runs.len(),
                "checkpoint_count": 0,
                "event_count": subagent_trace.events.len(),
                "node_run_ids": node_runs.iter().map(|node_run| node_run.id).collect::<Vec<_>>(),
                "source_flow_run_id": subagent_trace.source_flow_run.id
            },
            "source_refs": source_ref_values.clone(),
            "detail_refs": detail_refs,
            "debug_payload": subagent_parent_agent_tool_call_debug_payload(
                subagent_trace,
                parent_tool_call_description
            ),
            "node_run_refs": node_run_refs
        }),
        source_refs: serde_json::Value::Array(source_ref_values),
    })
}

pub(super) fn stitched_node_run_group_content(
    trace_node_id: Uuid,
    node_runs: &[domain::NodeRunRecord],
    trace: &domain::ApplicationRunStitchedTrace,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let node_run_ids = node_runs
        .iter()
        .map(|node_run| node_run.id)
        .collect::<HashSet<_>>();
    let events = trace
        .events
        .iter()
        .filter(|event| {
            event
                .node_run_id
                .is_some_and(|node_run_id| node_run_ids.contains(&node_run_id))
        })
        .count();
    let source_run_id = trace.source_flow_run.id;
    let primary_node_run = &node_runs[0];
    let source_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "source_kind": "stitched_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": source_run_id
            })
        })
        .collect::<Vec<_>>();
    let node_run_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "detail_kind": "node_run",
                "source_kind": "stitched_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": source_run_id,
                "count": 1
            })
        })
        .collect::<Vec<_>>();
    let detail_refs = serde_json::json!([
        {
            "detail_ref_id": "node_run",
            "detail_kind": "node_run",
            "source_kind": "stitched_node_run",
            "source_locator": primary_node_run.id,
            "source_flow_run_id": source_run_id,
            "count": node_runs.len()
        },
        {
            "detail_ref_id": "checkpoints",
            "detail_kind": "checkpoints",
            "source_kind": "stitched_flow_run_checkpoints",
            "source_locator": trace_node_id,
            "source_flow_run_id": source_run_id,
            "count": 0
        },
        {
            "detail_ref_id": "events",
            "detail_kind": "events",
            "source_kind": "stitched_flow_run_events",
            "source_locator": trace_node_id,
            "source_flow_run_id": source_run_id,
            "count": events
        }
    ]);

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": node_runs.len(),
                "checkpoint_count": 0,
                "event_count": events,
                "node_run_ids": node_runs.iter().map(|node_run| node_run.id).collect::<Vec<_>>(),
                "source_flow_run_id": source_run_id
            },
            "source_refs": source_refs.clone(),
            "detail_refs": detail_refs,
            "node_run_refs": node_run_refs
        }),
        source_refs: serde_json::Value::Array(source_refs),
    })
}
