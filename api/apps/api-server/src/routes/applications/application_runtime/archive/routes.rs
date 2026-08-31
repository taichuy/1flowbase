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
pub(crate) async fn export_application_run_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ApplicationRunArchiveQuery>,
) -> Result<axum::response::Response, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.archive.run.export.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        super::interface::ApplicationRuntimeArchiveInput::ExportOne {
            application_id: id,
            run_id,
            archive_version: query.archive_version,
        },
    )
    .await?;
    let super::interface::ApplicationRuntimeArchiveOutput::Download(download) = output else {
        unreachable!("runtime archive export binding returned a different output")
    };
    download_response("application/json", &download.filename, download.body)
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
pub(crate) async fn export_application_runs_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ApplicationRunArchiveBody>,
) -> Result<axum::response::Response, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.archive.runs.export.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::interface::ApplicationRuntimeArchiveInput::ExportMany {
            application_id: id,
            body,
        },
    )
    .await?;
    let super::interface::ApplicationRuntimeArchiveOutput::Download(download) = output else {
        unreachable!("runtime archives export binding returned a different output")
    };
    download_response("application/json", &download.filename, download.body)
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.archive.upload-sessions.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::interface::ApplicationRuntimeArchiveInput::CreateUploadSession {
            application_id: id,
            body,
        },
    )
    .await?;
    let super::interface::ApplicationRuntimeArchiveOutput::UploadSession(session) = output else {
        unreachable!("runtime archive upload-session binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(session))))
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
    let expected_sha256 = header_value(&headers, "x-chunk-sha256")
        .ok_or(ControlPlaneError::InvalidInput("chunk_sha256"))?;
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.archive.upload-chunks.upsert.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::interface::ApplicationRuntimeArchiveInput::UploadChunk {
            application_id: id,
            session_id,
            chunk_index,
            body: body.to_vec(),
            expected_sha256,
        },
    )
    .await?;
    let super::interface::ApplicationRuntimeArchiveOutput::Chunk(response) = output else {
        unreachable!("runtime archive chunk binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.archive.upload-sessions.complete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::interface::ApplicationRuntimeArchiveInput::CompleteUploadSession {
            application_id: id,
            session_id,
        },
    )
    .await?;
    let super::interface::ApplicationRuntimeArchiveOutput::ImportJob(response) = output else {
        unreachable!("runtime archive completion binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.archive.import-jobs.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        super::interface::ApplicationRuntimeArchiveInput::GetImportJob {
            application_id: id,
            job_id,
        },
    )
    .await?;
    let super::interface::ApplicationRuntimeArchiveOutput::ImportJob(response) = output else {
        unreachable!("runtime archive import-job binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}
