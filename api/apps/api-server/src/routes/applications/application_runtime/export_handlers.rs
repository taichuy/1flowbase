const APPLICATION_RUN_TRACE_EXPORT_VERSION: i32 = 1;

struct ApplicationRunTraceExportDocument {
    title: String,
    started_at: OffsetDateTime,
    export_status: String,
    export_warning_count: usize,
    value: serde_json::Value,
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/export",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceExportResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn export_application_run_trace_dump(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsExport,
    )
    .await?;
    let exported_at = OffsetDateTime::now_utc();
    let document = build_application_run_trace_export_document(
        state,
        context.actor.current_workspace_id,
        &application,
        id,
        run_id,
        exported_at,
    )
    .await?;
    let body = serde_json::to_vec_pretty(&document.value)?;

    download_response(
        "application/json",
        &application_run_export_json_filename(&document.title, document.started_at, run_id),
        body,
    )
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/logs/runs/export",
    request_body = ApplicationRunSelectedExportBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, description = "Zip archive containing manifest.json and selected run JSON dumps", body = crate::openapi::OpenApiBinaryBody, content_type = "application/zip"),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn export_application_runs_zip(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ApplicationRunSelectedExportBody>,
) -> Result<axum::response::Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let application = ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsExport,
    )
    .await?;
    if body.run_ids.is_empty() {
        return Err(ControlPlaneError::InvalidInput("run_ids").into());
    }

    let exported_at = OffsetDateTime::now_utc();
    let exported_at_text = application_logs::format_time(exported_at);
    let mut documents = Vec::with_capacity(body.run_ids.len());
    let mut entry_paths = HashSet::new();
    let mut manifest_runs = Vec::with_capacity(body.run_ids.len());

    for (index, run_id) in body.run_ids.into_iter().enumerate() {
        let document = build_application_run_trace_export_document(
            state.clone(),
            context.actor.current_workspace_id,
            &application,
            id,
            run_id,
            exported_at,
        )
        .await?;
        let entry_path = unique_zip_entry_path(
            application_run_export_zip_entry_name(
                index + 1,
                document.started_at,
                run_id,
                &document.title,
            ),
            &mut entry_paths,
        );

        manifest_runs.push(ApplicationRunSelectedExportManifestRunResponse {
            run_id: run_id.to_string(),
            title: document.title.clone(),
            started_at: application_logs::format_time(document.started_at),
            filename: entry_path.clone(),
            export_status: document.export_status.clone(),
            export_warning_count: document.export_warning_count,
        });
        documents.push((entry_path, document));
    }

    let export_status = if manifest_runs
        .iter()
        .any(|run| run.export_warning_count > 0)
    {
        "complete_with_warnings"
    } else {
        "complete"
    };
    let run_count = manifest_runs.len();
    let selected_run_ids = manifest_runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    let manifest = ApplicationRunSelectedExportManifestResponse {
        export_version: APPLICATION_RUN_TRACE_EXPORT_VERSION,
        exported_at: exported_at_text,
        export_status: export_status.to_string(),
        application_id: id.to_string(),
        run_count,
        selected_run_ids,
        entries: manifest_runs,
    };
    let zip_bytes = build_selected_runs_zip(manifest, documents)?;
    let zip_filename = format!(
        "1flowbase-runs-{}-{}-{}runs.zip",
        short_run_id(id),
        format_export_filename_timestamp(exported_at),
        run_count
    );

    download_response("application/zip", &zip_filename, zip_bytes)
}

fn build_selected_runs_zip(
    manifest: ApplicationRunSelectedExportManifestResponse,
    documents: Vec<(String, ApplicationRunTraceExportDocument)>,
) -> Result<Vec<u8>, ApiError> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    writer.start_file("manifest.json", options)?;
    std::io::Write::write_all(&mut writer, &serde_json::to_vec_pretty(&manifest)?)?;
    for (entry_path, document) in documents {
        writer.start_file(entry_path, options)?;
        std::io::Write::write_all(&mut writer, &serde_json::to_vec_pretty(&document.value)?)?;
    }

    Ok(writer.finish()?.into_inner())
}

async fn build_application_run_trace_export_document(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application: &domain::ApplicationRecord,
    application_id: Uuid,
    run_id: Uuid,
    exported_at: OffsetDateTime,
) -> Result<ApplicationRunTraceExportDocument, ApiError> {
    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        application_id,
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
    let detail = enrich_application_run_detail_visible_internal_llm_route_traces(
        detail,
        &runtime_events,
    );
    let title = detail.flow_run.title.clone();
    let started_at = detail.flow_run.started_at;
    let trace_tree =
        build_application_run_trace_export_tree(state.clone(), application, &detail.flow_run)
            .await?;
    let detail_response = to_application_run_detail_response(application, detail);
    let response = ApplicationRunTraceExportResponse {
        export_version: APPLICATION_RUN_TRACE_EXPORT_VERSION,
        exported_at: application_logs::format_time(exported_at),
        export_status: "complete".to_string(),
        export_warnings: Vec::new(),
        run: detail_response.run,
        statistics: detail_response.statistics,
        detail: detail_response.detail,
        flow_run: detail_response.flow_run,
        answer_snapshot: detail_response.answer_snapshot,
        node_runs: detail_response.node_runs,
        checkpoints: detail_response.checkpoints,
        callback_tasks: detail_response.callback_tasks,
        events: detail_response.events,
        stitched_trace: detail_response.stitched_trace,
        trace_tree,
    };
    let mut value = serde_json::to_value(response)?;
    backfill_export_node_run_error_payloads(&state, run_id, &mut value).await?;
    let mut warnings = Vec::new();
    let mut artifact_cache = std::collections::HashMap::new();
    let mut visiting_artifacts = HashSet::new();
    value = materialize_export_artifacts(MaterializeExportArtifactsInput {
        state,
        workspace_id,
        application_id,
        value,
        warnings: &mut warnings,
        artifact_cache: &mut artifact_cache,
        visiting_artifacts: &mut visiting_artifacts,
        source: "$".to_string(),
    })
    .await;
    let export_status = if warnings.is_empty() {
        "complete"
    } else {
        "complete_with_warnings"
    };
    let warning_count = warnings.len();
    let object = value
        .as_object_mut()
        .ok_or(ControlPlaneError::Conflict("application_run_export"))?;
    object.insert(
        "export_status".to_string(),
        serde_json::Value::String(export_status.to_string()),
    );
    object.insert(
        "export_warnings".to_string(),
        serde_json::to_value(&warnings)?,
    );

    Ok(ApplicationRunTraceExportDocument {
        title,
        started_at,
        export_status: export_status.to_string(),
        export_warning_count: warning_count,
        value,
    })
}

async fn backfill_export_node_run_error_payloads(
    state: &Arc<ApiState>,
    run_id: Uuid,
    value: &mut serde_json::Value,
) -> Result<(), ApiError> {
    let rows = sqlx::query(
        r#"
        select id, error_payload
        from node_runs
        where flow_run_id = $1
          and error_payload is not null
        "#,
    )
    .bind(run_id)
    .fetch_all(state.store.pool())
    .await?;
    if rows.is_empty() {
        return Ok(());
    }

    let error_payloads = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<Uuid, _>("id").to_string(),
                row.get::<serde_json::Value, _>("error_payload"),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let Some(node_runs) = value
        .get_mut("node_runs")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };

    for node_run in node_runs {
        let Some(node_run_object) = node_run.as_object_mut() else {
            continue;
        };
        let Some(node_run_id) = node_run_object
            .get("id")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if !node_run_object
            .get("error_payload")
            .is_none_or(serde_json::Value::is_null)
        {
            continue;
        }
        if let Some(error_payload) = error_payloads.get(node_run_id) {
            node_run_object.insert("error_payload".to_string(), error_payload.clone());
        }
    }

    Ok(())
}

async fn build_application_run_trace_export_tree(
    state: Arc<ApiState>,
    application: &domain::ApplicationRecord,
    flow_run: &domain::FlowRunRecord,
) -> Result<ApplicationRunTraceExportTreeResponse, ApiError> {
    let status =
        ensure_application_run_trace_projection_status(&state, application.id, flow_run.id).await?;
    let projection_status = to_trace_projection_status_response(&status);
    let statistics = if projection_is_succeeded(&status) {
        to_trace_projection_statistics_response(
            <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_statistics(
                &state.store,
                flow_run.id,
            )
            .await?,
        )
    } else {
        empty_trace_projection_statistics_response()
    };
    let nodes = if projection_is_succeeded(&status) {
        let roots =
            <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_trace_roots(
                &state.store,
                flow_run.id,
            )
            .await?;
        let mut nodes = Vec::with_capacity(roots.len());
        for root in roots {
            nodes.push(build_application_run_trace_export_node(state.clone(), flow_run.id, root).await?);
        }
        nodes
    } else {
        Vec::new()
    };

    Ok(ApplicationRunTraceExportTreeResponse {
        run: application_run_log_response_for_trace_tree(application, flow_run),
        statistics,
        flow_run: to_flow_run_response(flow_run.clone()),
        answer_snapshot: None,
        projection_status,
        nodes,
    })
}

fn build_application_run_trace_export_node(
    state: Arc<ApiState>,
    flow_run_id: Uuid,
    node: domain::ApplicationRunTraceNodeRecord,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<ApplicationRunTraceExportNodeResponse, ApiError>> + Send>,
> {
    Box::pin(async move {
        let summary = to_trace_node_summary_from_projection(node.clone());
        let (content_kind, source_refs, detail_refs, payload) = if node.has_content {
            match <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
                &state.store,
                flow_run_id,
                node.trace_node_id,
            )
            .await?
            {
                Some(content) => {
                    let detail_refs = content
                        .payload
                        .get("detail_refs")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
                    (
                        Some(content.content_kind),
                        content.source_refs,
                        detail_refs,
                        trace_node_content_raw_payload_response(content.payload),
                    )
                }
                None => (
                    None,
                    serde_json::Value::Array(Vec::new()),
                    serde_json::Value::Array(Vec::new()),
                    serde_json::json!({}),
                ),
            }
        } else {
            (
                None,
                serde_json::Value::Array(Vec::new()),
                serde_json::Value::Array(Vec::new()),
                serde_json::json!({}),
            )
        };
        let child_records =
            list_application_run_trace_export_children(&state, flow_run_id, node.trace_node_id)
                .await?;
        let mut children = Vec::with_capacity(child_records.len());
        for child in child_records {
            children
                .push(build_application_run_trace_export_node(state.clone(), flow_run_id, child).await?);
        }

        Ok(ApplicationRunTraceExportNodeResponse {
            trace_node_id: summary.trace_node_id,
            stable_locator: summary.stable_locator,
            parent_trace_node_id: summary.parent_trace_node_id,
            node_kind: summary.node_kind,
            flow_run_id: summary.flow_run_id,
            node_run_id: summary.node_run_id,
            callback_task_id: summary.callback_task_id,
            node_id: summary.node_id,
            node_type: summary.node_type,
            node_mode: summary.node_mode,
            node_alias: summary.node_alias,
            status: summary.status,
            started_at: summary.started_at,
            finished_at: summary.finished_at,
            duration_ms: summary.duration_ms,
            metrics_payload: summary.metrics_payload,
            has_children: summary.has_children,
            child_count: summary.child_count,
            has_content: summary.has_content,
            source_flow_run_id: summary.source_flow_run_id,
            source_trace_node_id: summary.source_trace_node_id,
            parent_callback_task_id: summary.parent_callback_task_id,
            parent_tool_call_id: summary.parent_tool_call_id,
            trace_relation_kind: summary.trace_relation_kind,
            content_kind,
            source_refs,
            detail_refs,
            payload,
            children,
        })
    })
}

async fn list_application_run_trace_export_children(
    state: &Arc<ApiState>,
    flow_run_id: Uuid,
    parent_trace_node_id: Uuid,
) -> Result<Vec<domain::ApplicationRunTraceNodeRecord>, ApiError> {
    let mut cursor = None;
    let mut items = Vec::new();

    loop {
        let page =
            <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
                &state.store,
                ListApplicationRunTraceChildrenPageInput {
                    flow_run_id,
                    parent_trace_node_id,
                    page_size: APPLICATION_RUN_TRACE_CHILDREN_MAX_PAGE_SIZE,
                    cursor,
                },
            )
            .await?;
        cursor = page.next_cursor;
        items.extend(page.items);
        if !page.has_more {
            return Ok(items);
        }
    }
}

fn empty_trace_projection_statistics_response() -> application_logs::ApplicationRunStatisticsResponse {
    to_trace_projection_statistics_response(ApplicationRunTraceProjectionStatistics {
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
        input_cache_hit_tokens: None,
        unique_node_count: 0,
        tool_callback_count: 0,
    })
}

struct MaterializeExportArtifactsInput<'a> {
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application_id: Uuid,
    value: serde_json::Value,
    warnings: &'a mut Vec<ApplicationRunTraceExportWarningResponse>,
    artifact_cache: &'a mut std::collections::HashMap<Uuid, serde_json::Value>,
    visiting_artifacts: &'a mut HashSet<Uuid>,
    source: String,
}

fn materialize_export_artifacts<'a>(
    input: MaterializeExportArtifactsInput<'a>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + 'a>> {
    Box::pin(async move {
        let MaterializeExportArtifactsInput {
            state,
            workspace_id,
            application_id,
            value,
            warnings,
            artifact_cache,
            visiting_artifacts,
            source,
        } = input;

        if let Some(artifact_ref) = runtime_debug_artifact_ref(&value) {
            if let Some(cached) = artifact_cache.get(&artifact_ref).cloned() {
                return cached;
            }
            if !visiting_artifacts.insert(artifact_ref) {
                warnings.push(ApplicationRunTraceExportWarningResponse {
                    code: "runtime_debug_artifact_cycle_skipped".to_string(),
                    source: source.clone(),
                    message: format!("runtime debug artifact {artifact_ref} was already being materialized"),
                });
                return value;
            }
            match load_runtime_debug_artifact_json_value(
                state.clone(),
                workspace_id,
                application_id,
                artifact_ref,
            )
            .await
            {
                Ok(full_value) => {
                    let materialized = materialize_export_artifacts(MaterializeExportArtifactsInput {
                        state,
                        workspace_id,
                        application_id,
                        value: full_value,
                        warnings,
                        artifact_cache,
                        visiting_artifacts,
                        source,
                    })
                    .await;
                    visiting_artifacts.remove(&artifact_ref);
                    artifact_cache.insert(artifact_ref, materialized.clone());
                    return materialized;
                }
                Err(error) => {
                    visiting_artifacts.remove(&artifact_ref);
                    warnings.push(ApplicationRunTraceExportWarningResponse {
                        code: "runtime_debug_artifact_materialize_failed".to_string(),
                        source: source.clone(),
                        message: error.0.to_string(),
                    });
                    return value;
                }
            }
        }

        match value {
            serde_json::Value::Array(items) => {
                let mut materialized = Vec::with_capacity(items.len());
                for (index, item) in items.into_iter().enumerate() {
                    materialized.push(
                        materialize_export_artifacts(MaterializeExportArtifactsInput {
                            state: state.clone(),
                            workspace_id,
                            application_id,
                            value: item,
                            warnings,
                            artifact_cache,
                            visiting_artifacts,
                            source: format!("{source}[{index}]"),
                        })
                        .await,
                    );
                }
                serde_json::Value::Array(materialized)
            }
            serde_json::Value::Object(object) => {
                let mut materialized = serde_json::Map::with_capacity(object.len());
                for (key, item) in object {
                    let child_source = format!("{source}.{key}");
                    let child = materialize_export_artifacts(MaterializeExportArtifactsInput {
                        state: state.clone(),
                        workspace_id,
                        application_id,
                        value: item,
                        warnings,
                        artifact_cache,
                        visiting_artifacts,
                        source: child_source,
                    })
                    .await;
                    materialized.insert(key, child);
                }
                serde_json::Value::Object(materialized)
            }
            value => value,
        }
    })
}

fn runtime_debug_artifact_ref(value: &serde_json::Value) -> Option<Uuid> {
    let object = value.as_object()?;
    if !object
        .get("__runtime_debug_artifact")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    object
        .get("artifact_ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn download_response(
    content_type: &'static str,
    filename: &str,
    body: Vec<u8>,
) -> Result<axum::response::Response, ApiError> {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(body))
        .map_err(ApiError::from)
}

fn application_run_export_json_filename(
    title: &str,
    started_at: OffsetDateTime,
    run_id: Uuid,
) -> String {
    format!(
        "1flowbase-run-{}-{}-{}.json",
        safe_filename_segment(title),
        format_export_filename_timestamp(started_at),
        short_run_id(run_id),
    )
}

fn application_run_export_zip_entry_name(
    index: usize,
    started_at: OffsetDateTime,
    run_id: Uuid,
    title: &str,
) -> String {
    format!(
        "runs/{index:03}_{}_{}_{}.json",
        format_export_filename_timestamp(started_at),
        short_run_id(run_id),
        safe_filename_segment(title),
    )
}

fn unique_zip_entry_path(path: String, used: &mut HashSet<String>) -> String {
    if used.insert(path.clone()) {
        return path;
    }

    let stem = path.strip_suffix(".json").unwrap_or(&path);
    for suffix in 2.. {
        let candidate = format!("{stem}-{suffix}.json");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded suffix loop must return a unique zip entry path")
}

fn safe_filename_segment(value: &str) -> String {
    let mut segment = String::new();
    let mut last_was_separator = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if last_was_separator && !segment.is_empty() {
                segment.push('-');
            }
            segment.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if matches!(character, '-' | '_' | '.' | ' ') || character.is_whitespace() {
            last_was_separator = true;
        }

        if segment.len() >= 64 {
            break;
        }
    }

    let segment = segment.trim_matches('-').to_string();
    if segment.is_empty() {
        "untitled".to_string()
    } else {
        segment
    }
}

fn format_export_filename_timestamp(value: OffsetDateTime) -> String {
    value
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.to_string())
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn short_run_id(value: Uuid) -> String {
    value.to_string().chars().take(8).collect()
}
