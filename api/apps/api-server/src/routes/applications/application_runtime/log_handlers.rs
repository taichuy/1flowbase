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
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let created_after = application_runs_created_after(&query);
    let sort_by = normalize_application_run_sort_by(query.sort_by.as_deref()).to_string();
    let sort_order = normalize_application_run_sort_order(query.sort_order.as_deref()).to_string();
    let refresh_cache = should_refresh_application_run_logs(query.cache_mode.as_deref());
    let cache = state.infrastructure.cache_store();
    let cache_key = application_log_cache::summary_page_cache_key(
        context.actor.current_workspace_id,
        id,
        &query,
        page,
        page_size,
        &sort_by,
        &sort_order,
    );

    if !refresh_cache {
        if let Some(cached) =
            application_log_cache::read::<FlowRunSummaryPageResponse>(cache.as_ref(), &cache_key)
                .await
        {
            return Ok(Json(ApiSuccess::new(cached)));
        }
    }

    let runs_page =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_logs_page(
            &state.store,
            id,
            control_plane::ports::ListApplicationRunsPageInput {
                page,
                page_size,
                created_after,
                sort_by: Some(sort_by),
                sort_order: Some(sort_order),
            },
        )
        .await?;

    let mut items = Vec::with_capacity(runs_page.items.len());

    for log_summary in runs_page.items {
        let statistics = application_logs::ApplicationRunStatisticsResponse {
            count_tokens_input_tokens: log_summary.count_tokens_input_tokens,
            total_tokens: log_summary.total_tokens,
            input_tokens: log_summary.input_tokens,
            output_tokens: log_summary.output_tokens,
            input_cache_hit_tokens: log_summary.input_cache_hit_tokens,
            input_cache_hit_rate: application_logs::input_cache_hit_rate_for_response(
                log_summary.total_tokens,
                log_summary.input_cache_hit_tokens,
            ),
            unique_node_count: log_summary.unique_node_count,
            tool_callback_count: log_summary.tool_callback_count,
        };
        items.push(to_flow_run_summary_response(
            &application,
            log_summary.run,
            statistics,
        ));
    }

    let response = FlowRunSummaryPageResponse {
        items,
        total: runs_page.total,
        page: runs_page.page,
        page_size: runs_page.page_size,
    };

    if application_log_cache::summary_page_cacheable(&response) {
        application_log_cache::write(
            cache.as_ref(),
            &cache_key,
            &response,
            application_log_cache::summary_page_cache_ttl(page),
        )
        .await;
    }

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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let page =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &state.store,
            id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation_id,
                around_run_id: query.around_run_id,
                before_run_id: parse_optional_uuid_cursor(query.before.as_deref()),
                after_run_id: parse_optional_uuid_cursor(query.after.as_deref()),
                limit: query.limit.unwrap_or(5),
            },
        )
        .await?;
    let mut items = Vec::with_capacity(page.items.len());
    for run in page.items {
        items.push(to_application_conversation_message_summary_response(
            run,
            query.around_run_id,
        ));
    }

    Ok(Json(ApiSuccess::new(
        ApplicationConversationMessagesPageResponse {
            items,
            page: ApplicationConversationMessagesPageInfoResponse {
                has_before: page.has_before,
                has_after: page.has_after,
                before_cursor: page.before_cursor.map(|value| value.to_string()),
                after_cursor: page.after_cursor.map(|value| value.to_string()),
            },
        },
    )))
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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let projection_page =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &state.store,
            id,
            run_id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: parse_run_conversation_message_sequence_cursor(
                    run_id,
                    query.before.as_deref(),
                ),
                after_sequence: parse_run_conversation_message_sequence_cursor(
                    run_id,
                    query.after.as_deref(),
                ),
                limit: query.limit.unwrap_or(5),
            },
        )
        .await?;
    if projection_page.total_count > 0 {
        return Ok(Json(ApiSuccess::new(
            conversation_messages_from_projection_page(run_id, projection_page),
        )));
    }

    let current_item =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_conversation_current_item(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;

    Ok(Json(ApiSuccess::new(
        conversation_messages_from_current_item(run_id, current_item),
    )))
}

async fn ensure_application_run_trace_projection_status(
    state: &Arc<ApiState>,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<domain::ApplicationRunTraceProjectionStatusRecord, ApiError> {
    let status =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
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
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
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
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
            &state.store,
            application_id,
            flow_run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let runtime_events = <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
        &state.store,
        flow_run_id,
        0,
    )
    .await?;
    let source = enrich_application_run_detail_visible_internal_llm_route_traces(
        source,
        &runtime_events,
    );
    let projection = build_application_run_trace_projection(&source)?;
    <MainDurableStore as OrchestrationRuntimeRepository>::replace_application_run_trace_projection(
        &state.store,
        &projection,
    )
    .await?;

    <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
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
    detail: &domain::ApplicationRunDetail,
) -> Option<AnswerSnapshotResponse> {
    let (answer_snapshot_node_run, _) = split_answer_snapshot_node_runs(detail);

    if !flow_run_can_expose_answer_snapshot(&detail.flow_run.status) {
        return None;
    }

    answer_snapshot_node_run
        .as_ref()
        .and_then(|node_run| to_answer_snapshot_response(node_run, detail))
        .or_else(|| to_flow_run_answer_snapshot_response(detail))
}

fn to_application_run_overview_response(
    application: &domain::ApplicationRecord,
    detail: domain::ApplicationRunDetail,
) -> ApplicationRunOverviewResponse {
    let (_, current_visible_node_runs) = split_answer_snapshot_node_runs(&detail);
    let statistics = application_run_statistics(&domain::ApplicationRunDetail {
        node_runs: current_visible_node_runs,
        ..detail.clone()
    });

    ApplicationRunOverviewResponse {
        run: application_run_log_response_for_trace_tree(application, &detail.flow_run),
        statistics,
        flow_run: to_flow_run_response(detail.flow_run.clone()),
        answer_snapshot: answer_snapshot_for_log_overview(&detail),
    }
}

async fn load_application_run_detail_for_log_overview(
    state: Arc<ApiState>,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<domain::ApplicationRunDetail, ApiError> {
    Ok(
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
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
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;
    let detail = load_application_run_detail_for_log_overview(state, id, run_id).await?;
    let response = to_application_run_overview_response(&application, detail);

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
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;
    let status = ensure_application_run_trace_projection_status(&state, id, run_id).await?;
    let flow_run = <MainDurableStore as OrchestrationRuntimeRepository>::get_flow_run(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let nodes = if projection_is_succeeded(&status) {
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_trace_roots(
            &state.store,
            run_id,
        )
        .await?
    } else {
        Vec::new()
    };
    let statistics = if projection_is_succeeded(&status) {
        to_trace_projection_statistics_response(
            <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_statistics(
                &state.store,
                run_id,
            )
            .await?,
        )
    } else {
        to_trace_projection_statistics_response(ApplicationRunTraceProjectionStatistics {
            total_tokens: None,
            input_tokens: None,
            output_tokens: None,
            input_cache_hit_tokens: None,
            unique_node_count: 0,
            tool_callback_count: 0,
        })
    };
    let response = ApplicationRunTraceTreeResponse {
        run: application_run_log_response_for_trace_tree(&application, &flow_run),
        statistics,
        flow_run: to_flow_run_response(flow_run),
        answer_snapshot: None,
        projection_status: to_trace_projection_status_response(&status),
        nodes: nodes
            .into_iter()
            .map(to_trace_node_summary_from_projection)
            .collect(),
    };

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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let status = ensure_application_run_trace_projection_status(&state, id, run_id).await?;
    let projection_status = to_trace_projection_status_response(&status);
    let page_size = application_run_trace_children_page_size(query.page_size);
    let parent_trace_node_id = parse_trace_projection_node_id(&query.parent_trace_node_id)?;
    let cursor =
        parse_application_run_trace_children_cursor(query.cursor.as_deref(), parent_trace_node_id)?;
    if !projection_is_succeeded(&status) {
        return Ok(Json(ApiSuccess::new(
            ApplicationRunTraceNodeChildrenResponse {
                projection_status,
                items: Vec::new(),
                page_info: ApplicationRunTraceNodeChildrenPageInfoResponse {
                    has_more: false,
                    next_cursor: None,
                    page_size,
                },
            },
        )));
    }
    <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node(
        &state.store,
        run_id,
        parent_trace_node_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("trace_node"))?;
    let page = <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
            &state.store,
            ListApplicationRunTraceChildrenPageInput {
                flow_run_id: run_id,
                parent_trace_node_id,
                page_size,
                cursor,
            },
        )
        .await?;
    let next_cursor = page
        .next_cursor
        .as_ref()
        .map(|cursor| encode_application_run_trace_children_cursor(cursor, parent_trace_node_id))
        .transpose()?;
    let items = page
        .items
        .into_iter()
        .map(to_trace_node_summary_from_projection)
        .collect();
    let response = ApplicationRunTraceNodeChildrenResponse {
        projection_status,
        items,
        page_info: ApplicationRunTraceNodeChildrenPageInfoResponse {
            has_more: page.has_more,
            next_cursor,
            page_size: page.page_size,
        },
    };

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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let status = ensure_application_run_trace_projection_status(&state, id, run_id).await?;
    let projection_status = to_trace_projection_status_response(&status);
    let trace_node_uuid = parse_trace_projection_node_id(&trace_node_id)?;
    if !projection_is_succeeded(&status) {
        return Ok(Json(ApiSuccess::new(ApplicationRunTraceNodeContentResponse {
            trace_node_id,
            node_kind: "trace_projection".to_string(),
            projection_status,
            content_kind: "trace_projection".to_string(),
            source_refs: serde_json::Value::Array(Vec::new()),
            detail_refs: serde_json::Value::Array(Vec::new()),
            payload: serde_json::json!({}),
        })));
    }
    let node = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node(
        &state.store,
        run_id,
        trace_node_uuid,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("trace_node"))?;
    let content =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
            &state.store,
            run_id,
            trace_node_uuid,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node_content"))?;
    let content =
        if let Some(preview_request) = trace_node_artifact_preview_request(raw_query.as_deref()) {
            offload_trace_node_content_artifacts(
                state.clone(),
                context.actor.current_workspace_id,
                id,
                run_id,
                content,
                preview_request,
            )
            .await?
        } else {
            content
        };
    let response = trace_projection_node_content_response(node, content, projection_status)?;

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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let status = ensure_application_run_trace_projection_status(&state, id, run_id).await?;
    let projection_status = to_trace_projection_status_response(&status);
    let trace_node_uuid = parse_trace_projection_node_id(&trace_node_id)?;
    if !projection_is_succeeded(&status) {
        return Ok(Json(ApiSuccess::new(
            ApplicationRunTraceNodeDetailResponse {
                trace_node_id,
                node_kind: "trace_projection".to_string(),
                projection_status,
                detail_ref_id,
                detail_kind: "trace_projection".to_string(),
                source_refs: serde_json::Value::Array(Vec::new()),
                payload: serde_json::json!({}),
            },
        )));
    }
    let node = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node(
        &state.store,
        run_id,
        trace_node_uuid,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("trace_node"))?;
    let content =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
            &state.store,
            run_id,
            trace_node_uuid,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node_content"))?;
    let detail_ref = trace_node_content_detail_ref(&content.payload, &detail_ref_id)
        .ok_or(ControlPlaneError::NotFound("trace_node_detail_ref"))?;
    let detail_kind = detail_ref
        .get("detail_kind")
        .and_then(serde_json::Value::as_str)
        .ok_or(ControlPlaneError::Conflict("trace_node_detail_ref"))?
        .to_string();
    let preview_request = trace_node_artifact_preview_request(raw_query.as_deref());
    let payload = match detail_kind.as_str() {
        "node_run" => {
            let node_run_ids = trace_node_content_node_run_ids(&content.payload)?;
            let detail_flow_run_id =
                trace_node_content_source_flow_run_id(&content.payload)?.unwrap_or(run_id);
            let node_runs =
                <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_trace_node_run_details(
                    &state.store,
                    detail_flow_run_id,
                    node_run_ids,
                )
                .await?;
            let node_run = merge_trace_node_run_detail(&node_runs)
                .ok_or(ControlPlaneError::NotFound("node_run"))?;
            let node_run = if let Some(preview_request) = preview_request {
                offload_trace_node_run_detail_artifacts(
                    state.clone(),
                    context.actor.current_workspace_id,
                    id,
                    detail_flow_run_id,
                    node_run,
                    preview_request,
                )
                .await?
            } else {
                node_run
            };
            trace_node_run_detail_payload(node_run)
        }
        "checkpoints" => {
            let node_run_ids = trace_node_content_node_run_ids(&content.payload)?
                .into_iter()
                .collect::<HashSet<_>>();
            let detail_flow_run_id =
                trace_node_content_source_flow_run_id(&content.payload)?.unwrap_or(run_id);
            let detail =
                <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
                    &state.store,
                    id,
                    detail_flow_run_id,
                )
                .await?
                .ok_or(ControlPlaneError::NotFound("flow_run"))?;
            let checkpoints = detail
                .checkpoints
                .into_iter()
                .filter(|checkpoint| {
                    checkpoint
                        .node_run_id
                        .is_some_and(|node_run_id| node_run_ids.contains(&node_run_id))
                })
                .map(to_checkpoint_response)
                .collect::<Vec<_>>();

            serde_json::json!({ "checkpoints": checkpoints })
        }
        "events" => {
            let node_run_ids = trace_node_content_node_run_ids(&content.payload)?
                .into_iter()
                .collect::<HashSet<_>>();
            let detail_flow_run_id =
                trace_node_content_source_flow_run_id(&content.payload)?.unwrap_or(run_id);
            let detail =
                <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
                    &state.store,
                    id,
                    detail_flow_run_id,
                )
                .await?
                .ok_or(ControlPlaneError::NotFound("flow_run"))?;
            let events = detail
                .events
                .into_iter()
                .filter(|event| {
                    event
                        .node_run_id
                        .is_some_and(|node_run_id| node_run_ids.contains(&node_run_id))
                })
                .map(to_run_event_response)
                .collect::<Vec<_>>();

            serde_json::json!({ "events": events })
        }
        _ => return Err(ControlPlaneError::NotFound("trace_node_detail_ref").into()),
    };
    let response = ApplicationRunTraceNodeDetailResponse {
        trace_node_id,
        node_kind: node.node_kind,
        projection_status,
        detail_ref_id,
        detail_kind,
        source_refs: serde_json::Value::Array(vec![detail_ref]),
        payload,
    };

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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let status = ensure_application_run_trace_projection_status(&state, id, run_id).await?;
    let projection_status = to_trace_projection_status_response(&status);
    let trace_node_uuid = parse_trace_projection_node_id(&trace_node_id)?;
    if !projection_is_succeeded(&status) {
        return Ok(Json(ApiSuccess::new(
            ApplicationRunTraceToolCallbackContentResponse {
                trace_node_id,
                tool_call_id,
                projection_status,
                payload: serde_json::json!({}),
            },
        )));
    }
    let owner = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node(
        &state.store,
        run_id,
        trace_node_uuid,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("trace_node"))?;
    let tool_node =
        find_trace_projection_tool_callback_node(&state, run_id, &owner, &tool_call_id).await?;
    let content =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
            &state.store,
            run_id,
            tool_node.trace_node_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node_content"))?;
    let response = ApplicationRunTraceToolCallbackContentResponse {
        trace_node_id: tool_node.trace_node_id.to_string(),
        tool_call_id,
        projection_status,
        payload: content.payload,
    };

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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let events = detail
        .events
        .into_iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "public_run_resume_requested"
                    | "public_run_resume_succeeded"
                    | "public_run_resume_failed"
                    | "public_run_resume_cancelled"
                    | "flow_run_resumed"
            )
        })
        .map(to_run_event_response)
        .collect();
    let callback_tasks = detail
        .callback_tasks
        .into_iter()
        .map(to_callback_task_response)
        .collect();

    Ok(Json(ApiSuccess::new(
        ApplicationRunResumeTimelineResponse {
            flow_run: to_flow_run_response(detail.flow_run),
            callback_tasks,
            events,
        },
    )))
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
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let runtime_events = <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
        &state.store,
        run_id,
        0,
    )
    .await?;
    let detail =
        enrich_application_run_detail_visible_internal_llm_route_traces(detail, &runtime_events);

    let Some(node_run) = detail
        .node_runs
        .into_iter()
        .rev()
        .find(|candidate| candidate.node_id == node_id)
    else {
        return Ok(Json(ApiSuccess::new(None)));
    };

    let node_run_id = node_run.id;
    let checkpoints = detail
        .checkpoints
        .into_iter()
        .filter(|checkpoint| checkpoint.node_run_id == Some(node_run_id))
        .collect();
    let events = detail
        .events
        .into_iter()
        .filter(|event| event.node_run_id == Some(node_run_id))
        .collect();

    Ok(Json(ApiSuccess::new(Some(to_node_last_run_response(
        domain::NodeLastRun {
            flow_run: detail.flow_run,
            node_run,
            checkpoints,
            events,
        },
    )))))
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
    ensure_application_visible(&state, context.user.id, id).await?;

    <MainDurableStore as OrchestrationRuntimeRepository>::get_flow_run(&state.store, id, run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;

    let page_size = runtime_debug_stream_page_size(query.limit);
    let from_sequence = query.from_sequence.unwrap_or(0).max(0);
    let mut records =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_event_backfill_page(
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
    ensure_application_visible(&state, context.user.id, id).await?;

    let last_run = <MainDurableStore as OrchestrationRuntimeRepository>::get_latest_node_run(
        &state.store,
        id,
        &node_id,
    )
    .await?;
    let last_run = match last_run {
        Some(last_run) => {
            let runtime_events =
                <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
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
