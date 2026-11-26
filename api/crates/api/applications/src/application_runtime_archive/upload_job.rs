use control_plane_contracts::ports::{
    CreateRunArchiveImportJobRecordInput, CreateRunArchiveUploadSessionInput,
    PersistRunArchiveUploadChunkInput, RunArchiveImportJobRecord, RunArchiveRepository,
    RunArchiveUploadSessionRecord,
};

use super::*;

pub(super) const RUN_ARCHIVE_UPLOAD_MAX_BYTES: i64 = 100 * 1024 * 1024;
pub(super) const RUN_ARCHIVE_UPLOAD_MAX_CHUNK_BYTES: i64 = 8 * 1024 * 1024;
pub(super) const RUN_ARCHIVE_UPLOAD_MAX_CHUNKS: i64 = 4096;

pub(super) type RunArchiveUploadSessionRow = RunArchiveUploadSessionRecord;
pub(super) type RunArchiveImportJobRow = RunArchiveImportJobRecord;

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
    store: &storage_durable_postgres::MainDurableStore,
    input: CreateRunArchiveImportJobInput<'_>,
) -> Result<Uuid, ApiError> {
    let job_id = Uuid::now_v7();
    <_ as RunArchiveRepository>::create_run_archive_import_job(
        store,
        &CreateRunArchiveImportJobRecordInput {
            job_id,
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            actor_user_id: input.actor_user_id,
            session_id: input.session_id,
            archive_version: input.archive_version,
            archive_sha256: input.archive_sha256.to_string(),
            run_count: input.run_count,
        },
    )
    .await?;
    Ok(job_id)
}

pub(super) async fn mark_run_archive_import_job_processing(
    store: &storage_durable_postgres::MainDurableStore,
    job_id: Uuid,
) -> Result<(), ApiError> {
    <_ as RunArchiveRepository>::mark_run_archive_import_job_processing(store, job_id).await?;
    Ok(())
}

pub(super) async fn mark_run_archive_import_job_succeeded(
    store: &storage_durable_postgres::MainDurableStore,
    job_id: Uuid,
    run_mappings: Vec<(String, Uuid)>,
) -> Result<(), ApiError> {
    <_ as RunArchiveRepository>::mark_run_archive_import_job_succeeded(
        store,
        job_id,
        &run_mappings,
    )
    .await?;
    Ok(())
}

pub(super) async fn mark_run_archive_import_job_failed(
    store: &storage_durable_postgres::MainDurableStore,
    job_id: Uuid,
    message: String,
) -> Result<(), ApiError> {
    <_ as RunArchiveRepository>::mark_run_archive_import_job_failed(store, job_id, &message)
        .await?;
    Ok(())
}

pub(super) async fn mark_upload_session_completed(
    store: &storage_durable_postgres::MainDurableStore,
    session_id: Uuid,
) -> Result<(), ApiError> {
    <_ as RunArchiveRepository>::mark_run_archive_upload_session_completed(store, session_id)
        .await?;
    Ok(())
}

pub(super) async fn load_run_archive_upload_session(
    store: &storage_durable_postgres::MainDurableStore,
    application_id: Uuid,
    session_id: Uuid,
) -> Result<RunArchiveUploadSessionRow, ApiError> {
    <_ as RunArchiveRepository>::get_run_archive_upload_session(store, application_id, session_id)
        .await?
        .ok_or_else(|| ControlPlaneError::NotFound("run_archive_upload_session").into())
}

pub(super) async fn cleanup_run_archive_upload_chunks(
    store: &storage_durable_postgres::MainDurableStore,
    session_id: Uuid,
) -> Result<(), ApiError> {
    <_ as RunArchiveRepository>::delete_run_archive_upload_chunks(store, session_id).await?;
    Ok(())
}

pub(super) async fn load_upload_session_archive_bytes(
    store: &storage_durable_postgres::MainDurableStore,
    session_id: Uuid,
) -> Result<Vec<u8>, ApiError> {
    Ok(
        <_ as RunArchiveRepository>::load_run_archive_upload_session_bytes(store, session_id)
            .await?,
    )
}

pub(super) async fn load_run_archive_import_job(
    store: &storage_durable_postgres::MainDurableStore,
    application_id: Uuid,
    job_id: Uuid,
) -> Result<RunArchiveImportJobRow, ApiError> {
    <_ as RunArchiveRepository>::get_run_archive_import_job(store, application_id, job_id)
        .await?
        .ok_or_else(|| ControlPlaneError::NotFound("run_archive_import_job").into())
}

pub(super) async fn to_import_job_response(
    store: &storage_durable_postgres::MainDurableStore,
    row: RunArchiveImportJobRow,
) -> Result<RunArchiveImportJobResponse, ApiError> {
    let mapping_rows =
        <_ as RunArchiveRepository>::list_run_archive_import_flow_run_mappings(store, row.job_id)
            .await?;
    let source_to_target_run_ids = mapping_rows
        .into_iter()
        .map(
            |(source_run_id, target_run_id)| RunArchiveImportRunMappingResponse {
                source_run_id,
                target_run_id: target_run_id.to_string(),
            },
        )
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

pub(super) struct PersistRunArchiveUploadSessionInput<'a> {
    pub session_id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
    pub filename: Option<&'a str>,
    pub total_size_bytes: i64,
    pub expected_sha256: &'a str,
    pub chunk_size_bytes: i64,
}

pub(super) async fn persist_run_archive_upload_session(
    store: &storage_durable_postgres::MainDurableStore,
    input: PersistRunArchiveUploadSessionInput<'_>,
) -> Result<(), ApiError> {
    <_ as RunArchiveRepository>::create_run_archive_upload_session(
        store,
        &CreateRunArchiveUploadSessionInput {
            session_id: input.session_id,
            scope_id: input.scope_id,
            application_id: input.application_id,
            actor_user_id: input.actor_user_id,
            filename: input.filename.map(ToString::to_string),
            total_size_bytes: input.total_size_bytes,
            expected_sha256: input.expected_sha256.to_string(),
            chunk_size_bytes: input.chunk_size_bytes,
        },
    )
    .await?;
    Ok(())
}

pub(super) struct PersistRunArchiveChunkInput<'a> {
    pub chunk_id: Uuid,
    pub scope_id: Uuid,
    pub session_id: Uuid,
    pub chunk_index: i32,
    pub content: &'a [u8],
    pub chunk_sha256: &'a str,
    pub actor_user_id: Uuid,
    pub total_size_bytes: i64,
}

pub(super) async fn persist_run_archive_chunk(
    store: &storage_durable_postgres::MainDurableStore,
    input: PersistRunArchiveChunkInput<'_>,
) -> Result<i64, ApiError> {
    Ok(
        <_ as RunArchiveRepository>::persist_run_archive_upload_chunk(
            store,
            &PersistRunArchiveUploadChunkInput {
                chunk_id: input.chunk_id,
                scope_id: input.scope_id,
                session_id: input.session_id,
                chunk_index: input.chunk_index,
                content: input.content.to_vec(),
                chunk_sha256: input.chunk_sha256.to_string(),
                actor_user_id: input.actor_user_id,
                total_size_bytes: input.total_size_bytes,
            },
        )
        .await?,
    )
}
