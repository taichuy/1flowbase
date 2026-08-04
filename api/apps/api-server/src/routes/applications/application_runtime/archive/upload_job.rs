use super::*;

pub(super) const RUN_ARCHIVE_UPLOAD_MAX_BYTES: i64 = 100 * 1024 * 1024;
pub(super) const RUN_ARCHIVE_UPLOAD_MAX_CHUNK_BYTES: i64 = 8 * 1024 * 1024;
pub(super) const RUN_ARCHIVE_UPLOAD_MAX_CHUNKS: i64 = 4096;

#[derive(Debug)]
pub(super) struct RunArchiveUploadSessionRow {
    pub(super) session_id: Uuid,
    pub(super) scope_id: Uuid,
    pub(super) application_id: Uuid,
    pub(super) status: String,
    pub(super) filename: Option<String>,
    pub(super) total_size_bytes: i64,
    pub(super) received_bytes: i64,
    pub(super) expected_sha256: Option<String>,
    pub(super) chunk_size_bytes: i64,
    pub(super) created_at: OffsetDateTime,
    pub(super) updated_at: OffsetDateTime,
}

#[derive(Debug)]
pub(super) struct RunArchiveImportJobRow {
    pub(super) job_id: Uuid,
    pub(super) application_id: Uuid,
    pub(super) upload_session_id: Uuid,
    pub(super) status: String,
    pub(super) archive_version: Option<i32>,
    pub(super) archive_sha256: Option<String>,
    pub(super) run_count: i32,
    pub(super) imported_run_count: i32,
    pub(super) error_payload: Option<serde_json::Value>,
    pub(super) result_payload: serde_json::Value,
    pub(super) created_at: OffsetDateTime,
    pub(super) updated_at: OffsetDateTime,
    pub(super) started_at: Option<OffsetDateTime>,
    pub(super) finished_at: Option<OffsetDateTime>,
}

pub(super) struct CreateRunArchiveImportJobInput<'a> {
    pub(super) workspace_id: Uuid,
    pub(super) application_id: Uuid,
    pub(super) actor_user_id: Uuid,
    pub(super) session_id: Uuid,
    pub(super) archive_version: i32,
    pub(super) archive_sha256: &'a str,
    pub(super) run_count: i32,
}
pub(super) async fn create_run_archive_import_job(
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

pub(super) async fn mark_run_archive_import_job_processing(
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

pub(super) async fn mark_run_archive_import_job_succeeded(
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

pub(super) async fn mark_run_archive_import_job_failed(
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

pub(super) async fn mark_upload_session_completed(
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

pub(super) async fn refresh_run_archive_upload_session_received_bytes(
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

pub(super) async fn load_run_archive_upload_session(
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

pub(super) async fn cleanup_run_archive_upload_chunks(
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

pub(super) async fn load_upload_session_archive_bytes(
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

pub(super) async fn load_run_archive_import_job(
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

pub(super) async fn to_import_job_response(
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

pub(super) fn to_upload_session_response(
    row: RunArchiveUploadSessionRow,
) -> RunArchiveUploadSessionResponse {
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

pub(super) fn expected_archive_chunk_count(
    total_size_bytes: i64,
    chunk_size_bytes: i64,
) -> Result<i64, ApiError> {
    if total_size_bytes <= 0 || chunk_size_bytes <= 0 {
        return Err(ControlPlaneError::InvalidInput("archive_chunk_count").into());
    }

    Ok((total_size_bytes + chunk_size_bytes - 1) / chunk_size_bytes)
}

pub(super) fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}
