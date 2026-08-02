use super::*;

pub(super) fn parse_trace_projection_node_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ControlPlaneError::InvalidInput("trace_node_id").into())
}

pub(super) const APPLICATION_RUN_TRACE_CHILDREN_DEFAULT_PAGE_SIZE: i64 = 20;
pub(super) const APPLICATION_RUN_TRACE_CHILDREN_MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ApplicationRunTraceChildrenCursorPayload {
    parent_trace_node_id: Uuid,
    order_key: String,
    trace_node_id: Uuid,
}

pub(super) fn application_run_trace_children_page_size(page_size: Option<i64>) -> i64 {
    page_size
        .unwrap_or(APPLICATION_RUN_TRACE_CHILDREN_DEFAULT_PAGE_SIZE)
        .clamp(1, APPLICATION_RUN_TRACE_CHILDREN_MAX_PAGE_SIZE)
}

pub(super) fn parse_application_run_trace_children_cursor(
    cursor: Option<&str>,
    parent_trace_node_id: Uuid,
) -> Result<Option<ApplicationRunTraceChildrenCursor>, ApiError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };

    let bytes = URL_SAFE_NO_PAD
        .decode(cursor.as_bytes())
        .map_err(|_| ControlPlaneError::InvalidInput("cursor"))?;
    let payload: ApplicationRunTraceChildrenCursorPayload =
        serde_json::from_slice(&bytes).map_err(|_| ControlPlaneError::InvalidInput("cursor"))?;

    if payload.order_key.is_empty() {
        return Err(ControlPlaneError::InvalidInput("cursor").into());
    }
    if payload.parent_trace_node_id != parent_trace_node_id {
        return Err(ControlPlaneError::InvalidInput("cursor").into());
    }

    Ok(Some(ApplicationRunTraceChildrenCursor {
        order_key: payload.order_key,
        trace_node_id: payload.trace_node_id,
    }))
}

pub(super) fn encode_application_run_trace_children_cursor(
    cursor: &ApplicationRunTraceChildrenCursor,
    parent_trace_node_id: Uuid,
) -> Result<String, ApiError> {
    let payload = ApplicationRunTraceChildrenCursorPayload {
        parent_trace_node_id,
        order_key: cursor.order_key.clone(),
        trace_node_id: cursor.trace_node_id,
    };
    let bytes = serde_json::to_vec(&payload).map_err(ApiError::from)?;

    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(super) fn to_trace_node_summary_from_projection(
    node: domain::ApplicationRunTraceNodeRecord,
) -> ApplicationRunTraceNodeSummaryResponse {
    let status = trace_node_summary_status(&node.status);

    ApplicationRunTraceNodeSummaryResponse {
        trace_node_id: node.trace_node_id.to_string(),
        stable_locator: node.stable_locator,
        parent_trace_node_id: node.parent_trace_node_id.map(|value| value.to_string()),
        node_kind: node.node_kind,
        flow_run_id: node.flow_run_id.to_string(),
        node_run_id: node
            .owner_kind
            .as_deref()
            .is_some_and(|kind| kind == "node_run")
            .then(|| node.owner_id.clone())
            .flatten(),
        callback_task_id: node
            .owner_kind
            .as_deref()
            .is_some_and(|kind| kind == "callback_task")
            .then(|| node.owner_id.clone())
            .flatten(),
        node_id: node.node_id,
        node_type: node.node_type,
        node_mode: node.node_mode,
        node_alias: node.node_alias,
        status,
        started_at: format_time(node.started_at),
        finished_at: format_optional_time(node.finished_at),
        duration_ms: node.duration_ms,
        metrics_payload: node.metrics_payload,
        has_children: node.has_children,
        child_count: node.child_count,
        has_content: node.has_content,
        source_flow_run_id: node.source_flow_run_id.map(|value| value.to_string()),
        source_trace_node_id: node.source_trace_node_id.map(|value| value.to_string()),
        parent_callback_task_id: node.parent_callback_task_id.map(|value| value.to_string()),
        parent_tool_call_id: node.parent_tool_call_id,
        trace_relation_kind: node.trace_relation_kind,
    }
}

pub(super) fn trace_node_summary_status(status: &str) -> String {
    match status {
        "pending" => domain::NodeRunStatus::WaitingCallback.as_str(),
        "completed" => domain::NodeRunStatus::Succeeded.as_str(),
        "cancelled" => domain::NodeRunStatus::Failed.as_str(),
        value => value,
    }
    .to_string()
}

pub(super) fn to_trace_projection_statistics_response(
    statistics: ApplicationRunTraceProjectionStatistics,
) -> application_logs::ApplicationRunStatisticsResponse {
    application_logs::ApplicationRunStatisticsResponse {
        count_tokens_input_tokens: None,
        total_tokens: statistics.total_tokens,
        input_tokens: statistics.input_tokens,
        output_tokens: statistics.output_tokens,
        input_cache_hit_tokens: statistics.input_cache_hit_tokens,
        input_cache_hit_rate: application_logs::input_cache_hit_rate_for_response(
            statistics.total_tokens,
            statistics.input_cache_hit_tokens,
        ),
        unique_node_count: statistics.unique_node_count,
        tool_callback_count: statistics.tool_callback_count,
    }
}

pub(super) fn trace_projection_node_content_response(
    node: domain::ApplicationRunTraceNodeRecord,
    content: domain::ApplicationRunTraceNodeContentRecord,
    projection_status: ApplicationRunTraceProjectionStatusResponse,
) -> Result<ApplicationRunTraceNodeContentResponse, ApiError> {
    let content_kind = content.content_kind;
    let payload = content.payload;
    let detail_refs = payload
        .get("detail_refs")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    let payload = trace_node_content_raw_payload_response(payload);

    Ok(ApplicationRunTraceNodeContentResponse {
        trace_node_id: node.trace_node_id.to_string(),
        node_kind: node.node_kind,
        projection_status,
        content_kind,
        source_refs: content.source_refs,
        detail_refs,
        payload,
    })
}

pub(super) fn trace_node_content_raw_payload_response(mut payload: serde_json::Value) -> serde_json::Value {
    let Some(payload_object) = payload.as_object_mut() else {
        return payload;
    };

    payload_object.remove("node_run");
    payload_object.remove("checkpoints");
    payload_object.remove("events");
    payload_object.remove("source_refs");
    payload_object.remove("detail_refs");

    if payload_object.is_empty() {
        serde_json::json!({})
    } else {
        payload
    }
}

pub(super) fn trace_node_content_detail_refs(payload: &serde_json::Value) -> Vec<serde_json::Value> {
    payload
        .get("detail_refs")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn trace_node_content_detail_ref(
    payload: &serde_json::Value,
    detail_ref_id: &str,
) -> Option<serde_json::Value> {
    trace_node_content_detail_refs(payload)
        .into_iter()
        .find(|detail_ref| {
            detail_ref
                .get("detail_ref_id")
                .and_then(serde_json::Value::as_str)
                == Some(detail_ref_id)
        })
}

pub(super) fn trace_node_content_node_run_ids(payload: &serde_json::Value) -> Result<Vec<Uuid>, ApiError> {
    let values = payload
        .get("payload_index")
        .and_then(|payload_index| payload_index.get("node_run_ids"))
        .and_then(serde_json::Value::as_array)
        .ok_or(ControlPlaneError::Conflict("trace_node_detail_ref"))?;
    let mut node_run_ids = Vec::with_capacity(values.len());

    for value in values {
        let Some(id) = value.as_str() else {
            return Err(ControlPlaneError::Conflict("trace_node_detail_ref").into());
        };
        node_run_ids.push(
            Uuid::parse_str(id)
                .map_err(|_| ControlPlaneError::Conflict("trace_node_detail_ref"))?,
        );
    }

    Ok(node_run_ids)
}

pub(super) fn trace_node_content_source_flow_run_id(
    payload: &serde_json::Value,
) -> Result<Option<Uuid>, ApiError> {
    let Some(value) = payload
        .get("payload_index")
        .and_then(|payload_index| payload_index.get("source_flow_run_id"))
    else {
        return Ok(None);
    };
    let Some(id) = value.as_str() else {
        return Err(ControlPlaneError::Conflict("trace_node_detail_ref").into());
    };

    Ok(Some(
        Uuid::parse_str(id).map_err(|_| ControlPlaneError::Conflict("trace_node_detail_ref"))?,
    ))
}

pub(super) fn strip_projected_tool_debug_payloads(mut node_run: domain::NodeRunRecord) -> domain::NodeRunRecord {
    if let Some(debug_payload) = node_run.debug_payload.as_object_mut() {
        for key in [
            "llm_rounds",
            "tool_callbacks",
            "visible_internal_llm_tool_trace",
            "visible_internal_llm_tool_events",
        ] {
            debug_payload.remove(key);
        }
    }

    node_run
}

pub(super) fn trace_node_run_detail_payload(node_run: domain::NodeRunRecord) -> serde_json::Value {
    serde_json::json!({
        "node_run": to_node_run_response(strip_projected_tool_debug_payloads(node_run))
    })
}

pub(super) async fn find_trace_projection_tool_callback_node(
    state: &Arc<ApiState>,
    flow_run_id: Uuid,
    owner: &domain::ApplicationRunTraceNodeRecord,
    tool_call_id: &str,
) -> Result<domain::ApplicationRunTraceNodeRecord, ApiError> {
    if owner.node_kind == "tool_callback" && owner.owner_id.as_deref() == Some(tool_call_id) {
        return Ok(owner.clone());
    }

    for stable_locator in [
        format!("{}/tool:{tool_call_id}", owner.stable_locator),
        format!("{}/tools/tool:{tool_call_id}", owner.stable_locator),
    ] {
        if let Some(node) =
            <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_by_locator(
                &state.store,
                flow_run_id,
                &stable_locator,
            )
            .await?
        {
            return Ok(node);
        }
    }

    Err(ControlPlaneError::NotFound("tool_callback").into())
}

pub(super) fn parse_trace_node_artifact_preview_field_path(value: &str) -> Option<Vec<String>> {
    let field_path = value
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if field_path.is_empty() {
        None
    } else {
        Some(field_path)
    }
}

pub(super) fn trace_node_artifact_preview_request(
    raw_query: Option<&str>,
) -> Option<RuntimeDebugArtifactPreviewRequest> {
    let raw_query = raw_query?;
    let mut auto_requested = false;
    let mut field_paths: Vec<Vec<String>> = Vec::new();

    for (key, value) in form_urlencoded::parse(raw_query.as_bytes()) {
        match key.as_ref() {
            "artifact_preview" if value.as_ref() == "auto" => {
                auto_requested = true;
            }
            "artifact_preview_field" => {
                if let Some(field_path) = parse_trace_node_artifact_preview_field_path(&value) {
                    if !field_paths
                        .iter()
                        .any(|existing_path| existing_path == &field_path)
                    {
                        field_paths.push(field_path);
                    }
                }
            }
            _ => {}
        }
    }

    if !field_paths.is_empty() {
        Some(RuntimeDebugArtifactPreviewRequest::Fields(field_paths))
    } else if auto_requested {
        Some(RuntimeDebugArtifactPreviewRequest::Auto)
    } else {
        None
    }
}
