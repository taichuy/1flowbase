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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-runs.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers: headers.clone() },
        interface_debug_commands::ApplicationRuntimeDebugCommandsInput::Start { application_id: id, body, headers },
    ).await?;
    let interface_debug_commands::ApplicationRuntimeDebugCommandsOutput::Run(response) = output else { unreachable!("debug run binding returned a different output") };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
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
    let stream = crate::routes::console_interface::invoke_server_stream::<
        interface_debug_commands::ApplicationRuntimeDebugStreamInput,
        interface_debug_commands::ApplicationRuntimeDebugStreamEvent,
        interface_debug_commands::ApplicationRuntimeDebugStreamOutput,
    >(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-runs.stream.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state,
            headers: headers.clone(),
        },
        interface_debug_commands::ApplicationRuntimeDebugStreamInput::Start {
            application_id: id,
            body,
            headers,
            stream_query,
        },
    )
    .await?;
    let (sender, receiver) = mpsc::channel(32);
    let (mut events, completion) = stream.into_parts();
    tokio::spawn(async move {
        while let Some(interface_debug_commands::ApplicationRuntimeDebugStreamEvent(event)) =
            events.recv().await
        {
            if sender.send(event).await.is_err() {
                return;
            }
        }
    });
    tokio::spawn(async move {
        let _ = completion.complete().await;
    });
    Ok(Sse::new(debug_run_stream::DebugRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default()))
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
    let from_sequence = debug_run_stream_from_sequence(run_id, &stream_query, &headers);
    let stream = crate::routes::console_interface::invoke_server_stream::<
        interface_debug_commands::ApplicationRuntimeDebugStreamInput,
        interface_debug_commands::ApplicationRuntimeDebugStreamEvent,
        interface_debug_commands::ApplicationRuntimeDebugStreamOutput,
    >(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-runs.stream.subscribe.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_debug_commands::ApplicationRuntimeDebugStreamInput::Subscribe {
            application_id: id,
            run_id,
            from_sequence,
        },
    )
    .await?;
    let (sender, receiver) = mpsc::channel(32);
    let (mut events, completion) = stream.into_parts();
    tokio::spawn(async move {
        while let Some(interface_debug_commands::ApplicationRuntimeDebugStreamEvent(event)) =
            events.recv().await
        {
            if sender.send(event).await.is_err() {
                return;
            }
        }
    });
    tokio::spawn(async move {
        let _ = completion.complete().await;
    });
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-snapshot.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_debug_artifacts::ApplicationRuntimeDebugArtifactsInput::Snapshot {
            application_id: id,
            run_id,
        },
    )
    .await?;
    let interface_debug_artifacts::ApplicationRuntimeDebugArtifactsOutput::Snapshot(response) =
        output
    else {
        unreachable!("runtime debug snapshot binding returned a different output")
    };
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state), "http.console.applications.runtime.runs.cancel.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface_debug_commands::ApplicationRuntimeDebugCommandsInput::Cancel { application_id: id, run_id },
    ).await?;
    let interface_debug_commands::ApplicationRuntimeDebugCommandsOutput::Run(response) = output else { unreachable!("debug cancel binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.runs.resume.v1", crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers: headers.clone() }, interface_debug_commands::ApplicationRuntimeDebugCommandsInput::Resume { application_id: id, run_id, body, headers }).await?;
    let interface_debug_commands::ApplicationRuntimeDebugCommandsOutput::Run(response) = output else { unreachable!("debug resume binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.callback-tasks.complete.v1", crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers: headers.clone() }, interface_debug_commands::ApplicationRuntimeDebugCommandsInput::CompleteCallback { application_id: id, callback_task_id, body, headers }).await?;
    let interface_debug_commands::ApplicationRuntimeDebugCommandsOutput::Run(response) = output else { unreachable!("callback completion binding returned a different output") };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = crate::routes::console_interface::invoke(Arc::clone(&state), "http.console.applications.runtime.nodes.debug-runs.create.v1", crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers: headers.clone() }, interface_debug_commands::ApplicationRuntimeDebugCommandsInput::StartNode { application_id: id, node_id, body, headers }).await?;
    let interface_debug_commands::ApplicationRuntimeDebugCommandsOutput::Node(response) = output else { unreachable!("node debug binding returned a different output") };
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-artifact.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_debug_artifacts::ApplicationRuntimeDebugArtifactsInput::Get {
            application_id: id,
            artifact_id,
        },
    )
    .await?;
    let interface_debug_artifacts::ApplicationRuntimeDebugArtifactsOutput::Content(content) = output
    else {
        unreachable!("runtime debug artifact binding returned a different output")
    };
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content.content_type)
        .body(axum::body::Body::from(content.bytes))
        .map_err(ApiError::from)
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-artifacts.resolve.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_debug_artifacts::ApplicationRuntimeDebugArtifactsInput::Resolve {
            application_id: id,
            body,
        },
    )
    .await?;
    let interface_debug_artifacts::ApplicationRuntimeDebugArtifactsOutput::Resolved(response) =
        output
    else {
        unreachable!("runtime debug artifacts resolve binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}
