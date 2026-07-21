use sha2::{Digest, Sha256};

use control_plane::{
    flow::FlowService,
    ports::{UpdateFlowRunInput, UpdateFlowRunPayloadsInput},
};

const RUN_ARCHIVE_VERSION: i32 = 1;
const APPLICATION_RUN_ARCHIVE_SEMANTICS: &str = "application_run_archive_v1";
const RUN_ARCHIVE_UPLOAD_MAX_BYTES: i64 = 100 * 1024 * 1024;
const RUN_ARCHIVE_UPLOAD_MAX_CHUNK_BYTES: i64 = 8 * 1024 * 1024;
const RUN_ARCHIVE_UPLOAD_MAX_CHUNKS: i64 = 4096;

#[derive(Debug)]
struct RunArchiveUploadSessionRow {
    session_id: Uuid,
    scope_id: Uuid,
    application_id: Uuid,
    status: String,
    filename: Option<String>,
    total_size_bytes: i64,
    received_bytes: i64,
    expected_sha256: Option<String>,
    chunk_size_bytes: i64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug)]
struct RunArchiveImportJobRow {
    job_id: Uuid,
    application_id: Uuid,
    upload_session_id: Uuid,
    status: String,
    archive_version: Option<i32>,
    archive_sha256: Option<String>,
    run_count: i32,
    imported_run_count: i32,
    error_payload: Option<serde_json::Value>,
    result_payload: serde_json::Value,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    started_at: Option<OffsetDateTime>,
    finished_at: Option<OffsetDateTime>,
}

struct CreateRunArchiveImportJobInput<'a> {
    workspace_id: Uuid,
    application_id: Uuid,
    actor_user_id: Uuid,
    session_id: Uuid,
    archive_version: i32,
    archive_sha256: &'a str,
    run_count: i32,
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/archive",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("archive_version" = Option<i32>, Query, description = "Archive contract version, currently 1")
    ),
    responses(
        (status = 200, body = RunArchiveV1Response),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
// Compatibility export endpoint; user-facing run export uses the trace zip path.
#[deprecated(note = "Use selected trace export zip for user-facing run export.")]
pub async fn export_application_run_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ApplicationRunArchiveQuery>,
) -> Result<axum::response::Response, ApiError> {
    ensure_run_archive_version(query.archive_version)?;
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsExport,
    )
    .await?;
    let archive = build_run_archive_v1_document(
        state,
        context.actor.current_workspace_id,
        context.actor.user_id,
        &application,
        vec![run_id],
        OffsetDateTime::now_utc(),
    )
    .await?;
    let filename = application_run_archive_filename(
        &archive.source.application_name,
        &archive.exported_at,
        archive.entries.len(),
    );
    let body = serde_json::to_vec_pretty(&archive)?;

    download_response("application/json", &filename, body)
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/logs/runs/archive",
    request_body = ApplicationRunArchiveBody,
    params(("id" = String, Path, description = "Application id")),
    responses(
        (status = 200, body = RunArchiveV1Response),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
// Compatibility export endpoint; user-facing run export uses the trace zip path.
#[deprecated(note = "Use selected trace export zip for user-facing run export.")]
pub async fn export_application_runs_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ApplicationRunArchiveBody>,
) -> Result<axum::response::Response, ApiError> {
    ensure_run_archive_version(body.archive_version)?;
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

    let archive = build_run_archive_v1_document(
        state,
        context.actor.current_workspace_id,
        context.actor.user_id,
        &application,
        body.run_ids,
        OffsetDateTime::now_utc(),
    )
    .await?;
    let filename = application_run_archive_filename(
        &archive.source.application_name,
        &archive.exported_at,
        archive.entries.len(),
    );
    let body = serde_json::to_vec_pretty(&archive)?;

    download_response("application/json", &filename, body)
}

fn ensure_run_archive_version(version: Option<i32>) -> Result<(), ApiError> {
    if version.unwrap_or(RUN_ARCHIVE_VERSION) == RUN_ARCHIVE_VERSION {
        return Ok(());
    }

    Err(ControlPlaneError::InvalidInput("unsupported_archive_version").into())
}

fn required_json_field<T>(value: &serde_json::Value, field: &'static str) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let field_value = value
        .get(field)
        .cloned()
        .ok_or(ControlPlaneError::Conflict(field))?;
    Ok(serde_json::from_value(field_value)?)
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/logs/runs/archive/import-sessions",
    request_body = RunArchiveUploadSessionCreateBody,
    params(("id" = String, Path, description = "Application id")),
    responses(
        (status = 201, body = RunArchiveUploadSessionResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_run_archive_upload_session(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RunArchiveUploadSessionCreateBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<RunArchiveUploadSessionResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let application = ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsImport,
    )
    .await?;
    if body.total_size_bytes <= 0 {
        return Err(ControlPlaneError::InvalidInput("total_size_bytes").into());
    }
    if body.total_size_bytes > RUN_ARCHIVE_UPLOAD_MAX_BYTES {
        return Err(ControlPlaneError::InvalidInput("archive_size").into());
    }
    let expected_sha256 = body
        .expected_sha256
        .as_deref()
        .ok_or(ControlPlaneError::InvalidInput("expected_sha256"))?;
    ensure_sha256_value(expected_sha256, "expected_sha256")?;
    let chunk_size_bytes = body
        .chunk_size_bytes
        .ok_or(ControlPlaneError::InvalidInput("chunk_size_bytes"))?;
    if chunk_size_bytes <= 0 || chunk_size_bytes > RUN_ARCHIVE_UPLOAD_MAX_CHUNK_BYTES {
        return Err(ControlPlaneError::InvalidInput("chunk_size_bytes").into());
    }
    let expected_chunk_count =
        expected_archive_chunk_count(body.total_size_bytes, chunk_size_bytes)?;
    if expected_chunk_count > RUN_ARCHIVE_UPLOAD_MAX_CHUNKS {
        return Err(ControlPlaneError::InvalidInput("archive_chunk_count").into());
    }

    let session_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into run_archive_upload_sessions (
            id,
            scope_id,
            application_id,
            actor_user_id,
            original_filename,
            total_size_bytes,
            expected_sha256,
            chunk_size_bytes,
            status,
            created_by,
            updated_by
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, 'uploading', $9, $9)
        "#,
    )
    .bind(session_id)
    .bind(application.workspace_id)
    .bind(application.id)
    .bind(context.actor.user_id)
    .bind(body.filename.as_deref())
    .bind(body.total_size_bytes)
    .bind(expected_sha256)
    .bind(chunk_size_bytes)
    .bind(context.actor.user_id)
    .execute(state.store.pool())
    .await?;

    let session = load_run_archive_upload_session(&state, id, session_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_upload_session_response(session))),
    ))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{id}/logs/runs/archive/import-sessions/{session_id}/chunks/{chunk_index}",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    params(
        ("id" = String, Path, description = "Application id"),
        ("session_id" = String, Path, description = "Upload session id"),
        ("chunk_index" = i32, Path, description = "Zero-based chunk index"),
        ("x-chunk-sha256" = String, Header, description = "SHA-256 digest of this chunk")
    ),
    responses(
        (status = 200, body = RunArchiveChunkUploadResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn upload_run_archive_chunk(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, session_id, chunk_index)): Path<(Uuid, Uuid, i32)>,
    body: axum::body::Bytes,
) -> Result<Json<ApiSuccess<RunArchiveChunkUploadResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsImport,
    )
    .await?;
    if chunk_index < 0 || body.is_empty() {
        return Err(ControlPlaneError::InvalidInput("archive_chunk").into());
    }
    let session = load_run_archive_upload_session(&state, id, session_id).await?;
    if session.status != "uploading" {
        return Err(ControlPlaneError::Conflict("archive_upload_session").into());
    }
    if i64::try_from(body.len()).unwrap_or(i64::MAX) > session.chunk_size_bytes {
        return Err(ControlPlaneError::InvalidInput("chunk_size_bytes").into());
    }
    let expected_chunk_count =
        expected_archive_chunk_count(session.total_size_bytes, session.chunk_size_bytes)?;
    if i64::from(chunk_index) >= expected_chunk_count {
        return Err(ControlPlaneError::InvalidInput("archive_chunk_count").into());
    }

    let actual_sha256 = sha256_bytes(&body);
    let expected_sha256 = header_value(&headers, "x-chunk-sha256")
        .ok_or(ControlPlaneError::InvalidInput("chunk_sha256"))?;
    ensure_sha256_value(&expected_sha256, "chunk_sha256")?;
    if normalize_sha256(&expected_sha256) != normalize_sha256(&actual_sha256) {
        return Err(ControlPlaneError::InvalidInput("chunk_sha256").into());
    }

    let chunk_id = Uuid::now_v7();
    let mut tx = state.store.pool().begin().await?;
    sqlx::query(
        r#"
        insert into run_archive_upload_chunks (
            id,
            scope_id,
            session_id,
            chunk_index,
            chunk_size_bytes,
            chunk_sha256,
            content,
            created_by,
            updated_by
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
        on conflict (session_id, chunk_index) do update
        set chunk_size_bytes = excluded.chunk_size_bytes,
            chunk_sha256 = excluded.chunk_sha256,
            content = excluded.content,
            updated_at = now(),
            updated_by = excluded.updated_by
        "#,
    )
    .bind(chunk_id)
    .bind(session.scope_id)
    .bind(session_id)
    .bind(chunk_index)
    .bind(i64::try_from(body.len()).unwrap_or(i64::MAX))
    .bind(&actual_sha256)
    .bind(body.as_ref())
    .bind(context.actor.user_id)
    .execute(&mut *tx)
    .await?;
    let received_bytes =
        refresh_run_archive_upload_session_received_bytes(&mut tx, session_id).await?;
    if received_bytes > session.total_size_bytes {
        return Err(ControlPlaneError::InvalidInput("archive_size").into());
    }
    tx.commit().await?;

    Ok(Json(ApiSuccess::new(RunArchiveChunkUploadResponse {
        session_id: session_id.to_string(),
        chunk_index,
        chunk_size_bytes: i64::try_from(body.len()).unwrap_or(i64::MAX),
        chunk_sha256: actual_sha256,
        received_bytes,
        status: "uploading".to_string(),
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/logs/runs/archive/import-sessions/{session_id}/complete",
    params(
        ("id" = String, Path, description = "Application id"),
        ("session_id" = String, Path, description = "Upload session id")
    ),
    responses(
        (status = 200, body = RunArchiveImportJobResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn complete_run_archive_upload_session(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<RunArchiveImportJobResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let application = ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsImport,
    )
    .await?;
    let session = load_run_archive_upload_session(&state, id, session_id).await?;
    if session.status != "uploading" {
        return Err(ControlPlaneError::Conflict("archive_upload_session").into());
    }

    let archive_bytes = load_upload_session_archive_bytes(&state, session_id).await?;
    if i64::try_from(archive_bytes.len()).unwrap_or(i64::MAX) != session.total_size_bytes {
        return Err(ControlPlaneError::InvalidInput("archive_size").into());
    }
    let archive_sha256 = sha256_bytes(&archive_bytes);
    let expected_sha256 = session
        .expected_sha256
        .as_deref()
        .ok_or(ControlPlaneError::InvalidInput("expected_sha256"))?;
    ensure_sha256_value(expected_sha256, "expected_sha256")?;
    if normalize_sha256(expected_sha256) != normalize_sha256(&archive_sha256) {
        return Err(ControlPlaneError::InvalidInput("archive_sha256").into());
    }
    let archive = parse_run_archive_v1(&archive_bytes)?;
    let job_id = create_run_archive_import_job(
        &state,
        CreateRunArchiveImportJobInput {
            workspace_id: application.workspace_id,
            application_id: application.id,
            actor_user_id: context.actor.user_id,
            session_id,
            archive_version: archive.archive_version,
            archive_sha256: &archive_sha256,
            run_count: i32::try_from(archive.entries.len()).unwrap_or(i32::MAX),
        },
    )
    .await?;

    mark_upload_session_completed(&state, session_id).await?;
    cleanup_run_archive_upload_chunks(&state, session_id).await?;
    let restore_state = state.clone();
    let restore_actor_user_id = context.actor.user_id;
    tokio::spawn(async move {
        let restore_result = restore_run_archive_v1(
            restore_state.clone(),
            &application,
            restore_actor_user_id,
            job_id,
            archive,
        )
        .await;
        if let Err(error) = restore_result {
            error!("run archive restore failed: {}", error.0);
            if let Err(mark_error) =
                mark_run_archive_import_job_failed(&restore_state, job_id, error.0.to_string())
                    .await
            {
                error!(
                    "failed to mark run archive import job failed: {}",
                    mark_error.0
                );
            }
        }
    });

    let job = load_run_archive_import_job(&state, id, job_id).await?;
    Ok(Json(ApiSuccess::new(
        to_import_job_response(&state, job).await?,
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/archive/import-jobs/{job_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("job_id" = String, Path, description = "Import job id")
    ),
    responses(
        (status = 200, body = RunArchiveImportJobResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_run_archive_import_job(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<RunArchiveImportJobResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_non_crud_operation(
        &state,
        context.user.id,
        id,
        ApplicationNonCrudConsoleOperation::LogsImport,
    )
    .await?;
    let job = load_run_archive_import_job(&state, id, job_id).await?;

    Ok(Json(ApiSuccess::new(
        to_import_job_response(&state, job).await?,
    )))
}

async fn build_run_archive_v1_document(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    application: &domain::ApplicationRecord,
    run_ids: Vec<Uuid>,
    exported_at: OffsetDateTime,
) -> Result<RunArchiveV1Response, ApiError> {
    let mut entries = Vec::with_capacity(run_ids.len());
    for run_id in &run_ids {
        let entry = build_run_archive_v1_entry(
            state.clone(),
            workspace_id,
            application,
            application.id,
            *run_id,
            exported_at,
        )
        .await?;
        entries.push(entry);
    }
    let manifest_entries = finalize_run_archive_v1_entries(&mut entries)?;
    let content_sha256 = run_archive_v1_entries_content_sha256(&entries)?;
    let exported_at_text = application_logs::format_time(exported_at);
    let selected_run_ids = run_ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    let manifest = RunArchiveV1ManifestResponse {
        archive_version: RUN_ARCHIVE_VERSION,
        archive_semantics: APPLICATION_RUN_ARCHIVE_SEMANTICS.to_string(),
        exported_at: exported_at_text.clone(),
        source_workspace_id: application.workspace_id.to_string(),
        source_application_id: application.id.to_string(),
        run_count: entries.len(),
        selected_run_ids,
        entries: manifest_entries,
        content_sha256: content_sha256.clone(),
        checksum: content_sha256.clone(),
    };
    let source = RunArchiveV1SourceResponse {
        source_kind: "application_run".to_string(),
        workspace_id: application.workspace_id.to_string(),
        application_id: application.id.to_string(),
        application_type: application.application_type.as_str().to_string(),
        application_name: application.name.clone(),
        exported_by_user_id: actor_user_id.to_string(),
        exported_at: exported_at_text.clone(),
        archive_builder: "api-server.application-runtime.run-archive-v1".to_string(),
    };

    Ok(RunArchiveV1Response {
        archive_version: RUN_ARCHIVE_VERSION,
        exported_at: exported_at_text,
        manifest,
        source,
        entries,
        content_digest: content_sha256,
    })
}

async fn build_run_archive_v1_entry(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application: &domain::ApplicationRecord,
    application_id: Uuid,
    run_id: Uuid,
    exported_at: OffsetDateTime,
) -> Result<RunArchiveV1EntryResponse, ApiError> {
    let export_document = build_application_run_trace_export_document(
        state.clone(),
        workspace_id,
        application,
        application_id,
        run_id,
        exported_at,
    )
    .await?;
    let export_value = export_document.value.clone();
    let flow_run = required_json_field(&export_value, "flow_run")?;
    let node_runs = required_json_field(&export_value, "node_runs")?;
    let checkpoints = required_json_field(&export_value, "checkpoints")?;
    let callback_tasks = required_json_field(&export_value, "callback_tasks")?;
    let events = required_json_field(&export_value, "events")?;
    let mut trace_tree = export_value
        .get("trace_tree")
        .cloned()
        .ok_or(ControlPlaneError::Conflict(
            "application_run_archive_trace_tree",
        ))?;
    normalize_run_archive_trace_tree_projection_status(&mut trace_tree);
    let export_warnings = required_json_field(&export_value, "export_warnings")?;
    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        application_id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let compiled_plan = match detail.flow_run.compiled_plan_id {
        Some(compiled_plan_id) => {
            <MainDurableStore as OrchestrationRuntimeRepository>::get_compiled_plan(
                &state.store,
                compiled_plan_id,
            )
            .await?
            .map(serde_json::to_value)
            .transpose()?
        }
        None => None,
    };
    let runtime_spans = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_spans(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let runtime_events = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
            &state.store,
            run_id,
            0,
        )
        .await?,
    )?;
    let runtime_items = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_items(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let context_projections = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_context_projections(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let usage_ledger = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_usage_ledger(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let model_failover_attempts = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_model_failover_attempt_ledger(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let capability_invocations = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_capability_invocations(
            &state.store,
            run_id,
        )
        .await?,
    )?;

    Ok(RunArchiveV1EntryResponse {
        source_run_id: run_id.to_string(),
        content_digest: String::new(),
        flow_run,
        flow_run_fact: serde_json::to_value(&detail.flow_run)?,
        compiled_plan,
        node_runs,
        checkpoints,
        callback_tasks,
        events,
        runtime_spans,
        runtime_events,
        runtime_items,
        context_projections,
        usage_ledger,
        model_failover_attempts,
        capability_invocations,
        trace_tree,
        export_warnings,
    })
}

async fn create_run_archive_import_job(
    state: &Arc<ApiState>,
    input: CreateRunArchiveImportJobInput<'_>,
) -> Result<Uuid, ApiError> {
    let job_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into run_archive_import_jobs (
            id,
            scope_id,
            application_id,
            actor_user_id,
            upload_session_id,
            status,
            archive_version,
            archive_sha256,
            run_count,
            created_by,
            updated_by
        ) values ($1, $2, $3, $4, $5, 'queued', $6, $7, $8, $4, $4)
        "#,
    )
    .bind(job_id)
    .bind(input.workspace_id)
    .bind(input.application_id)
    .bind(input.actor_user_id)
    .bind(input.session_id)
    .bind(input.archive_version)
    .bind(input.archive_sha256)
    .bind(input.run_count)
    .execute(state.store.pool())
    .await?;
    Ok(job_id)
}

async fn mark_run_archive_import_job_processing(
    state: &Arc<ApiState>,
    job_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        update run_archive_import_jobs
        set status = 'processing',
            started_at = coalesce(started_at, now()),
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .execute(state.store.pool())
    .await?;
    Ok(())
}

async fn mark_run_archive_import_job_succeeded(
    state: &Arc<ApiState>,
    job_id: Uuid,
    run_mappings: Vec<(String, Uuid)>,
) -> Result<(), ApiError> {
    let result_payload = serde_json::json!({
        "source_to_target_run_ids": run_mappings
            .iter()
            .map(|(source_run_id, target_run_id)| serde_json::json!({
                "source_run_id": source_run_id,
                "target_run_id": target_run_id.to_string()
            }))
            .collect::<Vec<_>>()
    });
    sqlx::query(
        r#"
        update run_archive_import_jobs
        set status = 'succeeded',
            imported_run_count = $2,
            result_payload = $3,
            finished_at = now(),
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .bind(i32::try_from(run_mappings.len()).unwrap_or(i32::MAX))
    .bind(result_payload)
    .execute(state.store.pool())
    .await?;
    Ok(())
}

async fn mark_run_archive_import_job_failed(
    state: &Arc<ApiState>,
    job_id: Uuid,
    message: String,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        update run_archive_import_jobs
        set status = 'failed',
            error_payload = $2,
            finished_at = now(),
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .bind(serde_json::json!({ "message": message }))
    .execute(state.store.pool())
    .await?;
    Ok(())
}

async fn mark_upload_session_completed(
    state: &Arc<ApiState>,
    session_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        update run_archive_upload_sessions
        set status = 'completed',
            completed_at = now(),
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(session_id)
    .execute(state.store.pool())
    .await?;
    Ok(())
}

async fn refresh_run_archive_upload_session_received_bytes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
) -> Result<i64, ApiError> {
    let received_bytes = sqlx::query_scalar::<_, i64>(
        r#"
        select coalesce(sum(chunk_size_bytes), 0)::bigint
        from run_archive_upload_chunks
        where session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        update run_archive_upload_sessions
        set received_bytes = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(session_id)
    .bind(received_bytes)
    .execute(&mut **tx)
    .await?;
    Ok(received_bytes)
}

async fn load_run_archive_upload_session(
    state: &Arc<ApiState>,
    application_id: Uuid,
    session_id: Uuid,
) -> Result<RunArchiveUploadSessionRow, ApiError> {
    let row = sqlx::query(
        r#"
        select
            id,
            scope_id,
            application_id,
            status,
            original_filename,
            total_size_bytes,
            received_bytes,
            expected_sha256,
            chunk_size_bytes,
            created_at,
            updated_at
        from run_archive_upload_sessions
        where id = $1
          and application_id = $2
        "#,
    )
    .bind(session_id)
    .bind(application_id)
    .fetch_optional(state.store.pool())
    .await?
    .ok_or(ControlPlaneError::NotFound("run_archive_upload_session"))?;

    Ok(RunArchiveUploadSessionRow {
        session_id: row.get("id"),
        scope_id: row.get("scope_id"),
        application_id: row.get("application_id"),
        status: row.get("status"),
        filename: row.get("original_filename"),
        total_size_bytes: row.get("total_size_bytes"),
        received_bytes: row.get("received_bytes"),
        expected_sha256: row.get("expected_sha256"),
        chunk_size_bytes: row.get("chunk_size_bytes"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn cleanup_run_archive_upload_chunks(
    state: &Arc<ApiState>,
    session_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        delete from run_archive_upload_chunks
        where session_id = $1
        "#,
    )
    .bind(session_id)
    .execute(state.store.pool())
    .await?;
    Ok(())
}

async fn load_upload_session_archive_bytes(
    state: &Arc<ApiState>,
    session_id: Uuid,
) -> Result<Vec<u8>, ApiError> {
    let chunks = sqlx::query(
        r#"
        select chunk_index, content
        from run_archive_upload_chunks
        where session_id = $1
        order by chunk_index asc
        "#,
    )
    .bind(session_id)
    .fetch_all(state.store.pool())
    .await?;
    if chunks.is_empty() {
        return Err(ControlPlaneError::InvalidInput("archive_chunks").into());
    }
    let mut bytes = Vec::new();
    for (expected_index, chunk) in chunks.into_iter().enumerate() {
        let chunk_index: i32 = chunk.get("chunk_index");
        if chunk_index != i32::try_from(expected_index).unwrap_or(i32::MAX) {
            return Err(ControlPlaneError::InvalidInput("archive_chunks").into());
        }
        let content: Vec<u8> = chunk.get("content");
        bytes.extend(content);
    }
    Ok(bytes)
}

async fn load_run_archive_import_job(
    state: &Arc<ApiState>,
    application_id: Uuid,
    job_id: Uuid,
) -> Result<RunArchiveImportJobRow, ApiError> {
    let row = sqlx::query(
        r#"
        select
            id,
            application_id,
            upload_session_id,
            status,
            archive_version,
            archive_sha256,
            run_count,
            imported_run_count,
            error_payload,
            result_payload,
            created_at,
            updated_at,
            started_at,
            finished_at
        from run_archive_import_jobs
        where id = $1
          and application_id = $2
        "#,
    )
    .bind(job_id)
    .bind(application_id)
    .fetch_optional(state.store.pool())
    .await?
    .ok_or(ControlPlaneError::NotFound("run_archive_import_job"))?;

    Ok(RunArchiveImportJobRow {
        job_id: row.get("id"),
        application_id: row.get("application_id"),
        upload_session_id: row.get("upload_session_id"),
        status: row.get("status"),
        archive_version: row.get("archive_version"),
        archive_sha256: row.get("archive_sha256"),
        run_count: row.get("run_count"),
        imported_run_count: row.get("imported_run_count"),
        error_payload: row.get("error_payload"),
        result_payload: row.get("result_payload"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}

async fn to_import_job_response(
    state: &Arc<ApiState>,
    row: RunArchiveImportJobRow,
) -> Result<RunArchiveImportJobResponse, ApiError> {
    let mapping_rows = sqlx::query(
        r#"
        select source_id, target_id
        from run_archive_import_mappings
        where job_id = $1
          and entity_kind = 'flow_run'
        order by created_at asc, source_id asc
        "#,
    )
    .bind(row.job_id)
    .fetch_all(state.store.pool())
    .await?;
    let source_to_target_run_ids = mapping_rows
        .into_iter()
        .map(|row| RunArchiveImportRunMappingResponse {
            source_run_id: row.get::<String, _>("source_id"),
            target_run_id: row.get::<Uuid, _>("target_id").to_string(),
        })
        .collect();

    Ok(RunArchiveImportJobResponse {
        job_id: row.job_id.to_string(),
        application_id: row.application_id.to_string(),
        upload_session_id: row.upload_session_id.to_string(),
        status: row.status,
        archive_version: row.archive_version,
        archive_sha256: row.archive_sha256,
        run_count: row.run_count,
        imported_run_count: row.imported_run_count,
        source_to_target_run_ids,
        error_payload: row.error_payload,
        result_payload: row.result_payload,
        created_at: application_logs::format_time(row.created_at),
        updated_at: application_logs::format_time(row.updated_at),
        started_at: application_logs::format_optional_time(row.started_at),
        finished_at: application_logs::format_optional_time(row.finished_at),
    })
}

fn to_upload_session_response(row: RunArchiveUploadSessionRow) -> RunArchiveUploadSessionResponse {
    RunArchiveUploadSessionResponse {
        session_id: row.session_id.to_string(),
        application_id: row.application_id.to_string(),
        status: row.status,
        filename: row.filename,
        total_size_bytes: row.total_size_bytes,
        received_bytes: row.received_bytes,
        expected_sha256: row.expected_sha256,
        created_at: application_logs::format_time(row.created_at),
        updated_at: application_logs::format_time(row.updated_at),
    }
}

fn extract_archive_from_zip(bytes: &[u8]) -> Result<RunArchiveV1Response, ApiError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|_| ControlPlaneError::InvalidInput("archive_not_valid_zip"))?;

    if let Some(content) = read_zip_file(&mut zip, "archive.json")? {
        return parse_run_archive_json(&content);
    }
    if let Some(archive) = extract_archive_from_trace_export_zip(&mut zip)? {
        return Ok(archive);
    }

    let mut root_json_names = Vec::new();
    for index in 0..zip.len() {
        let file = zip
            .by_index(index)
            .map_err(|_| ControlPlaneError::InvalidInput("archive_zip_read_error"))?;
        let file_name = file.name().to_string();
        if file_name.ends_with(".json") && !file_name.contains('/') {
            root_json_names.push(file_name);
        }
    }

    root_json_names.sort_by_key(|name| name == "manifest.json");
    for file_name in root_json_names {
        let Some(content) = read_zip_file(&mut zip, &file_name)? else {
            continue;
        };
        if let Ok(archive) = parse_run_archive_json(&content) {
            return Ok(archive);
        }
    }

    Err(ControlPlaneError::InvalidInput("archive_json_not_found_in_zip").into())
}

fn read_zip_file(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    use std::io::Read;

    let mut file = match zip.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(_) => return Err(ControlPlaneError::InvalidInput("archive_zip_read_error").into()),
    };
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|_| ControlPlaneError::InvalidInput("archive_zip_read_error"))?;
    Ok(Some(content))
}

fn parse_run_archive_json(content: &[u8]) -> Result<RunArchiveV1Response, ApiError> {
    serde_json::from_slice(content)
        .map_err(|_| ControlPlaneError::InvalidInput("archive_json_invalid").into())
}

fn extract_archive_from_trace_export_zip(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<Option<RunArchiveV1Response>, ApiError> {
    let Some(manifest_content) = read_zip_file(zip, "manifest.json")? else {
        return Ok(None);
    };
    let manifest: ApplicationRunSelectedExportManifestResponse =
        match serde_json::from_slice(&manifest_content) {
            Ok(manifest) => manifest,
            Err(_) => return Ok(None),
        };
    if manifest.export_version != APPLICATION_RUN_TRACE_EXPORT_VERSION
        || manifest.run_count == 0
        || manifest.entries.len() != manifest.run_count
    {
        return Err(ControlPlaneError::InvalidInput("archive_trace_manifest").into());
    }

    let mut documents = Vec::with_capacity(manifest.entries.len());
    for entry in &manifest.entries {
        let content = read_zip_file(zip, &entry.filename)?
            .ok_or(ControlPlaneError::InvalidInput("archive_trace_entry_missing"))?;
        let document: ApplicationRunTraceExportResponse = serde_json::from_slice(&content)
            .map_err(|_| ControlPlaneError::InvalidInput("archive_trace_entry_json"))?;
        if document.export_version != APPLICATION_RUN_TRACE_EXPORT_VERSION
            || document.flow_run.id != entry.run_id
        {
            return Err(ControlPlaneError::InvalidInput("archive_trace_entry").into());
        }
        documents.push(document);
    }

    Ok(Some(build_archive_from_trace_exports(manifest, documents)?))
}

fn build_archive_from_trace_exports(
    manifest: ApplicationRunSelectedExportManifestResponse,
    documents: Vec<ApplicationRunTraceExportResponse>,
) -> Result<RunArchiveV1Response, ApiError> {
    let first_document = documents
        .first()
        .ok_or(ControlPlaneError::InvalidInput("archive_trace_entries"))?;
    let exported_by_user_id = if first_document.run.principal.kind == "user" {
        first_document.run.principal.id.clone().unwrap_or_default()
    } else {
        String::new()
    };
    let source = RunArchiveV1SourceResponse {
        source_kind: "application_run_trace_export_zip".to_string(),
        workspace_id: "unknown".to_string(),
        application_id: manifest.application_id.clone(),
        application_type: first_document.run.application_type.clone(),
        application_name: "imported application runs".to_string(),
        exported_by_user_id,
        exported_at: manifest.exported_at.clone(),
        archive_builder: "api-server.application-runtime.trace-export-zip-import-v1".to_string(),
    };
    let mut entries = documents
        .into_iter()
        .map(trace_export_to_archive_entry)
        .collect::<Result<Vec<_>, ApiError>>()?;
    let manifest_entries = finalize_run_archive_v1_entries(&mut entries)?;
    let content_sha256 = run_archive_v1_entries_content_sha256(&entries)?;
    let archive_manifest = RunArchiveV1ManifestResponse {
        archive_version: RUN_ARCHIVE_VERSION,
        archive_semantics: APPLICATION_RUN_ARCHIVE_SEMANTICS.to_string(),
        exported_at: manifest.exported_at.clone(),
        source_workspace_id: "unknown".to_string(),
        source_application_id: manifest.application_id,
        run_count: entries.len(),
        selected_run_ids: manifest.selected_run_ids,
        entries: manifest_entries,
        content_sha256: content_sha256.clone(),
        checksum: content_sha256.clone(),
    };

    Ok(RunArchiveV1Response {
        archive_version: RUN_ARCHIVE_VERSION,
        exported_at: manifest.exported_at,
        manifest: archive_manifest,
        source,
        entries,
        content_digest: content_sha256,
    })
}

fn trace_export_to_archive_entry(
    document: ApplicationRunTraceExportResponse,
) -> Result<RunArchiveV1EntryResponse, ApiError> {
    let source_run_id = document.flow_run.id.clone();
    let flow_run_fact = trace_export_flow_run_fact(&document.flow_run)?;
    let mut trace_tree = serde_json::to_value(document.trace_tree)?;
    normalize_run_archive_trace_tree_projection_status(&mut trace_tree);

    Ok(RunArchiveV1EntryResponse {
        source_run_id,
        content_digest: String::new(),
        flow_run: document.flow_run,
        flow_run_fact,
        compiled_plan: None,
        node_runs: document.node_runs,
        checkpoints: document.checkpoints,
        callback_tasks: document.callback_tasks,
        events: document.events,
        runtime_spans: Vec::new(),
        runtime_events: Vec::new(),
        runtime_items: Vec::new(),
        context_projections: Vec::new(),
        usage_ledger: Vec::new(),
        model_failover_attempts: Vec::new(),
        capability_invocations: Vec::new(),
        trace_tree,
        export_warnings: document.export_warnings,
    })
}

fn normalize_run_archive_trace_tree_projection_status(trace_tree: &mut serde_json::Value) {
    let Some(projection_status) = trace_tree
        .get_mut("projection_status")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };

    projection_status.insert("last_attempt_at".to_string(), serde_json::Value::Null);
    projection_status.insert("last_success_at".to_string(), serde_json::Value::Null);
}

fn finalize_run_archive_v1_entries(
    entries: &mut [RunArchiveV1EntryResponse],
) -> Result<Vec<RunArchiveV1ManifestEntryResponse>, ApiError> {
    for entry in entries.iter_mut() {
        entry.content_digest = run_archive_v1_entry_content_sha256(entry)?;
    }

    Ok(entries
        .iter()
        .map(|entry| RunArchiveV1ManifestEntryResponse {
            source_run_id: entry.source_run_id.clone(),
            content_sha256: entry.content_digest.clone(),
            content_digest: entry.content_digest.clone(),
        })
        .collect())
}

fn run_archive_v1_entries_content_sha256(
    entries: &[RunArchiveV1EntryResponse],
) -> Result<String, ApiError> {
    let payload = entries
        .iter()
        .map(run_archive_v1_entry_digest_payload)
        .collect::<Vec<_>>();
    Ok(sha256_bytes(&serde_json::to_vec(&payload)?))
}

fn run_archive_v1_entry_content_sha256(
    entry: &RunArchiveV1EntryResponse,
) -> Result<String, ApiError> {
    Ok(sha256_bytes(&serde_json::to_vec(
        &run_archive_v1_entry_digest_payload(entry),
    )?))
}

fn run_archive_v1_entry_digest_payload(entry: &RunArchiveV1EntryResponse) -> serde_json::Value {
    let mut value = serde_json::json!({
        "source_run_id": &entry.source_run_id,
        "flow_run": &entry.flow_run,
        "flow_run_fact": &entry.flow_run_fact,
        "compiled_plan": &entry.compiled_plan,
        "node_runs": &entry.node_runs,
        "checkpoints": &entry.checkpoints,
        "callback_tasks": &entry.callback_tasks,
        "events": &entry.events,
        "runtime_spans": &entry.runtime_spans,
        "runtime_events": &entry.runtime_events,
        "runtime_items": &entry.runtime_items,
        "context_projections": &entry.context_projections,
        "usage_ledger": &entry.usage_ledger,
        "model_failover_attempts": &entry.model_failover_attempts,
        "capability_invocations": &entry.capability_invocations,
    });
    remove_run_archive_digest_volatile_fields(&mut value);
    value
}

fn remove_run_archive_digest_volatile_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                remove_run_archive_digest_volatile_fields(item);
            }
        }
        serde_json::Value::Object(object) => {
            object.remove("updated_at");
            for item in object.values_mut() {
                remove_run_archive_digest_volatile_fields(item);
            }
        }
        _ => {}
    }
}

fn trace_export_flow_run_fact(flow_run: &FlowRunResponse) -> Result<serde_json::Value, ApiError> {
    let mut value = serde_json::to_value(flow_run)?;
    let object = value
        .as_object_mut()
        .ok_or(ControlPlaneError::InvalidInput("archive_trace_flow_run"))?;
    if let Some(external_user) = flow_run.expand_id.as_ref() {
        object.insert(
            "external_user".to_string(),
            serde_json::Value::String(external_user.clone()),
        );
    }
    object.insert(
        "document_hash".to_string(),
        serde_json::Value::String("imported-trace-export".to_string()),
    );
    Ok(value)
}

fn parse_run_archive_v1(bytes: &[u8]) -> Result<RunArchiveV1Response, ApiError> {
    // Try to parse as JSON first (single file archive)
    let archive: RunArchiveV1Response = match serde_json::from_slice(bytes) {
        Ok(archive) => archive,
        Err(_) => {
            // If JSON parsing fails, try to extract from ZIP
            extract_archive_from_zip(bytes)?
        }
    };
    if archive.archive_version != RUN_ARCHIVE_VERSION
        || archive.manifest.archive_version != RUN_ARCHIVE_VERSION
        || archive.manifest.archive_semantics != APPLICATION_RUN_ARCHIVE_SEMANTICS
    {
        return Err(ControlPlaneError::InvalidInput("archive_version").into());
    }
    let content_sha256 = run_archive_v1_entries_content_sha256(&archive.entries)?;
    if normalize_sha256(&content_sha256) != normalize_sha256(&archive.manifest.content_sha256) {
        return Err(ControlPlaneError::InvalidInput("archive_content_sha256").into());
    }
    if normalize_sha256(&archive.manifest.checksum)
        != normalize_sha256(&archive.manifest.content_sha256)
        || normalize_sha256(&archive.content_digest)
            != normalize_sha256(&archive.manifest.content_sha256)
    {
        return Err(ControlPlaneError::InvalidInput("archive_checksum").into());
    }
    if archive.entries.is_empty()
        || archive.entries.len() != archive.manifest.run_count
        || archive.entries.len() != archive.manifest.entries.len()
    {
        return Err(ControlPlaneError::InvalidInput("archive_entries").into());
    }
    for (entry, manifest_entry) in archive.entries.iter().zip(&archive.manifest.entries) {
        if entry.source_run_id != manifest_entry.source_run_id {
            return Err(ControlPlaneError::InvalidInput("archive_entries").into());
        }
        let entry_content_digest = run_archive_v1_entry_content_sha256(entry)?;
        if normalize_sha256(&entry_content_digest) != normalize_sha256(&entry.content_digest) {
            return Err(ControlPlaneError::InvalidInput("archive_entry_digest").into());
        }
        if normalize_sha256(&entry_content_digest)
            != normalize_sha256(&manifest_entry.content_sha256)
        {
            return Err(ControlPlaneError::InvalidInput("archive_entry_sha256").into());
        }
        if normalize_sha256(&entry.content_digest)
            != normalize_sha256(&manifest_entry.content_digest)
        {
            return Err(ControlPlaneError::InvalidInput("archive_entry_digest").into());
        }
    }

    Ok(archive)
}

fn records_to_json_values<T: Serialize>(
    records: Vec<T>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    records
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::from)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn normalize_sha256(value: &str) -> String {
    value
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase()
}

fn ensure_sha256_value(value: &str, field: &'static str) -> Result<(), ApiError> {
    let normalized = normalize_sha256(value);
    if normalized.len() == 64 && normalized.chars().all(|value| value.is_ascii_hexdigit()) {
        return Ok(());
    }

    Err(ControlPlaneError::InvalidInput(field).into())
}

fn expected_archive_chunk_count(
    total_size_bytes: i64,
    chunk_size_bytes: i64,
) -> Result<i64, ApiError> {
    if total_size_bytes <= 0 || chunk_size_bytes <= 0 {
        return Err(ControlPlaneError::InvalidInput("archive_chunk_count").into());
    }

    Ok((total_size_bytes + chunk_size_bytes - 1) / chunk_size_bytes)
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn application_run_archive_filename(
    application_name: &str,
    exported_at: &str,
    run_count: usize,
) -> String {
    let timestamp = exported_at
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!(
        "1flowbase-run-archive-{}-{}-{}runs.json",
        safe_filename_segment(application_name),
        timestamp,
        run_count
    )
}
