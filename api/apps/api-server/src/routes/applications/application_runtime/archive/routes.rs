use super::*;

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
pub(crate) async fn export_application_run_archive(
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
pub(crate) async fn export_application_runs_archive(
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
pub(crate) async fn create_run_archive_upload_session(
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
pub(crate) async fn upload_run_archive_chunk(
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
pub(crate) async fn complete_run_archive_upload_session(
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
pub(crate) async fn get_run_archive_import_job(
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
