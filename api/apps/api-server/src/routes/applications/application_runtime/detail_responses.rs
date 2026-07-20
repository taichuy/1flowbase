fn to_node_run_response(run: domain::NodeRunRecord) -> NodeRunResponse {
    let (input_payload, output_payload) = normalize_node_run_payloads_for_logs(&run);
    let input_payload_view = node_input_payload_view(&input_payload);

    NodeRunResponse {
        id: run.id.to_string(),
        flow_run_id: run.flow_run_id.to_string(),
        node_id: run.node_id,
        node_type: run.node_type,
        node_alias: run.node_alias,
        status: run.status.as_str().to_string(),
        input_payload,
        input_payload_view,
        output_payload,
        error_payload: run.error_payload,
        metrics_payload: run.metrics_payload,
        debug_payload: run.debug_payload,
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
    }
}

fn normalize_node_run_payloads_for_logs(
    run: &domain::NodeRunRecord,
) -> (serde_json::Value, serde_json::Value) {
    if run.node_type == "start" {
        let input_payload = if run
            .input_payload
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
            && run
                .output_payload
                .as_object()
                .is_some_and(|object| !object.is_empty())
        {
            run.output_payload.clone()
        } else {
            run.input_payload.clone()
        };

        return (input_payload, serde_json::json!({}));
    }

    (run.input_payload.clone(), run.output_payload.clone())
}

fn node_input_payload_view(payload: &serde_json::Value) -> serde_json::Value {
    payload.clone()
}

#[cfg(test)]
fn decode_runtime_debug_artifact_preview(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let object = payload.as_object()?;
    if !object
        .get("__runtime_debug_artifact")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    object
        .get("preview")
        .and_then(serde_json::Value::as_str)
        .and_then(|preview| serde_json::from_str(preview).ok())
}

#[cfg(test)]
fn start_input_payload(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("node-start")
        .or_else(|| payload.get("start"))
        .unwrap_or(payload)
}

fn to_checkpoint_response(checkpoint: domain::CheckpointRecord) -> CheckpointResponse {
    CheckpointResponse {
        id: checkpoint.id.to_string(),
        flow_run_id: checkpoint.flow_run_id.to_string(),
        node_run_id: checkpoint.node_run_id.map(|value| value.to_string()),
        status: checkpoint.status,
        reason: checkpoint.reason,
        locator_payload: checkpoint.locator_payload,
        variable_snapshot: checkpoint.variable_snapshot,
        external_ref_payload: checkpoint.external_ref_payload,
        created_at: format_time(checkpoint.created_at),
    }
}

fn to_callback_task_response(task: domain::CallbackTaskRecord) -> CallbackTaskResponse {
    CallbackTaskResponse {
        id: task.id.to_string(),
        flow_run_id: task.flow_run_id.to_string(),
        node_run_id: task.node_run_id.to_string(),
        callback_kind: task.callback_kind,
        status: task.status.as_str().to_string(),
        request_payload: task.request_payload,
        response_payload: task.response_payload,
        external_ref_payload: task.external_ref_payload,
        created_at: format_time(task.created_at),
        completed_at: format_optional_time(task.completed_at),
    }
}

fn to_run_event_response(event: domain::RunEventRecord) -> RunEventResponse {
    RunEventResponse {
        id: event.id.to_string(),
        flow_run_id: event.flow_run_id.to_string(),
        node_run_id: event.node_run_id.map(|value| value.to_string()),
        sequence: event.sequence,
        event_type: event.event_type,
        payload: event.payload,
        created_at: format_time(event.created_at),
    }
}

fn to_stitched_trace_response(
    trace: domain::ApplicationRunStitchedTrace,
) -> ApplicationRunStitchedTraceResponse {
    ApplicationRunStitchedTraceResponse {
        source_flow_run: to_flow_run_response(trace.source_flow_run),
        node_runs: trace
            .node_runs
            .into_iter()
            .filter(stitched_trace_node_run_is_trace_step)
            .map(to_node_run_response)
            .collect(),
        callback_tasks: trace
            .callback_tasks
            .into_iter()
            .map(to_callback_task_response)
            .collect(),
        events: trace
            .events
            .into_iter()
            .map(to_run_event_response)
            .collect(),
    }
}

fn stitched_trace_node_run_is_trace_step(run: &domain::NodeRunRecord) -> bool {
    !matches!(run.node_type.as_str(), "start" | "answer")
}

fn is_waiting_prefix_answer_node_run(run: &domain::NodeRunRecord) -> bool {
    if run.node_type != "answer" {
        return false;
    }

    let input_marker = run
        .input_payload
        .get("presentation")
        .and_then(serde_json::Value::as_object)
        .and_then(|presentation| presentation.get("materialized_from"))
        .and_then(serde_json::Value::as_str);
    let debug_marker = run
        .debug_payload
        .get("answer_presentation")
        .and_then(serde_json::Value::as_object)
        .and_then(|presentation| presentation.get("materialized_from"))
        .and_then(serde_json::Value::as_str);

    input_marker == Some("waiting_prefix") || debug_marker == Some("waiting_prefix")
}

fn split_answer_snapshot_node_runs(
    detail: &domain::ApplicationRunDetail,
) -> (Option<domain::NodeRunRecord>, Vec<domain::NodeRunRecord>) {
    let mut answer_snapshot = None;
    let mut node_runs = Vec::new();

    for node_run in detail.node_runs.iter().cloned() {
        if is_waiting_prefix_answer_node_run(&node_run) {
            answer_snapshot = Some(node_run);
        } else {
            node_runs.push(node_run);
        }
    }

    (answer_snapshot, node_runs)
}

fn waiting_node_for_answer_snapshot(
    detail: &domain::ApplicationRunDetail,
) -> (Option<String>, Option<String>) {
    if let Some(checkpoint) = detail
        .checkpoints
        .iter()
        .rev()
        .find(|checkpoint| checkpoint.status.starts_with("waiting"))
    {
        let waiting_node_id = checkpoint
            .locator_payload
            .get("node_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        let waiting_node_run_id = checkpoint.node_run_id.map(|value| value.to_string());
        return (waiting_node_id, waiting_node_run_id);
    }

    if let Some(task) = detail
        .callback_tasks
        .iter()
        .rev()
        .find(|task| task.status == domain::CallbackTaskStatus::Pending)
    {
        let waiting_node_run_id = task.node_run_id.to_string();
        let waiting_node_id = detail
            .node_runs
            .iter()
            .find(|node_run| node_run.id == task.node_run_id)
            .map(|node_run| node_run.node_id.clone());
        return (waiting_node_id, Some(waiting_node_run_id));
    }

    (None, None)
}

fn answer_snapshot_text(output_payload: &serde_json::Value) -> Option<String> {
    output_payload
        .get("answer")
        .or_else(|| output_payload.get("text"))
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn answer_snapshot_complete(run: &domain::NodeRunRecord) -> bool {
    if let Some(complete) = run
        .input_payload
        .get("presentation")
        .and_then(|presentation| presentation.get("complete"))
        .and_then(serde_json::Value::as_bool)
    {
        return complete;
    }

    !run.debug_payload
        .get("answer_presentation")
        .and_then(|presentation| presentation.get("partial"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn to_answer_snapshot_response(
    run: &domain::NodeRunRecord,
    detail: &domain::ApplicationRunDetail,
) -> Option<AnswerSnapshotResponse> {
    let text = answer_snapshot_text(&run.output_payload)?;
    let (waiting_node_id, waiting_node_run_id) = waiting_node_for_answer_snapshot(detail);

    Some(AnswerSnapshotResponse {
        kind: "answer".to_string(),
        text,
        output_payload: run.output_payload.clone(),
        complete: answer_snapshot_complete(run),
        materialized_from: "waiting_prefix".to_string(),
        answer_node_id: run.node_id.clone(),
        answer_node_run_id: run.id.to_string(),
        waiting_node_id,
        waiting_node_run_id,
    })
}

fn flow_run_can_expose_answer_snapshot(status: &domain::FlowRunStatus) -> bool {
    matches!(
        status,
        domain::FlowRunStatus::WaitingCallback | domain::FlowRunStatus::WaitingHuman
    )
}

fn to_application_run_detail_response(
    application: &domain::ApplicationRecord,
    detail: domain::ApplicationRunDetail,
) -> ApplicationRunDetailResponse {
    let (answer_snapshot_node_run, visible_node_run_records) =
        split_answer_snapshot_node_runs(&detail);
    let answer_snapshot = if flow_run_can_expose_answer_snapshot(&detail.flow_run.status) {
        answer_snapshot_node_run
            .as_ref()
            .and_then(|node_run| to_answer_snapshot_response(node_run, &detail))
    } else {
        None
    };
    let statistics = application_run_statistics(&domain::ApplicationRunDetail {
        node_runs: visible_node_run_records.clone(),
        ..detail.clone()
    });
    let flow_run = to_flow_run_response(detail.flow_run.clone());
    let node_runs = visible_node_run_records
        .into_iter()
        .map(to_node_run_response)
        .collect::<Vec<_>>();
    let checkpoints = detail
        .checkpoints
        .clone()
        .into_iter()
        .map(to_checkpoint_response)
        .collect::<Vec<_>>();
    let callback_tasks = detail
        .callback_tasks
        .clone()
        .into_iter()
        .map(to_callback_task_response)
        .collect::<Vec<_>>();
    let events = detail
        .events
        .clone()
        .into_iter()
        .map(to_run_event_response)
        .collect::<Vec<_>>();
    let stitched_trace = detail
        .stitched_trace
        .clone()
        .into_iter()
        .map(to_stitched_trace_response)
        .collect::<Vec<_>>();
    let application_type = application.application_type.as_str().to_string();
    let invocation_context = detail.flow_run.run_mode.invocation_context(
        Some(detail.flow_run.created_by),
        detail.flow_run.authorized_account.clone(),
        detail.flow_run.api_key_id,
    );
    let run = application_logs::ApplicationRunLogResponse {
        id: detail.flow_run.id.to_string(),
        application_id: application.id.to_string(),
        application_type: application_type.clone(),
        run_object_kind: application.sections.logs.run_object_kind.clone(),
        run_kind: detail.flow_run.run_mode.as_str().to_string(),
        status: detail.flow_run.status.as_str().to_string(),
        title: detail.flow_run.title.clone(),
        execution_stage: invocation_context.execution_stage.as_str().to_string(),
        invocation_source: invocation_context.invocation_source.as_str().to_string(),
        compatibility_mode: detail.flow_run.compatibility_mode.clone(),
        subject: application_logs::ApplicationRunSubjectResponse {
            kind: application_type,
            id: Some(detail.flow_run.flow_id.to_string()),
            draft_id: Some(detail.flow_run.draft_id.to_string()),
            target_node_id: detail.flow_run.target_node_id.clone(),
        },
        principal: application_logs::principal_response(invocation_context.principal),
        correlation: application_logs::ApplicationRunCorrelationResponse {
            api_key_id: detail.flow_run.api_key_id.map(|value| value.to_string()),
            publication_version_id: detail
                .flow_run
                .publication_version_id
                .map(|value| value.to_string()),
            external_user: detail.flow_run.external_user.clone(),
            external_conversation_id: detail.flow_run.external_conversation_id.clone(),
            external_trace_id: detail.flow_run.external_trace_id.clone(),
            compatibility_mode: detail.flow_run.compatibility_mode.clone(),
            idempotency_key: detail.flow_run.idempotency_key.clone(),
        },
        started_at: application_logs::format_time(detail.flow_run.started_at),
        finished_at: application_logs::format_optional_time(detail.flow_run.finished_at),
        created_at: application_logs::format_time(detail.flow_run.created_at),
        updated_at: application_logs::format_time(detail.flow_run.updated_at),
    };
    let typed_detail = application_logs::ApplicationRunTypedDetailResponse {
        kind: application.application_type.as_str().to_string(),
        flow_run: flow_run.clone(),
        answer_snapshot: answer_snapshot.clone(),
        node_runs: node_runs.clone(),
        checkpoints: checkpoints.clone(),
        callback_tasks: callback_tasks.clone(),
        events: events.clone(),
        stitched_trace: stitched_trace.clone(),
    };

    ApplicationRunDetailResponse {
        run,
        statistics,
        detail: typed_detail,
        flow_run,
        answer_snapshot,
        node_runs,
        checkpoints,
        callback_tasks,
        events,
        stitched_trace,
    }
}

fn to_node_last_run_response(last_run: domain::NodeLastRun) -> NodeLastRunResponse {
    NodeLastRunResponse {
        flow_run: to_flow_run_response(last_run.flow_run),
        node_run: to_node_run_response(last_run.node_run),
        checkpoints: last_run
            .checkpoints
            .into_iter()
            .map(to_checkpoint_response)
            .collect(),
        events: last_run
            .events
            .into_iter()
            .map(to_run_event_response)
            .collect(),
    }
}

fn to_runtime_debug_stream_part_response(
    part: observability::DebugStreamPart,
) -> RuntimeDebugStreamPartResponse {
    RuntimeDebugStreamPartResponse {
        id: part.id.to_string(),
        flow_run_id: part.flow_run_id.to_string(),
        item_id: part.item_id.map(|value| value.to_string()),
        span_id: part.span_id.map(|value| value.to_string()),
        part_type: part.part_type,
        status: part.status,
        trust_level: part.trust_level.as_str().to_string(),
        payload: part.payload,
    }
}
