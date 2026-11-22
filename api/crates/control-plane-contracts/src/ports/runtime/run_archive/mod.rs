use anyhow::Result;
use async_trait::async_trait;
use domain::FlowRunStatus;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RunArchiveUploadSessionRecord {
    pub session_id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub status: String,
    pub filename: Option<String>,
    pub total_size_bytes: i64,
    pub received_bytes: i64,
    pub expected_sha256: Option<String>,
    pub chunk_size_bytes: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct CreateRunArchiveUploadSessionInput {
    pub session_id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
    pub filename: Option<String>,
    pub total_size_bytes: i64,
    pub expected_sha256: String,
    pub chunk_size_bytes: i64,
}

#[derive(Clone, Debug)]
pub struct PersistRunArchiveUploadChunkInput {
    pub chunk_id: Uuid,
    pub scope_id: Uuid,
    pub session_id: Uuid,
    pub chunk_index: i32,
    pub content: Vec<u8>,
    pub chunk_sha256: String,
    pub actor_user_id: Uuid,
    pub total_size_bytes: i64,
}

#[derive(Clone, Debug)]
pub struct RunArchiveImportJobRecord {
    pub job_id: Uuid,
    pub application_id: Uuid,
    pub upload_session_id: Uuid,
    pub status: String,
    pub archive_version: Option<i32>,
    pub archive_sha256: Option<String>,
    pub run_count: i32,
    pub imported_run_count: i32,
    pub error_payload: Option<Value>,
    pub result_payload: Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct CreateRunArchiveImportJobRecordInput {
    pub job_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
    pub session_id: Uuid,
    pub archive_version: i32,
    pub archive_sha256: String,
    pub run_count: i32,
}

#[derive(Clone, Debug, Default)]
pub struct RunArchiveExportContextRows {
    pub context_projections: Vec<Value>,
    pub canonical_contents: Vec<Value>,
    pub invocation_context_bindings: Vec<Value>,
    pub recovery_history: Vec<Value>,
    pub resume_claims: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct RestoreArchivedFlowRunInput {
    pub run_mode: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub status: String,
    pub input_payload: Value,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct RestoreArchivedNodeRunInput {
    pub id: String,
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub status: String,
    pub input_payload: Value,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RestoreArchivedRunEventInput {
    pub id: String,
    pub node_run_id: Option<String>,
    pub sequence: i64,
    pub event_type: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct RestoreArchivedCheckpointInput {
    pub id: String,
    pub node_run_id: Option<String>,
    pub status: String,
    pub reason: String,
    pub locator_payload: Value,
    pub variable_snapshot: Value,
    pub external_ref_payload: Option<Value>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct RestoreArchivedCallbackTaskInput {
    pub id: String,
    pub node_run_id: String,
    pub callback_kind: String,
    pub status: String,
    pub request_payload: Value,
    pub response_payload: Option<Value>,
    pub external_ref_payload: Option<Value>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RestoreRunArchiveEntryInput {
    pub source_run_id: String,
    pub flow_run: RestoreArchivedFlowRunInput,
    pub flow_run_fact: Value,
    pub compiled_plan: Option<Value>,
    pub node_runs: Vec<RestoreArchivedNodeRunInput>,
    pub checkpoints: Vec<RestoreArchivedCheckpointInput>,
    pub callback_tasks: Vec<RestoreArchivedCallbackTaskInput>,
    pub events: Vec<RestoreArchivedRunEventInput>,
    pub runtime_spans: Vec<Value>,
    pub runtime_events: Vec<Value>,
    pub runtime_items: Vec<Value>,
    pub context_projections: Vec<Value>,
    pub canonical_contents: Vec<Value>,
    pub invocation_context_bindings: Vec<Value>,
    pub recovery_history: Vec<Value>,
    pub resume_claims: Vec<Value>,
    pub usage_ledger: Vec<Value>,
    pub model_failover_attempts: Vec<Value>,
    pub capability_invocations: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct RestoreRunArchiveBatchInput {
    pub job_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
    pub flow_id: Uuid,
    pub flow_draft_id: Uuid,
    pub flow_draft_updated_at: OffsetDateTime,
    pub entries: Vec<RestoreRunArchiveEntryInput>,
}

#[derive(Clone, Debug)]
pub struct RestoredRunFinalUpdate {
    pub target_run_id: Uuid,
    pub status: FlowRunStatus,
    pub input_payload: Value,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct RestoreRunArchiveBatchResult {
    pub run_mappings: Vec<(String, Uuid)>,
    pub final_run_updates: Vec<RestoredRunFinalUpdate>,
}

#[async_trait]
pub trait RunArchiveRepository: Send + Sync {
    async fn list_node_run_error_payloads(&self, flow_run_id: Uuid) -> Result<Vec<(Uuid, Value)>>;

    async fn load_run_archive_export_context_rows(
        &self,
        flow_run_id: Uuid,
    ) -> Result<RunArchiveExportContextRows>;

    async fn create_run_archive_upload_session(
        &self,
        input: &CreateRunArchiveUploadSessionInput,
    ) -> Result<()>;

    async fn get_run_archive_upload_session(
        &self,
        application_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<RunArchiveUploadSessionRecord>>;

    async fn persist_run_archive_upload_chunk(
        &self,
        input: &PersistRunArchiveUploadChunkInput,
    ) -> Result<i64>;

    async fn load_run_archive_upload_session_bytes(&self, session_id: Uuid) -> Result<Vec<u8>>;

    async fn mark_run_archive_upload_session_completed(&self, session_id: Uuid) -> Result<()>;

    async fn delete_run_archive_upload_chunks(&self, session_id: Uuid) -> Result<()>;

    async fn create_run_archive_import_job(
        &self,
        input: &CreateRunArchiveImportJobRecordInput,
    ) -> Result<()>;

    async fn get_run_archive_import_job(
        &self,
        application_id: Uuid,
        job_id: Uuid,
    ) -> Result<Option<RunArchiveImportJobRecord>>;

    async fn list_run_archive_import_flow_run_mappings(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<(String, Uuid)>>;

    async fn mark_run_archive_import_job_processing(&self, job_id: Uuid) -> Result<()>;

    async fn mark_run_archive_import_job_succeeded(
        &self,
        job_id: Uuid,
        run_mappings: &[(String, Uuid)],
    ) -> Result<()>;

    async fn mark_run_archive_import_job_failed(&self, job_id: Uuid, message: &str) -> Result<()>;

    async fn update_run_archive_import_job_projection_warnings(
        &self,
        job_id: Uuid,
        run_mappings: &[(String, Uuid)],
        warnings: Vec<Value>,
    ) -> Result<()>;

    async fn restore_run_archive_entries(
        &self,
        input: &RestoreRunArchiveBatchInput,
    ) -> Result<RestoreRunArchiveBatchResult>;
}
