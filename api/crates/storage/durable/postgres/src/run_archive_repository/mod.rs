mod restore_context_records;
mod restore_records;
mod restore_transaction;

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{
    CreateRunArchiveImportJobRecordInput, CreateRunArchiveUploadSessionInput,
    PersistRunArchiveUploadChunkInput, RestoreRunArchiveBatchInput, RestoreRunArchiveBatchResult,
    RunArchiveExportContextRows, RunArchiveImportJobRecord, RunArchiveRepository,
    RunArchiveUploadSessionRecord,
};
use control_plane_contracts::ControlPlaneContractError as ControlPlaneError;
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::PgControlPlaneStore;

#[derive(Default)]
pub(super) struct ArchiveRestoreIdMaps {
    pub(super) node_runs: HashMap<Uuid, Uuid>,
    pub(super) runtime_spans: HashMap<Uuid, Uuid>,
    pub(super) runtime_events: HashMap<Uuid, Uuid>,
    pub(super) runtime_items: HashMap<Uuid, Uuid>,
    pub(super) usage_ledger: HashMap<Uuid, Uuid>,
    pub(super) model_failover_attempts: HashMap<Uuid, Uuid>,
    pub(super) context_projections: HashMap<Uuid, Uuid>,
    pub(super) canonical_contents: HashMap<Uuid, Uuid>,
    pub(super) checkpoints: HashMap<Uuid, Uuid>,
    pub(super) callback_tasks: HashMap<Uuid, Uuid>,
}

async fn archive_json_rows(pool: &PgPool, query: &'static str, run_id: Uuid) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar::<_, Value>(query)
        .bind(run_id)
        .fetch_all(pool)
        .await?)
}

async fn refresh_upload_session_received_bytes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
) -> Result<i64> {
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

#[async_trait]
impl RunArchiveRepository for PgControlPlaneStore {
    async fn list_node_run_error_payloads(&self, flow_run_id: Uuid) -> Result<Vec<(Uuid, Value)>> {
        let rows = sqlx::query(
            r#"
            select id, error_payload
            from node_run_records
            where flow_run_id = $1
              and error_payload is not null
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<Uuid, _>("id"),
                    row.get::<Value, _>("error_payload"),
                )
            })
            .collect())
    }

    async fn load_run_archive_export_context_rows(
        &self,
        flow_run_id: Uuid,
    ) -> Result<RunArchiveExportContextRows> {
        let pool = self.pool();
        let context_projections = archive_json_rows(
            pool,
            r#"
            select to_jsonb(projections) as value
              from runtime_context_projections projections
             where flow_run_id = $1 order by created_at, id
            "#,
            flow_run_id,
        )
        .await?;
        let canonical_contents = archive_json_rows(
            pool,
            r#"
            select to_jsonb(contents) as value
              from runtime_canonical_contents contents
             where exists (
                 select 1 from runtime_context_projections projections
                  where projections.flow_run_id = $1
                    and projections.actual_content_id = contents.id
             ) or exists (
                 select 1 from flow_run_recovery_history recovery
                  where recovery.flow_run_id = $1
                    and recovery.recovery_content_id = contents.id
             )
             order by created_at, id
            "#,
            flow_run_id,
        )
        .await?;
        let invocation_context_bindings = archive_json_rows(
            pool,
            "select to_jsonb(bindings) as value from runtime_invocation_context_bindings bindings where flow_run_id = $1 order by created_at, invocation_span_id",
            flow_run_id,
        )
        .await?;
        let recovery_history = archive_json_rows(
            pool,
            "select to_jsonb(recovery) as value from flow_run_recovery_history recovery where flow_run_id = $1 order by sequence, id",
            flow_run_id,
        )
        .await?;
        let resume_claims = archive_json_rows(
            pool,
            "select to_jsonb(claims) as value from flow_run_resume_claims claims where flow_run_id = $1 order by created_at, id",
            flow_run_id,
        )
        .await?;

        Ok(RunArchiveExportContextRows {
            context_projections,
            canonical_contents,
            invocation_context_bindings,
            recovery_history,
            resume_claims,
        })
    }

    async fn create_run_archive_upload_session(
        &self,
        input: &CreateRunArchiveUploadSessionInput,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into run_archive_upload_sessions (
                id, scope_id, application_id, actor_user_id, original_filename,
                total_size_bytes, expected_sha256, chunk_size_bytes, status, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, 'uploading', $9, $9)
            "#,
        )
        .bind(input.session_id)
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.actor_user_id)
        .bind(input.filename.as_deref())
        .bind(input.total_size_bytes)
        .bind(&input.expected_sha256)
        .bind(input.chunk_size_bytes)
        .bind(input.actor_user_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_run_archive_upload_session(
        &self,
        application_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<RunArchiveUploadSessionRecord>> {
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
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| RunArchiveUploadSessionRecord {
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
        }))
    }

    async fn persist_run_archive_upload_chunk(
        &self,
        input: &PersistRunArchiveUploadChunkInput,
    ) -> Result<i64> {
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"
            insert into run_archive_upload_chunks (
                id, scope_id, session_id, chunk_index, chunk_size_bytes,
                chunk_sha256, content, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
            on conflict (session_id, chunk_index) do update
            set chunk_size_bytes = excluded.chunk_size_bytes,
                chunk_sha256 = excluded.chunk_sha256,
                content = excluded.content,
                updated_at = now(),
                updated_by = excluded.updated_by
            "#,
        )
        .bind(input.chunk_id)
        .bind(input.scope_id)
        .bind(input.session_id)
        .bind(input.chunk_index)
        .bind(i64::try_from(input.content.len()).unwrap_or(i64::MAX))
        .bind(&input.chunk_sha256)
        .bind(&input.content)
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;
        let received_bytes =
            refresh_upload_session_received_bytes(&mut tx, input.session_id).await?;
        if received_bytes > input.total_size_bytes {
            return Err(ControlPlaneError::InvalidInput("archive_size").into());
        }
        tx.commit().await?;
        Ok(received_bytes)
    }

    async fn load_run_archive_upload_session_bytes(&self, session_id: Uuid) -> Result<Vec<u8>> {
        let chunks = sqlx::query(
            r#"
            select chunk_index, content
            from run_archive_upload_chunks
            where session_id = $1
            order by chunk_index asc
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
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

    async fn mark_run_archive_upload_session_completed(&self, session_id: Uuid) -> Result<()> {
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn delete_run_archive_upload_chunks(&self, session_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            delete from run_archive_upload_chunks
            where session_id = $1
            "#,
        )
        .bind(session_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn create_run_archive_import_job(
        &self,
        input: &CreateRunArchiveImportJobRecordInput,
    ) -> Result<()> {
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
        .bind(input.job_id)
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.actor_user_id)
        .bind(input.session_id)
        .bind(input.archive_version)
        .bind(&input.archive_sha256)
        .bind(input.run_count)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_run_archive_import_job(
        &self,
        application_id: Uuid,
        job_id: Uuid,
    ) -> Result<Option<RunArchiveImportJobRecord>> {
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
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| RunArchiveImportJobRecord {
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
        }))
    }

    async fn list_run_archive_import_flow_run_mappings(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<(String, Uuid)>> {
        let rows = sqlx::query(
            r#"
            select source_id, target_id
            from run_archive_import_mappings
            where job_id = $1
              and entity_kind = 'flow_run'
            order by created_at asc, source_id asc
            "#,
        )
        .bind(job_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("source_id"),
                    row.get::<Uuid, _>("target_id"),
                )
            })
            .collect())
    }

    async fn mark_run_archive_import_job_processing(&self, job_id: Uuid) -> Result<()> {
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn mark_run_archive_import_job_succeeded(
        &self,
        job_id: Uuid,
        run_mappings: &[(String, Uuid)],
    ) -> Result<()> {
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn mark_run_archive_import_job_failed(&self, job_id: Uuid, message: &str) -> Result<()> {
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn update_run_archive_import_job_projection_warnings(
        &self,
        job_id: Uuid,
        run_mappings: &[(String, Uuid)],
        warnings: Vec<Value>,
    ) -> Result<()> {
        let result_payload = serde_json::json!({
            "source_to_target_run_ids": run_mappings
                .iter()
                .map(|(source_run_id, target_run_id)| serde_json::json!({
                    "source_run_id": source_run_id,
                    "target_run_id": target_run_id.to_string()
                }))
                .collect::<Vec<_>>(),
            "warnings": warnings
        });
        sqlx::query(
            r#"
            update run_archive_import_jobs
            set result_payload = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(job_id)
        .bind(result_payload)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn restore_run_archive_entries(
        &self,
        input: &RestoreRunArchiveBatchInput,
    ) -> Result<RestoreRunArchiveBatchResult> {
        restore_transaction::restore_run_archive_entries(self.pool(), input).await
    }
}
