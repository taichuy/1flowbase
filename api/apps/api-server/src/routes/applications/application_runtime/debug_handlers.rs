async fn ensure_application_visible(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
    application_id: Uuid,
) -> Result<domain::ApplicationRecord, ApiError> {
    Ok(ApplicationService::new(state.store.clone())
        .get_application(actor_user_id, application_id)
        .await?)
}

async fn ensure_application_non_crud_operation(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
    application_id: Uuid,
    operation: ApplicationNonCrudConsoleOperation,
) -> Result<domain::ApplicationRecord, ApiError> {
    Ok(ApplicationService::new(state.store.clone())
        .load_application_for_non_crud_console_operation(actor_user_id, application_id, operation)
        .await?)
}

async fn load_application_in_current_workspace(
    state: &Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
) -> Result<domain::ApplicationRecord, ApiError> {
    state
        .store
        .get_application(workspace_id, application_id)
        .await?
        .ok_or_else(|| ControlPlaneError::NotFound("application").into())
}

fn parse_runtime_event_cursor(run_id: Uuid, event_id: &str) -> Option<i64> {
    if let Ok(sequence) = event_id.parse::<i64>() {
        return Some(sequence);
    }

    let (cursor_run_id, sequence) = event_id.rsplit_once(':')?;
    if cursor_run_id != run_id.to_string() {
        return None;
    }

    sequence.parse::<i64>().ok()
}

fn debug_run_stream_from_sequence(
    run_id: Uuid,
    query: &DebugRunStreamQuery,
    headers: &HeaderMap,
) -> Option<i64> {
    query.from_sequence.or_else(|| {
        query
            .last_event_id
            .as_deref()
            .and_then(|event_id| parse_runtime_event_cursor(run_id, event_id))
            .or_else(|| {
                headers
                    .get("last-event-id")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|event_id| parse_runtime_event_cursor(run_id, event_id))
            })
    })
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/debug-runs",
    request_body = StartFlowDebugRunBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 201, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn start_flow_debug_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<StartFlowDebugRunBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ApplicationRunDetailResponse>>), ApiError> {
    let _http_activity = state
        .runtime_activity
        .start(id, ApplicationActivityKind::HttpRequest);
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let mcp_runtime_invoker = Arc::new(virtual_ui::ApiMcpRuntimeToolInvoker::new(
        state.clone(), headers.clone(), context.actor.clone(), Vec::new(),
    ).await?);

    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue());
    let detail = runtime_service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            input_payload: body.input_payload,
            document_snapshot: body.document,
            debug_session_id: body.debug_session_id,
        })
        .await?;
    let application =
        load_application_in_current_workspace(&state, context.actor.current_workspace_id, id)
            .await?;
    let flow_run_id = detail.flow_run.id;
    let workspace_id = context.actor.current_workspace_id;
    let background_state = state.clone();

    tokio::spawn(async move {
        let _execution_activity = background_state
            .runtime_activity
            .start(id, ApplicationActivityKind::ApplicationExecution);
        let background_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            api_provider_runtime(&background_state),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            background_state.api_node_id.clone(),
            background_state.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_state.file_storage_registry.clone())
        .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
        .with_llm_routing_counter_store(background_state.infrastructure.cache_store())
        .with_provider_request_log_queue(background_state.infrastructure.task_queue());
        let continue_result = scope_application_activity(
            id,
            background_service.continue_flow_debug_run(ContinueFlowDebugRunCommand {
                application_id: id,
                flow_run_id,
                workspace_id,
            }),
        )
        .await;
        match continue_result {
            Ok(detail) => {
                if let Err(error) = offload_application_run_detail_artifacts(
                    background_state.clone(),
                    workspace_id,
                    id,
                    detail,
                )
                .await
                {
                    error!(
                        application_id = %id,
                        flow_run_id = %flow_run_id,
                        error = %error.0,
                        "failed to offload flow debug artifacts"
                    );
                }
            }
            Err(error) => {
                error!(
                    application_id = %id,
                    flow_run_id = %flow_run_id,
                    error = %error,
                    "failed to continue flow debug run"
                );
            }
        }
    });

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_application_run_detail_response(
            &application,
            detail,
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/debug-runs/stream",
    request_body = StartFlowDebugRunBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("from_sequence" = Option<i64>, Query, description = "Resume after this stream sequence"),
        ("last_event_id" = Option<String>, Query, description = "SSE event cursor")
    ),
    responses(
        (status = 200, body = debug_run_stream::RuntimeDebugSseEventResponse, content_type = "text/event-stream"),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn start_flow_debug_run_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(stream_query): Query<DebugRunStreamQuery>,
    Json(body): Json<StartFlowDebugRunBody>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let _http_activity = state
        .runtime_activity
        .start(id, ApplicationActivityKind::HttpRequest);
    let request_received_at = std::time::Instant::now();
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let mcp_runtime_invoker = Arc::new(virtual_ui::ApiMcpRuntimeToolInvoker::new(
        state.clone(), headers.clone(), context.actor.clone(), Vec::new(),
    ).await?);

    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue());
    let shell = runtime_service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            input_payload: body.input_payload.clone(),
            document_snapshot: body.document.clone(),
            debug_session_id: body.debug_session_id.clone(),
        })
        .await?;
    let run_id = shell.id;
    let workspace_id = context.actor.current_workspace_id;
    let actor_user_id = context.user.id;

    state
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await?;
    let persister_handle = spawn_runtime_debug_event_persister(
        state.store.clone(),
        state.runtime_event_stream.clone(),
        run_id,
    );
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_accepted(run_id))
        .await?;
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::heartbeat())
        .await?;

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(debug_run_stream::send_runtime_event_stream(
        state.runtime_event_stream.clone(),
        Arc::new(state.store.clone()),
        run_id,
        debug_run_stream_from_sequence(run_id, &stream_query, &headers),
        sender,
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
        let _execution_activity = background_state
            .runtime_activity
            .start(id, ApplicationActivityKind::ApplicationExecution);
        let background_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            api_provider_runtime(&background_state),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            background_state.api_node_id.clone(),
            background_state.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_state.file_storage_registry.clone())
        .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
        .with_llm_routing_counter_store(background_state.infrastructure.cache_store())
        .with_provider_request_log_queue(background_state.infrastructure.task_queue())
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        let prepare_result = scope_application_activity(
            id,
            background_service.prepare_flow_debug_run_from_shell(PrepareFlowDebugRunCommand {
                actor_user_id,
                application_id: id,
                flow_run_id: run_id,
                input_payload: body.input_payload,
                document_snapshot: body.document,
                debug_session_id: body.debug_session_id.unwrap_or_default(),
            }),
        )
        .await;
        let result = match prepare_result {
            Ok(_) => {
                scope_application_activity(
                    id,
                    background_service.continue_flow_debug_run(ContinueFlowDebugRunCommand {
                        application_id: id,
                        flow_run_id: run_id,
                        workspace_id,
                    }),
                )
                .await
            }
            Err(error) => Err(error),
        };

        match result {
            Ok(detail) => {
                if matches!(
                    detail.flow_run.status,
                    domain::FlowRunStatus::Succeeded
                        | domain::FlowRunStatus::Incomplete
                        | domain::FlowRunStatus::Failed
                        | domain::FlowRunStatus::Cancelled
                ) {
                    project_runtime_event_stream_terminal(
                        background_state.runtime_event_stream.clone(),
                        &detail.flow_run,
                    )
                    .await;
                }
                if let Err(error) = offload_application_run_detail_artifacts(
                    background_state.clone(),
                    workspace_id,
                    id,
                    detail,
                )
                .await
                {
                    error!(
                        application_id = %id,
                        flow_run_id = %run_id,
                        error = %error.0,
                        "failed to offload streamed flow debug artifacts"
                    );
                }
            }
            Err(error) => {
                match background_state.store.get_flow_run(id, run_id).await {
                    Ok(Some(winner)) => {
                        project_runtime_event_stream_terminal(
                            background_state.runtime_event_stream.clone(),
                            &winner,
                        )
                        .await;
                    }
                    Ok(None) => {
                        error!(
                            application_id = %id,
                            flow_run_id = %run_id,
                            "failed streamed flow debug run has no durable winner to project"
                        );
                    }
                    Err(load_error) => {
                        error!(
                            application_id = %id,
                            flow_run_id = %run_id,
                            error = %load_error,
                            "failed to load durable winner for runtime stream projection"
                        );
                    }
                }
                error!(
                    application_id = %id,
                    flow_run_id = %run_id,
                    error = %error,
                    "failed to prepare and continue streamed flow debug run"
                );
            }
        }
        wait_for_runtime_debug_event_persister(persister_handle, id, run_id).await;
    });

    tracing::info!(
        application_id = %id,
        flow_run_id = %run_id,
        http_to_sse_open_ms = request_received_at.elapsed().as_millis() as u64,
        "flow debug stream opened"
    );

    let sse_activity = state
        .runtime_activity
        .start(id, ApplicationActivityKind::SseConnection);
    let stream = debug_run_stream::DebugRunSseStream::new(receiver).map(move |event| {
        let _keep_alive = &sse_activity;
        event
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/runs/{run_id}/debug-stream",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("from_sequence" = Option<i64>, Query, description = "Resume after this stream sequence"),
        ("last_event_id" = Option<String>, Query, description = "SSE event cursor")
    ),
    responses(
        (status = 200, body = debug_run_stream::RuntimeDebugSseEventResponse, content_type = "text/event-stream"),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn subscribe_flow_debug_run_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(stream_query): Query<DebugRunStreamQuery>,
) -> Result<Sse<debug_run_stream::DebugRunSseStream>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let flow_run = state
        .store
        .get_flow_run(id, run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if flow_run.created_by != context.user.id {
        return Err(ControlPlaneError::NotFound("flow_run").into());
    }

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(debug_run_stream::send_runtime_event_stream(
        state.runtime_event_stream.clone(),
        Arc::new(state.store.clone()),
        run_id,
        debug_run_stream_from_sequence(run_id, &stream_query, &headers),
        sender,
    ));

    Ok(Sse::new(debug_run_stream::DebugRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default()))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/runs/{run_id}/debug-snapshot",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_flow_debug_run_snapshot(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;
    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;

    if detail.flow_run.created_by != context.user.id {
        return Err(ControlPlaneError::NotFound("flow_run").into());
    }

    let runtime_events =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
            &state.store,
            run_id,
            0,
        )
        .await?;

    let detail = offload_application_run_detail_artifacts(
        state,
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    let mut response = to_application_run_detail_response(&application, detail);
    response.context_snapshot = to_context_snapshot_response(&runtime_events);

    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/runs/{run_id}/cancel",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn cancel_flow_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone());

    let detail = runtime_service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            flow_run_id: run_id,
        })
        .await?;
    let application =
        load_application_in_current_workspace(&state, context.actor.current_workspace_id, id)
            .await?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/runs/{run_id}/resume",
    request_body = ResumeFlowRunBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn resume_flow_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ResumeFlowRunBody>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let _http_activity = state
        .runtime_activity
        .start(id, ApplicationActivityKind::HttpRequest);
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let mcp_runtime_invoker = Arc::new(virtual_ui::ApiMcpRuntimeToolInvoker::new(
        state.clone(), headers.clone(), context.actor.clone(), Vec::new(),
    ).await?);

    let checkpoint_id = Uuid::parse_str(&body.checkpoint_id)
        .map_err(|_| ControlPlaneError::InvalidInput("checkpoint_id"))?;
    let detail = scope_application_activity(
        id,
        OrchestrationRuntimeService::new(
            state.store.clone(),
            api_provider_runtime(&state),
            state.runtime_engine.clone(),
            state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            state.api_node_id.clone(),
            state.provider_install_root.clone(),
        )
        .with_file_storage_registry(state.file_storage_registry.clone())
        .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
        .with_llm_routing_counter_store(state.infrastructure.cache_store())
        .with_provider_request_log_queue(state.infrastructure.task_queue())
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            flow_run_id: run_id,
            checkpoint_id,
            input_payload: body.input_payload,
        }),
    )
    .await?;
    let application =
        load_application_in_current_workspace(&state, context.actor.current_workspace_id, id)
            .await?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/callback-tasks/{callback_task_id}/complete",
    request_body = CompleteCallbackTaskBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("callback_task_id" = String, Path, description = "Callback task id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn complete_callback_task(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, callback_task_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CompleteCallbackTaskBody>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let _http_activity = state
        .runtime_activity
        .start(id, ApplicationActivityKind::HttpRequest);
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let mcp_runtime_invoker = Arc::new(virtual_ui::ApiMcpRuntimeToolInvoker::new(
        state.clone(), headers.clone(), context.actor.clone(), Vec::new(),
    ).await?);

    let detail = scope_application_activity(
        id,
        OrchestrationRuntimeService::new(
            state.store.clone(),
            api_provider_runtime(&state),
            state.runtime_engine.clone(),
            state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            state.api_node_id.clone(),
            state.provider_install_root.clone(),
        )
        .with_file_storage_registry(state.file_storage_registry.clone())
        .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
        .with_llm_routing_counter_store(state.infrastructure.cache_store())
        .with_provider_request_log_queue(state.infrastructure.task_queue())
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: context.user.id,
            application_id: id,
            callback_task_id,
            response_payload: body.response_payload,
        }),
    )
    .await?;
    let application =
        load_application_in_current_workspace(&state, context.actor.current_workspace_id, id)
            .await?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/nodes/{node_id}/debug-runs",
    request_body = StartNodeDebugPreviewBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("node_id" = String, Path, description = "Node id")
    ),
    responses(
        (status = 201, body = NodeLastRunResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn start_node_debug_preview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, node_id)): Path<(Uuid, String)>,
    Json(body): Json<StartNodeDebugPreviewBody>,
) -> Result<(StatusCode, Json<ApiSuccess<NodeLastRunResponse>>), ApiError> {
    let _http_activity = state
        .runtime_activity
        .start(id, ApplicationActivityKind::HttpRequest);
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let mcp_runtime_invoker = Arc::new(virtual_ui::ApiMcpRuntimeToolInvoker::new(
        state.clone(), headers.clone(), context.actor.clone(), Vec::new(),
    ).await?);

    let outcome = scope_application_activity(
        id,
        OrchestrationRuntimeService::new(
            state.store.clone(),
            api_provider_runtime(&state),
            state.runtime_engine.clone(),
            state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            state.api_node_id.clone(),
            state.provider_install_root.clone(),
        )
        .with_file_storage_registry(state.file_storage_registry.clone())
        .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
        .with_llm_routing_counter_store(state.infrastructure.cache_store())
        .with_provider_request_log_queue(state.infrastructure.task_queue())
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: context.user.id,
            application_id: id,
            node_id,
            input_payload: body.input_payload,
            document_snapshot: body.document,
            debug_session_id: body.debug_session_id,
        }),
    )
    .await?;

    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        domain::ApplicationRunDetail {
            flow_run: outcome.flow_run,
            node_runs: vec![outcome.node_run],
            checkpoints: Vec::new(),
            callback_tasks: Vec::new(),
            events: outcome.events,
            stitched_trace: Vec::new(),
            subagent_traces: Vec::new(),
        },
    )
    .await?;
    let node_run = detail
        .node_runs
        .into_iter()
        .next()
        .ok_or(ControlPlaneError::NotFound("node_run"))?;
    let response = to_node_last_run_response(domain::NodeLastRun {
        flow_run: detail.flow_run,
        node_run,
        checkpoints: Vec::new(),
        events: detail.events,
    });

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/debug-artifacts/{artifact_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("artifact_id" = String, Path, description = "Runtime debug artifact id")
    ),
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_debug_artifact(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, artifact_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    load_runtime_debug_artifact_response(state, context.actor.current_workspace_id, id, artifact_id)
        .await
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/debug-artifacts/resolve",
    request_body = ResolveRuntimeDebugArtifactsBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = ResolveRuntimeDebugArtifactsResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn resolve_runtime_debug_artifacts(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ResolveRuntimeDebugArtifactsBody>,
) -> Result<Json<ApiSuccess<ResolveRuntimeDebugArtifactsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    if body.artifact_refs.len() > RUNTIME_DEBUG_ARTIFACT_RESOLVE_MAX_REFS {
        return Err(ControlPlaneError::InvalidInput("artifact_refs").into());
    }

    let mut seen = HashSet::new();
    let mut artifacts = Vec::new();
    for artifact_id in body.artifact_refs {
        if !seen.insert(artifact_id) {
            continue;
        }

        let value = load_runtime_debug_artifact_json_value(
            state.clone(),
            context.actor.current_workspace_id,
            id,
            artifact_id,
        )
        .await?;
        artifacts.push(RuntimeDebugArtifactValueResponse {
            artifact_ref: artifact_id.to_string(),
            content_type: "application/json".to_string(),
            value,
        });
    }

    Ok(Json(ApiSuccess::new(
        ResolveRuntimeDebugArtifactsResponse { artifacts },
    )))
}
