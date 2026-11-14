use super::llm_tool_callbacks::{record_string_field, record_value_field};
use super::*;

pub fn enrich_application_run_detail_visible_internal_llm_route_traces(
    mut detail: domain::ApplicationRunDetail,
    runtime_events: &[domain::RuntimeEventRecord],
) -> domain::ApplicationRunDetail {
    enrich_node_runs_visible_internal_llm_route_traces(&mut detail.node_runs, runtime_events);

    for trace in &mut detail.stitched_trace {
        enrich_node_runs_visible_internal_llm_route_traces(
            &mut trace.node_runs,
            &trace.runtime_events,
        );
    }

    detail
}

pub fn enrich_application_run_trace_projection_source_visible_internal_llm_route_traces(
    mut source: domain::ApplicationRunTraceProjectionSource,
    runtime_events: &[domain::RuntimeEventRecord],
) -> domain::ApplicationRunTraceProjectionSource {
    enrich_node_runs_visible_internal_llm_route_traces(&mut source.node_runs, runtime_events);
    for trace in &mut source.stitched_trace {
        enrich_node_runs_visible_internal_llm_route_traces(
            &mut trace.node_runs,
            &trace.runtime_events,
        );
    }
    source
}

pub fn enrich_node_last_run_visible_internal_llm_route_traces(
    mut last_run: domain::NodeLastRun,
    runtime_events: &[domain::RuntimeEventRecord],
) -> domain::NodeLastRun {
    let runtime_events_by_node_run_id = visible_internal_llm_tool_runtime_events_by_node_run_id(
        std::slice::from_ref(&last_run.node_run),
        runtime_events,
    );
    let debug_payload = runtime_events_by_node_run_id
        .get(&last_run.node_run.id)
        .map(|runtime_events| {
            with_runtime_visible_internal_llm_tool_events(
                last_run.node_run.debug_payload.clone(),
                runtime_events,
            )
        })
        .unwrap_or_else(|| last_run.node_run.debug_payload.clone());
    last_run.node_run.debug_payload =
        with_inline_visible_internal_llm_tool_trace_index_with_main_output(
            debug_payload,
            Some(&last_run.node_run.output_payload),
        );

    last_run
}

fn enrich_node_runs_visible_internal_llm_route_traces(
    node_runs: &mut [domain::NodeRunRecord],
    runtime_events: &[domain::RuntimeEventRecord],
) {
    let runtime_events_by_node_run_id =
        visible_internal_llm_tool_runtime_events_by_node_run_id(node_runs, runtime_events);
    let branch_node_run_payloads = visible_internal_llm_tool_branch_node_run_payloads(node_runs);

    for node_run in node_runs {
        let debug_payload = runtime_events_by_node_run_id
            .get(&node_run.id)
            .map(|runtime_events| {
                with_runtime_visible_internal_llm_tool_events(
                    node_run.debug_payload.clone(),
                    runtime_events,
                )
            })
            .unwrap_or_else(|| node_run.debug_payload.clone());
        node_run.debug_payload =
            with_inline_visible_internal_llm_tool_trace_index_with_branch_node_runs(
                debug_payload,
                Some(&node_run.output_payload),
                &branch_node_run_payloads,
            );
    }
}

fn visible_internal_llm_tool_branch_node_run_payloads(
    node_runs: &[domain::NodeRunRecord],
) -> Vec<VisibleInternalLlmToolBranchNodeRunPayload> {
    node_runs
        .iter()
        .map(|node_run| VisibleInternalLlmToolBranchNodeRunPayload {
            node_run_id: node_run.id.to_string(),
            node_id: node_run.node_id.clone(),
            node_alias: node_run.node_alias.clone(),
            node_type: node_run.node_type.clone(),
            input_payload: node_run.input_payload.clone(),
            output_payload: node_run.output_payload.clone(),
            metrics_payload: node_run.metrics_payload.clone(),
            debug_payload: node_run.debug_payload.clone(),
        })
        .collect()
}

fn visible_internal_llm_tool_runtime_events_by_node_run_id(
    node_runs: &[domain::NodeRunRecord],
    runtime_events: &[domain::RuntimeEventRecord],
) -> HashMap<Uuid, Vec<Value>> {
    let mut latest_node_run_id_by_node_id = HashMap::<String, Uuid>::new();
    let mut node_run_ids = std::collections::HashSet::<Uuid>::new();
    for node_run in node_runs {
        latest_node_run_id_by_node_id.insert(node_run.node_id.clone(), node_run.id);
        node_run_ids.insert(node_run.id);
    }

    let mut events_by_node_run_id = HashMap::<Uuid, Vec<Value>>::new();
    for event in runtime_events {
        let Some(payload) = visible_internal_llm_tool_runtime_event_payload(event) else {
            continue;
        };
        let explicit_node_run_id =
            visible_internal_llm_tool_runtime_event_node_run_id(event, &payload);
        let owner_node_run_id = match explicit_node_run_id {
            Some(node_run_id) if node_run_ids.contains(&node_run_id) => Some(node_run_id),
            Some(_) => None,
            None => visible_internal_llm_tool_runtime_event_owner_node_id(&payload)
                .and_then(|node_id| latest_node_run_id_by_node_id.get(node_id).copied()),
        };
        let Some(owner_node_run_id) = owner_node_run_id else {
            continue;
        };

        events_by_node_run_id
            .entry(owner_node_run_id)
            .or_default()
            .push(payload);
    }

    events_by_node_run_id
}

fn visible_internal_llm_tool_runtime_event_payload(
    event: &domain::RuntimeEventRecord,
) -> Option<Value> {
    if !event.event_type.starts_with("visible_internal_llm_tool_") {
        return None;
    }

    let mut payload = event.payload.as_object()?.clone();
    payload.insert("event_type".to_string(), json!(event.event_type));
    if !payload.contains_key("node_run_id") {
        if let Some(node_run_id) = event.node_run_id {
            payload.insert("node_run_id".to_string(), json!(node_run_id.to_string()));
        }
    }

    Some(Value::Object(payload))
}

fn visible_internal_llm_tool_runtime_event_owner_node_id(payload: &Value) -> Option<&str> {
    payload
        .get("main_node_id")
        .or_else(|| payload.get("node_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn visible_internal_llm_tool_runtime_event_node_run_id(
    event: &domain::RuntimeEventRecord,
    payload: &Value,
) -> Option<Uuid> {
    payload
        .get("node_run_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .or(event.node_run_id)
}

fn with_runtime_visible_internal_llm_tool_events(
    mut payload: Value,
    runtime_events: &[Value],
) -> Value {
    if runtime_events.is_empty() {
        return payload;
    }
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };

    if !object.contains_key("visible_internal_llm_tool_events") {
        object.insert(
            "visible_internal_llm_tool_events".to_string(),
            Value::Array(runtime_events.to_vec()),
        );
    }

    with_synthetic_visible_internal_llm_tool_rounds(payload, runtime_events)
}

fn with_synthetic_visible_internal_llm_tool_rounds(
    mut payload: Value,
    runtime_events: &[Value],
) -> Value {
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    if object.contains_key("llm_rounds") {
        return payload;
    }

    let rounds = synthetic_visible_internal_llm_tool_rounds(runtime_events);
    if rounds.is_empty() {
        return payload;
    }

    object.insert("llm_rounds".to_string(), Value::Array(rounds));
    payload
}

fn synthetic_visible_internal_llm_tool_rounds(runtime_events: &[Value]) -> Vec<Value> {
    let mut calls_by_id = std::collections::BTreeMap::<String, Value>::new();
    let mut results_by_id = std::collections::BTreeMap::<String, Value>::new();

    for event in runtime_events {
        let Some(event_object) = event.as_object() else {
            continue;
        };
        let Some(tool_call_id) =
            record_string_field(event_object, &["tool_call_id", "id", "call_id"])
        else {
            continue;
        };
        let tool_name = record_string_field(event_object, &["tool_name", "name"])
            .unwrap_or_else(|| "Tool".to_string());

        calls_by_id.entry(tool_call_id.clone()).or_insert_with(|| {
            let mut call = Map::new();
            call.insert("id".to_string(), json!(tool_call_id));
            call.insert("name".to_string(), json!(tool_name));
            call.insert("type".to_string(), json!("visible_internal_llm_tool"));
            if let Some(arguments) = event_object.get("arguments") {
                call.insert("arguments".to_string(), arguments.clone());
            }
            Value::Object(call)
        });

        match event_object
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "visible_internal_llm_tool_completed" => {
                results_by_id.insert(
                    tool_call_id.clone(),
                    synthetic_visible_internal_llm_tool_result(
                        &tool_call_id,
                        &tool_name,
                        event_object,
                        false,
                    ),
                );
            }
            "visible_internal_llm_tool_failed" => {
                results_by_id.insert(
                    tool_call_id.clone(),
                    synthetic_visible_internal_llm_tool_result(
                        &tool_call_id,
                        &tool_name,
                        event_object,
                        true,
                    ),
                );
            }
            _ => {}
        }
    }

    if calls_by_id.is_empty() {
        return Vec::new();
    }

    let mut rounds = vec![json!({
        "round_index": 0,
        "assistant": {
            "role": "assistant",
            "tool_calls": calls_by_id.into_values().collect::<Vec<_>>()
        }
    })];

    if !results_by_id.is_empty() {
        rounds.push(json!({
            "round_index": 1,
            "tool_results": results_by_id.into_values().collect::<Vec<_>>()
        }));
    }

    rounds
}

fn synthetic_visible_internal_llm_tool_result(
    tool_call_id: &str,
    tool_name: &str,
    event_object: &Map<String, Value>,
    is_error: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("role".to_string(), json!("tool"));
    result.insert("tool_call_id".to_string(), json!(tool_call_id));
    result.insert("name".to_string(), json!(tool_name));
    result.insert("is_error".to_string(), json!(is_error));

    if is_error {
        if let Some(error_payload) = event_object.get("error_payload") {
            result.insert("error".to_string(), error_payload.clone());
            result.insert("content".to_string(), error_payload.clone());
        }
    } else if let Some(content) = record_value_field(
        event_object,
        &[
            "content",
            "output",
            "output_payload",
            "result",
            "response_payload",
        ],
    ) {
        result.insert("content".to_string(), content);
    } else {
        result.insert("content".to_string(), json!(null));
    }

    Value::Object(result)
}

fn with_inline_visible_internal_llm_tool_trace_index_with_main_output(
    payload: Value,
    main_resume_output_fallback: Option<&Value>,
) -> Value {
    with_inline_visible_internal_llm_tool_trace_index_with_branch_node_runs(
        payload,
        main_resume_output_fallback,
        &[],
    )
}

fn with_inline_visible_internal_llm_tool_trace_index_with_branch_node_runs(
    mut payload: Value,
    main_resume_output_fallback: Option<&Value>,
    branch_node_run_payloads: &[VisibleInternalLlmToolBranchNodeRunPayload],
) -> Value {
    let Some(object) = payload.as_object() else {
        return payload;
    };
    if object.contains_key("visible_internal_llm_tool_trace") {
        return payload;
    }
    let runtime_events = object
        .get("visible_internal_llm_tool_events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    payload = with_synthetic_visible_internal_llm_tool_rounds(payload, &runtime_events);

    let traces = collect_visible_internal_llm_tool_route_traces_with_branch_node_runs(
        &payload,
        main_resume_output_fallback,
        branch_node_run_payloads,
    );
    if traces.is_empty() {
        return payload;
    }

    let summaries = traces
        .into_iter()
        .map(|trace| {
            if branch_node_run_payloads.is_empty() {
                trace.inline_summary_payload()
            } else {
                trace.inline_summary_with_branch_traces_payload()
            }
        })
        .collect::<Vec<_>>();
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "visible_internal_llm_tool_trace".to_string(),
            Value::Array(summaries),
        );
    }

    payload
}
