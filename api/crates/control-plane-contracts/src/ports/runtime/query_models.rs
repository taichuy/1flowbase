use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListApplicationRunsPageInput {
    pub page: i64,
    pub page_size: i64,
    pub created_after: Option<OffsetDateTime>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunSummaryPage {
    pub items: Vec<domain::ApplicationRunSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunLogSummaryPage {
    pub items: Vec<domain::ApplicationRunLogSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunCountTokensResult {
    pub flow_run_id: Uuid,
    pub input_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ListApplicationConversationRunsPageInput {
    pub external_conversation_id: String,
    pub around_run_id: Option<Uuid>,
    pub before_run_id: Option<Uuid>,
    pub after_run_id: Option<Uuid>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationConversationRunsPage {
    pub items: Vec<domain::ApplicationConversationRunSummary>,
    pub has_before: bool,
    pub has_after: bool,
    pub before_cursor: Option<Uuid>,
    pub after_cursor: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Page units are business turns (task anchors), never provider message items.
pub struct ListApplicationRunConversationMessageItemsPageInput {
    pub before_sequence: Option<i64>,
    pub after_sequence: Option<i64>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunConversationMessageItemsPage {
    pub items: Vec<domain::ApplicationRunConversationMessageItem>,
    /// System/developer context retained by the member runs of this task.
    /// Context travels with its turn and does not consume the turn limit.
    pub contexts: Vec<domain::ApplicationRunConversationContextItem>,
    pub output_state: Option<domain::ApplicationRunConversationOutputState>,
    pub total_count: i64,
    pub has_before: bool,
    pub has_after: bool,
    pub before_cursor: Option<i64>,
    pub after_cursor: Option<i64>,
    /// Newest sequence in this page. Passing it back as `after_sequence` reads
    /// only what was appended since, which lets a client catch up on more items
    /// than one page holds without knowing the cursor format.
    pub newest_sequence: Option<i64>,
}

/// Run identity and lifecycle metadata. Payloads are deliberately not part of this read contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowRunMetadataReadModel {
    pub id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub draft_id: Uuid,
    pub compiled_plan_id: Option<Uuid>,
    pub debug_session_id: String,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub run_mode: domain::FlowRunMode,
    pub target_node_id: Option<String>,
    pub title: String,
    pub status: domain::FlowRunStatus,
    pub created_by: Uuid,
    pub authorized_account: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<&domain::FlowRunRecord> for FlowRunMetadataReadModel {
    fn from(run: &domain::FlowRunRecord) -> Self {
        Self {
            id: run.id,
            application_id: run.application_id,
            flow_id: run.flow_id,
            draft_id: run.draft_id,
            compiled_plan_id: run.compiled_plan_id,
            debug_session_id: run.debug_session_id.clone(),
            flow_schema_version: run.flow_schema_version.clone(),
            document_hash: run.document_hash.clone(),
            run_mode: run.run_mode,
            target_node_id: run.target_node_id.clone(),
            title: run.title.clone(),
            status: run.status,
            created_by: run.created_by,
            authorized_account: run.authorized_account.clone(),
            api_key_id: run.api_key_id,
            publication_version_id: run.publication_version_id,
            external_user: run.external_user.clone(),
            external_conversation_id: run.external_conversation_id.clone(),
            external_trace_id: run.external_trace_id.clone(),
            compatibility_mode: run.compatibility_mode.clone(),
            idempotency_key: run.idempotency_key.clone(),
            started_at: run.started_at,
            finished_at: run.finished_at,
            created_at: run.created_at,
            updated_at: run.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunOverviewReadModel {
    pub flow_run: FlowRunMetadataReadModel,
    pub tool_callback_count: i64,
    pub waiting_node_id: Option<String>,
    pub waiting_node_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunResumeTimelineReadModel {
    pub flow_run: domain::FlowRunRecord,
    pub callback_tasks: Vec<domain::CallbackTaskRecord>,
    pub events: Vec<domain::RunEventRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunResumeTimelineSummaryReadModel {
    pub flow_run_status: domain::FlowRunStatus,
    pub callback_tasks: Vec<ApplicationRunResumeCallbackSummary>,
    pub events: Vec<ApplicationRunResumeEventSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunResumeCallbackSummary {
    pub id: Uuid,
    pub callback_kind: String,
    pub status: domain::CallbackTaskStatus,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunResumeEventSummary {
    pub id: Uuid,
    pub event_type: String,
    pub description: Option<String>,
    pub created_at: OffsetDateTime,
}
