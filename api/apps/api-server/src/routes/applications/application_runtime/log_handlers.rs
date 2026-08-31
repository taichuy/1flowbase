#[path = "log_handlers/trace_projection_payloads.rs"]
mod trace_projection_payloads;
use trace_projection_payloads::*;

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs",
    params(
        ("id" = String, Path, description = "Application id"),
        ("page" = Option<i64>, Query, description = "1-based page number"),
        ("page_size" = Option<i64>, Query, description = "Page size"),
        ("time_range_days" = Option<i64>, Query, description = "Created-at day window, defaults to 7 days"),
        ("sort_by" = Option<String>, Query, description = "Sort field: created_at, started_at, finished_at or updated_at"),
        ("sort_order" = Option<String>, Query, description = "Sort direction: asc or desc"),
        ("cache_mode" = Option<String>, Query, description = "Read mode: refresh bypasses application log cache reads")
    ),
    responses(
        (status = 200, body = FlowRunSummaryPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_runs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<ApplicationRunsQuery>,
) -> Result<Json<ApiSuccess<FlowRunSummaryPageResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.logs.list.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::ListRuns { application_id: id, query }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::Runs(response) = output else { unreachable!("application runtime logs binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/conversations/{conversation_id}/messages",
    params(
        ("id" = String, Path, description = "Application id"),
        ("conversation_id" = String, Path, description = "External conversation id"),
        ("around_run_id" = Option<String>, Query, description = "Flow run id to center the page around"),
        ("before" = Option<String>, Query, description = "Load runs before this cursor run id"),
        ("after" = Option<String>, Query, description = "Load runs after this cursor run id"),
        ("limit" = Option<i64>, Query, description = "Page size, defaults to 5")
    ),
    responses(
        (status = 200, body = ApplicationConversationMessagesPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_conversation_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, conversation_id)): Path<(Uuid, String)>,
    Query(query): Query<ApplicationConversationMessagesQuery>,
) -> Result<Json<ApiSuccess<ApplicationConversationMessagesPageResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.conversations.messages.list.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::ListConversationMessages { application_id: id, conversation_id, query }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ConversationMessages(response) = output else { unreachable!("application conversation messages binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/conversation/messages",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("before" = Option<String>, Query, description = "Load messages before this cursor"),
        ("after" = Option<String>, Query, description = "Load messages after this cursor"),
        ("limit" = Option<i64>, Query, description = "Page size, defaults to 5")
    ),
    responses(
        (status = 200, body = ApplicationConversationMessagesPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_run_conversation_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ApplicationConversationMessagesQuery>,
) -> Result<Json<ApiSuccess<ApplicationConversationMessagesPageResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.run-conversation.messages.list.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::ListRunConversationMessages { application_id: id, run_id, query }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ConversationMessages(response) = output else { unreachable!("application run conversation messages binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

async fn ensure_application_run_trace_projection_status(
    state: &Arc<ApiState>,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<domain::ApplicationRunTraceProjectionStatusRecord, ApiError> {
    let status =
        <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
            &state.store,
            flow_run_id,
            APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        )
        .await?;

    if let Some(status) = status.as_ref() {
        match status.status {
            domain::ApplicationRunTraceProjectionStatus::Pending
            | domain::ApplicationRunTraceProjectionStatus::Running
            | domain::ApplicationRunTraceProjectionStatus::Failed => return Ok(status.clone()),
            domain::ApplicationRunTraceProjectionStatus::Succeeded
            | domain::ApplicationRunTraceProjectionStatus::Stale
            | domain::ApplicationRunTraceProjectionStatus::Partial => {}
        }
    }

    let source_watermark =
        <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
            &state.store,
            application_id,
            flow_run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if !projection_status_needs_lazy_rebuild(status.as_ref(), &source_watermark) {
        return status.ok_or_else(|| ControlPlaneError::Conflict("trace_projection_status").into());
    }

    let source =
        <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
            &state.store,
            application_id,
            flow_run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let runtime_events = <_ as OrchestrationRuntimeRepository>::list_runtime_events(
        &state.store,
        flow_run_id,
        0,
    )
    .await?;
    let source =
        enrich_application_run_detail_visible_internal_llm_route_traces(source, &runtime_events);
    let projection = build_application_run_trace_projection(&source)?;
    <_ as OrchestrationRuntimeRepository>::replace_application_run_trace_projection(
        &state.store,
        &projection,
    )
    .await?;

    <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
        &state.store,
        flow_run_id,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION,
    )
    .await?
    .ok_or_else(|| ControlPlaneError::Conflict("trace_projection_status").into())
}

fn to_trace_projection_status_response(
    status: &domain::ApplicationRunTraceProjectionStatusRecord,
) -> ApplicationRunTraceProjectionStatusResponse {
    ApplicationRunTraceProjectionStatusResponse {
        projection_status: status.status.as_str().to_string(),
        projection_version: status.projection_version,
        source_watermark: status.source_watermark.clone(),
        attempt_count: status.attempt_count,
        last_attempt_at: format_optional_time(status.last_attempt_at),
        last_success_at: format_optional_time(status.last_success_at),
        last_error_code: status.last_error_code.clone(),
        last_error_stage: status.last_error_stage.clone(),
        last_error_source_kind: status.last_error_source_kind.clone(),
        last_error_source_locator: status.last_error_source_locator.clone(),
        last_error_ref: status.last_error_ref.clone(),
        retriable: status.retriable,
    }
}

fn application_run_log_response_for_trace_tree(
    application: &domain::ApplicationRecord,
    flow_run: &domain::FlowRunRecord,
) -> application_logs::ApplicationRunLogResponse {
    let application_type = application.application_type.as_str().to_string();

    let invocation_context = flow_run.run_mode.invocation_context(
        Some(flow_run.created_by),
        flow_run.authorized_account.clone(),
        flow_run.api_key_id,
    );

    application_logs::ApplicationRunLogResponse {
        id: flow_run.id.to_string(),
        application_id: application.id.to_string(),
        application_type: application_type.clone(),
        run_object_kind: application.sections.logs.run_object_kind.clone(),
        run_kind: flow_run.run_mode.as_str().to_string(),
        status: flow_run.status.as_str().to_string(),
        title: flow_run.title.clone(),
        execution_stage: invocation_context.execution_stage.as_str().to_string(),
        invocation_source: invocation_context.invocation_source.as_str().to_string(),
        compatibility_mode: flow_run.compatibility_mode.clone(),
        subject: application_logs::ApplicationRunSubjectResponse {
            kind: application_type,
            id: Some(flow_run.flow_id.to_string()),
            draft_id: Some(flow_run.draft_id.to_string()),
            target_node_id: flow_run.target_node_id.clone(),
        },
        principal: application_logs::principal_response(invocation_context.principal),
        correlation: application_logs::ApplicationRunCorrelationResponse {
            api_key_id: flow_run.api_key_id.map(|value| value.to_string()),
            publication_version_id: flow_run
                .publication_version_id
                .map(|value| value.to_string()),
            external_user: flow_run.external_user.clone(),
            external_conversation_id: flow_run.external_conversation_id.clone(),
            external_trace_id: flow_run.external_trace_id.clone(),
            compatibility_mode: flow_run.compatibility_mode.clone(),
            idempotency_key: flow_run.idempotency_key.clone(),
        },
        started_at: application_logs::format_time(flow_run.started_at),
        finished_at: application_logs::format_optional_time(flow_run.finished_at),
        created_at: application_logs::format_time(flow_run.created_at),
        updated_at: application_logs::format_time(flow_run.updated_at),
    }
}

fn projection_is_succeeded(status: &domain::ApplicationRunTraceProjectionStatusRecord) -> bool {
    status.status == domain::ApplicationRunTraceProjectionStatus::Succeeded
}

fn answer_snapshot_for_log_overview(
    overview: &ApplicationRunOverviewReadModel,
) -> Option<AnswerSnapshotResponse> {
    let (answer_snapshot_node_run, _) = split_answer_snapshot_node_run_records(&overview.node_runs);

    if !flow_run_can_expose_answer_snapshot(&overview.flow_run.status) {
        return None;
    }

    let waiting_node = (
        overview.waiting_node_id.clone(),
        overview.waiting_node_run_id.map(|value| value.to_string()),
    );

    answer_snapshot_node_run
        .as_ref()
        .and_then(|node_run| {
            to_answer_snapshot_response_with_waiting_node(node_run, waiting_node.clone())
        })
        .or_else(|| {
            to_flow_run_answer_snapshot_response_with_waiting_node(&overview.flow_run, waiting_node)
        })
}

fn to_application_run_overview_response(
    application: &domain::ApplicationRecord,
    overview: ApplicationRunOverviewReadModel,
) -> ApplicationRunOverviewResponse {
    let (_, current_visible_node_runs) =
        split_answer_snapshot_node_run_records(&overview.node_runs);
    let statistics = application_run_statistics_for_records(
        &current_visible_node_runs,
        overview.tool_callback_count,
    );

    ApplicationRunOverviewResponse {
        run: application_run_log_response_for_trace_tree(application, &overview.flow_run),
        statistics,
        flow_run: to_flow_run_response(overview.flow_run.clone()),
        answer_snapshot: answer_snapshot_for_log_overview(&overview),
    }
}

async fn load_application_run_overview(
    state: Arc<ApiState>,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<ApplicationRunOverviewReadModel, ApiError> {
    Ok(
        <_ as OrchestrationRuntimeRepository>::get_application_run_overview(
            &state.store,
            application_id,
            flow_run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?,
    )
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/overview",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunOverviewResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_overview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunOverviewResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.run.overview.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::GetRunOverview { application_id: id, run_id }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::RunOverview(response) = output else { unreachable!("application run overview binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceTreeResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_trace_tree(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunTraceTreeResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.trace-tree.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::GetTraceTree { application_id: id, run_id }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::TraceTree(response) = output else { unreachable!("application trace tree binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("parent_trace_node_id" = String, Query, description = "Trace node id to expand"),
        ("page_size" = Option<i64>, Query, description = "Page size, defaults to 20 and maxes at 100"),
        ("cursor" = Option<String>, Query, description = "Opaque cursor for the next children page")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceNodeChildrenResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_trace_node_children(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ApplicationRunTraceNodeChildrenQuery>,
) -> Result<Json<ApiSuccess<ApplicationRunTraceNodeChildrenResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.trace-tree.children.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::GetTraceChildren { application_id: id, run_id, query }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::TraceChildren(response) = output else { unreachable!("application trace children binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/content",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("trace_node_id" = String, Path, description = "Trace node id to load"),
        ("artifact_preview" = Option<String>, Query, description = "Set to auto to materialize runtime debug artifact previews"),
        ("artifact_preview_field" = Option<Vec<String>>, Query, description = "Repeated dot-separated response payload field paths to preview")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceNodeContentResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_trace_node_content(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id, trace_node_id)): Path<(Uuid, Uuid, String)>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<ApiSuccess<ApplicationRunTraceNodeContentResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.trace-node.content.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_trace_payloads::ApplicationRuntimeTracePayloadsInput::GetNodeContent { application_id: id, run_id, trace_node_id, raw_query }).await?;
    let interface_trace_payloads::ApplicationRuntimeTracePayloadsOutput::NodeContent(response) = output else { unreachable!("application trace node content binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/details/{detail_ref_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("trace_node_id" = String, Path, description = "Trace node id that owns the detail ref"),
        ("detail_ref_id" = String, Path, description = "Detail ref id from node content"),
        ("artifact_preview" = Option<String>, Query, description = "Set to auto to materialize runtime debug artifact previews"),
        ("artifact_preview_field" = Option<Vec<String>>, Query, description = "Repeated dot-separated response payload field paths to preview")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceNodeDetailResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_trace_node_detail(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id, trace_node_id, detail_ref_id)): Path<(Uuid, Uuid, String, String)>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<ApiSuccess<ApplicationRunTraceNodeDetailResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.trace-node.detail.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_trace_payloads::ApplicationRuntimeTracePayloadsInput::GetNodeDetail { application_id: id, run_id, trace_node_id, detail_ref_id, raw_query }).await?;
    let interface_trace_payloads::ApplicationRuntimeTracePayloadsOutput::NodeDetail(response) = output else { unreachable!("application trace node detail binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/tool-callbacks/{tool_call_id}/content",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("trace_node_id" = String, Path, description = "Trace node id that owns the tool callback"),
        ("tool_call_id" = String, Path, description = "Tool call id to load")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceToolCallbackContentResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_trace_tool_callback_content(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id, trace_node_id, tool_call_id)): Path<(Uuid, Uuid, String, String)>,
) -> Result<Json<ApiSuccess<ApplicationRunTraceToolCallbackContentResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.trace-tool-callback.content.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_trace_payloads::ApplicationRuntimeTracePayloadsInput::GetToolCallbackContent { application_id: id, run_id, trace_node_id, tool_call_id }).await?;
    let interface_trace_payloads::ApplicationRuntimeTracePayloadsOutput::ToolCallbackContent(response) = output else { unreachable!("application trace tool callback content binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/resume-timeline",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunResumeTimelineResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_resume_timeline(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunResumeTimelineResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.resume-timeline.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::GetResumeTimeline { application_id: id, run_id }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ResumeTimeline(response) = output else { unreachable!("application resume timeline binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/resume-timeline-summary",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunResumeTimelineSummaryResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_resume_timeline_summary(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunResumeTimelineSummaryResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.resume-timeline-summary.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::GetResumeTimelineSummary { application_id: id, run_id }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ResumeTimelineSummary(response) = output else { unreachable!("application resume timeline summary binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("node_id" = String, Path, description = "Flow node id")
    ),
    responses(
        (status = 200, body = NodeLastRunResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_node_last_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id, node_id)): Path<(Uuid, Uuid, String)>,
) -> Result<Json<ApiSuccess<Option<NodeLastRunResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.run-node-last-run.get.v1", crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }, interface_runtime_reads::ApplicationRuntimeReadsInput::GetRunNodeLastRun { application_id: id, run_id, node_id }).await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::RunNodeLastRun(response) = output else { unreachable!("application run node last-run binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/debug-stream",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("from_sequence" = Option<i64>, Query, description = "Return runtime debug events after this stream sequence"),
        ("limit" = Option<i64>, Query, description = "Page size, defaults to 500 and maxes at 1000")
    ),
    responses(
        (status = 200, body = RuntimeDebugStreamResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_debug_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<RuntimeDebugStreamQuery>,
) -> Result<Json<ApiSuccess<RuntimeDebugStreamResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, &context.actor, id).await?;

    <_ as OrchestrationRuntimeRepository>::get_flow_run(&state.store, id, run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;

    let page_size = runtime_debug_stream_page_size(query.limit);
    let from_sequence = query.from_sequence.unwrap_or(0).max(0);
    let mut records =
        <_ as OrchestrationRuntimeRepository>::list_runtime_event_backfill_page(
            &state.store,
            run_id,
            from_sequence,
            page_size + 1,
        )
        .await?;
    let has_more = records.len() > page_size;
    if has_more {
        records.truncate(page_size);
    }
    let next_sequence = records
        .last()
        .map(debug_run_stream::durable_event_stream_sequence);
    let parts = records
        .iter()
        .filter_map(|event| {
            control_plane::runtime_observability::debug_read_model::fold_event_to_debug_part(
                run_id, event,
            )
        })
        .map(to_runtime_debug_stream_part_response)
        .collect();

    Ok(Json(ApiSuccess::new(RuntimeDebugStreamResponse {
        parts,
        page_size: i64::try_from(page_size).unwrap_or(i64::MAX),
        next_sequence,
        has_more,
    })))
}

fn runtime_debug_stream_page_size(limit: Option<i64>) -> usize {
    limit
        .unwrap_or(RUNTIME_DEBUG_STREAM_DEFAULT_PAGE_SIZE as i64)
        .clamp(1, RUNTIME_DEBUG_STREAM_MAX_PAGE_SIZE as i64) as usize
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/nodes/{node_id}/last-run",
    params(
        ("id" = String, Path, description = "Application id"),
        ("node_id" = String, Path, description = "Node id")
    ),
    responses(
        (status = 200, body = NodeLastRunResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_node_last_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, node_id)): Path<(Uuid, String)>,
) -> Result<Json<ApiSuccess<Option<NodeLastRunResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, &context.actor, id).await?;

    let last_run = <_ as OrchestrationRuntimeRepository>::get_latest_node_run(
        &state.store,
        id,
        &node_id,
    )
    .await?;
    let last_run = match last_run {
        Some(last_run) => {
            let runtime_events =
                <_ as OrchestrationRuntimeRepository>::list_runtime_events(
                    &state.store,
                    last_run.flow_run.id,
                    0,
                )
                .await?;
            Some(to_node_last_run_response(
                enrich_node_last_run_visible_internal_llm_route_traces(last_run, &runtime_events),
            ))
        }
        None => None,
    };

    Ok(Json(ApiSuccess::new(last_run)))
}
