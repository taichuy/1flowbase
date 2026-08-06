#[derive(Debug, Deserialize, ToSchema)]
pub struct StartNodeDebugPreviewBody {
    pub input_payload: serde_json::Value,
    pub document: Option<serde_json::Value>,
    pub debug_session_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartFlowDebugRunBody {
    pub input_payload: serde_json::Value,
    pub document: Option<serde_json::Value>,
    pub debug_session_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DebugRunStreamQuery {
    pub from_sequence: Option<i64>,
    pub last_event_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RuntimeDebugStreamQuery {
    pub from_sequence: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ApplicationRunsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub time_range_days: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub cache_mode: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResumeFlowRunBody {
    pub checkpoint_id: String,
    pub input_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteCallbackTaskBody {
    pub response_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveRuntimeDebugArtifactsBody {
    pub artifact_refs: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationRunSelectedExportBody {
    pub run_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RunArchiveUploadSessionCreateBody {
    pub filename: Option<String>,
    pub total_size_bytes: i64,
    pub expected_sha256: Option<String>,
    pub chunk_size_bytes: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RunArchiveV1SourceResponse {
    pub source_kind: String,
    pub workspace_id: String,
    pub application_id: String,
    pub application_type: String,
    pub application_name: String,
    pub exported_by_user_id: String,
    pub exported_at: String,
    pub archive_builder: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RunArchiveV1ManifestResponse {
    pub archive_version: i32,
    pub archive_semantics: String,
    pub exported_at: String,
    pub source_workspace_id: String,
    pub source_application_id: String,
    pub run_count: usize,
    pub selected_run_ids: Vec<String>,
    pub entries: Vec<RunArchiveV1ManifestEntryResponse>,
    pub content_sha256: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RunArchiveV1ManifestEntryResponse {
    pub source_run_id: String,
    pub content_sha256: String,
    pub content_digest: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RunArchiveV1EntryResponse {
    pub source_run_id: String,
    pub content_digest: String,
    pub flow_run: FlowRunResponse,
    pub flow_run_fact: serde_json::Value,
    pub compiled_plan: Option<serde_json::Value>,
    pub node_runs: Vec<NodeRunResponse>,
    pub checkpoints: Vec<CheckpointResponse>,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
    pub runtime_spans: Vec<serde_json::Value>,
    pub runtime_events: Vec<serde_json::Value>,
    pub runtime_items: Vec<serde_json::Value>,
    pub context_projections: Vec<serde_json::Value>,
    pub usage_ledger: Vec<serde_json::Value>,
    pub model_failover_attempts: Vec<serde_json::Value>,
    pub capability_invocations: Vec<serde_json::Value>,
    pub trace_tree: serde_json::Value,
    pub export_warnings: Vec<ApplicationRunTraceExportWarningResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RunArchiveV1Response {
    pub archive_version: i32,
    pub exported_at: String,
    pub manifest: RunArchiveV1ManifestResponse,
    pub source: RunArchiveV1SourceResponse,
    pub entries: Vec<RunArchiveV1EntryResponse>,
    pub content_digest: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunArchiveUploadSessionResponse {
    pub session_id: String,
    pub application_id: String,
    pub status: String,
    pub filename: Option<String>,
    pub total_size_bytes: i64,
    pub received_bytes: i64,
    pub expected_sha256: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunArchiveChunkUploadResponse {
    pub session_id: String,
    pub chunk_index: i32,
    pub chunk_size_bytes: i64,
    pub chunk_sha256: String,
    pub received_bytes: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunArchiveImportRunMappingResponse {
    pub source_run_id: String,
    pub target_run_id: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunArchiveImportJobResponse {
    pub job_id: String,
    pub application_id: String,
    pub upload_session_id: String,
    pub status: String,
    pub archive_version: Option<i32>,
    pub archive_sha256: Option<String>,
    pub run_count: i32,
    pub imported_run_count: i32,
    pub source_to_target_run_ids: Vec<RunArchiveImportRunMappingResponse>,
    pub error_payload: Option<serde_json::Value>,
    pub result_payload: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationRunArchiveQuery {
    pub archive_version: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationRunArchiveBody {
    pub run_ids: Vec<Uuid>,
    pub archive_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RuntimeDebugArtifactValueResponse {
    pub artifact_ref: String,
    pub content_type: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResolveRuntimeDebugArtifactsResponse {
    pub artifacts: Vec<RuntimeDebugArtifactValueResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct FlowRunSummaryResponse {
    pub id: String,
    pub application_id: String,
    pub application_type: String,
    pub run_object_kind: String,
    pub run_kind: String,
    pub run_mode: String,
    pub status: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub expand_id: Option<String>,
    pub authorized_account: Option<String>,
    pub execution_stage: String,
    pub invocation_source: String,
    pub compatibility_mode: Option<String>,
    pub subject: application_logs::ApplicationRunSubjectResponse,
    pub principal: application_logs::ApplicationRunPrincipalResponse,
    pub correlation: application_logs::ApplicationRunCorrelationResponse,
    pub statistics: application_logs::ApplicationRunStatisticsResponse,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct FlowRunSummaryPageResponse {
    pub items: Vec<FlowRunSummaryResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct FlowRunResponse {
    pub id: String,
    pub application_id: String,
    pub flow_id: String,
    pub draft_id: String,
    pub compiled_plan_id: Option<String>,
    pub run_mode: String,
    pub status: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub expand_id: Option<String>,
    pub authorized_account: Option<String>,
    pub external_conversation_id: Option<String>,
    pub query: Option<String>,
    pub model: Option<String>,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub created_by: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationConversationMessagesQuery {
    pub around_run_id: Option<Uuid>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationConversationMessageResponse {
    pub run_id: String,
    pub detail_run_id: Option<String>,
    pub can_open_detail: bool,
    pub role: Option<String>,
    pub content: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub query: Option<String>,
    pub model: Option<String>,
    pub answer: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationConversationMessagesPageInfoResponse {
    pub has_before: bool,
    pub has_after: bool,
    pub before_cursor: Option<String>,
    pub after_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationConversationMessagesPageResponse {
    pub items: Vec<ApplicationConversationMessageResponse>,
    pub page: ApplicationConversationMessagesPageInfoResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct NodeRunResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub status: String,
    pub input_payload: serde_json::Value,
    pub input_payload_view: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CheckpointResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_run_id: Option<String>,
    pub status: String,
    pub reason: String,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CallbackTaskResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_run_id: String,
    pub callback_kind: String,
    pub status: String,
    pub request_payload: serde_json::Value,
    pub response_payload: Option<serde_json::Value>,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RunEventResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_run_id: Option<String>,
    pub sequence: i64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AnswerSnapshotResponse {
    pub kind: String,
    pub text: String,
    pub output_payload: serde_json::Value,
    pub complete: bool,
    pub materialized_from: String,
    pub answer_node_id: Option<String>,
    pub answer_node_run_id: Option<String>,
    pub waiting_node_id: Option<String>,
    pub waiting_node_run_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ContextSnapshotMeasurementResponse {
    pub method: String,
    pub accuracy: String,
    pub coverage: String,
    pub unknown_block_count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ContextSnapshotResponse {
    #[serde(rename = "type")]
    pub event_type: String,
    pub event_id: String,
    pub run_id: String,
    pub node_run_id: Option<String>,
    pub node_id: String,
    pub sequence: i64,
    pub input_tokens: u64,
    pub effective_context_window: Option<u64>,
    pub remaining_tokens: Option<u64>,
    pub measurement: ContextSnapshotMeasurementResponse,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunStitchedTraceResponse {
    pub source_flow_run: FlowRunResponse,
    pub node_runs: Vec<NodeRunResponse>,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunDetailResponse {
    pub run: application_logs::ApplicationRunLogResponse,
    pub statistics: application_logs::ApplicationRunStatisticsResponse,
    pub detail: application_logs::ApplicationRunTypedDetailResponse,
    pub flow_run: FlowRunResponse,
    pub answer_snapshot: Option<AnswerSnapshotResponse>,
    pub context_snapshot: Option<ContextSnapshotResponse>,
    pub node_runs: Vec<NodeRunResponse>,
    pub checkpoints: Vec<CheckpointResponse>,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
    pub stitched_trace: Vec<ApplicationRunStitchedTraceResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunOverviewResponse {
    pub run: application_logs::ApplicationRunLogResponse,
    pub statistics: application_logs::ApplicationRunStatisticsResponse,
    pub flow_run: FlowRunResponse,
    pub answer_snapshot: Option<AnswerSnapshotResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceNodeSummaryResponse {
    pub trace_node_id: String,
    pub stable_locator: String,
    pub parent_trace_node_id: Option<String>,
    pub node_kind: String,
    pub flow_run_id: String,
    pub node_run_id: Option<String>,
    pub callback_task_id: Option<String>,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub node_mode: Option<String>,
    pub node_alias: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub metrics_payload: serde_json::Value,
    pub has_children: bool,
    pub child_count: i64,
    pub has_content: bool,
    pub source_flow_run_id: Option<String>,
    pub source_trace_node_id: Option<String>,
    pub parent_callback_task_id: Option<String>,
    pub parent_tool_call_id: Option<String>,
    pub trace_relation_kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunTraceProjectionStatusResponse {
    pub projection_status: String,
    pub projection_version: i32,
    pub source_watermark: String,
    pub attempt_count: i32,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_stage: Option<String>,
    pub last_error_source_kind: Option<String>,
    pub last_error_source_locator: Option<String>,
    pub last_error_ref: Option<String>,
    pub retriable: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceTreeResponse {
    pub run: application_logs::ApplicationRunLogResponse,
    pub statistics: application_logs::ApplicationRunStatisticsResponse,
    pub flow_run: FlowRunResponse,
    pub answer_snapshot: Option<AnswerSnapshotResponse>,
    pub projection_status: ApplicationRunTraceProjectionStatusResponse,
    pub nodes: Vec<ApplicationRunTraceNodeSummaryResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunTraceExportWarningResponse {
    pub code: String,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunTraceExportNodeResponse {
    pub trace_node_id: String,
    pub stable_locator: String,
    pub parent_trace_node_id: Option<String>,
    pub node_kind: String,
    pub flow_run_id: String,
    pub node_run_id: Option<String>,
    pub callback_task_id: Option<String>,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub node_mode: Option<String>,
    pub node_alias: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub metrics_payload: serde_json::Value,
    pub has_children: bool,
    pub child_count: i64,
    pub has_content: bool,
    pub source_flow_run_id: Option<String>,
    pub source_trace_node_id: Option<String>,
    pub parent_callback_task_id: Option<String>,
    pub parent_tool_call_id: Option<String>,
    pub trace_relation_kind: Option<String>,
    pub content_kind: Option<String>,
    pub source_refs: serde_json::Value,
    pub detail_refs: serde_json::Value,
    pub payload: serde_json::Value,
    #[schema(no_recursion)]
    pub children: Vec<ApplicationRunTraceExportNodeResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunTraceExportTreeResponse {
    pub run: application_logs::ApplicationRunLogResponse,
    pub statistics: application_logs::ApplicationRunStatisticsResponse,
    pub flow_run: FlowRunResponse,
    pub answer_snapshot: Option<AnswerSnapshotResponse>,
    pub projection_status: ApplicationRunTraceProjectionStatusResponse,
    pub nodes: Vec<ApplicationRunTraceExportNodeResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunTraceExportResponse {
    pub export_version: i32,
    pub exported_at: String,
    pub export_status: String,
    pub export_warnings: Vec<ApplicationRunTraceExportWarningResponse>,
    pub run: application_logs::ApplicationRunLogResponse,
    pub statistics: application_logs::ApplicationRunStatisticsResponse,
    pub detail: application_logs::ApplicationRunTypedDetailResponse,
    pub flow_run: FlowRunResponse,
    pub answer_snapshot: Option<AnswerSnapshotResponse>,
    pub node_runs: Vec<NodeRunResponse>,
    pub checkpoints: Vec<CheckpointResponse>,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
    pub stitched_trace: Vec<ApplicationRunStitchedTraceResponse>,
    pub trace_tree: ApplicationRunTraceExportTreeResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunSelectedExportManifestRunResponse {
    pub run_id: String,
    pub title: String,
    pub started_at: String,
    pub filename: String,
    pub export_status: String,
    pub export_warning_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunSelectedExportManifestResponse {
    pub export_version: i32,
    pub exported_at: String,
    pub export_status: String,
    pub application_id: String,
    pub run_count: usize,
    pub selected_run_ids: Vec<String>,
    pub entries: Vec<ApplicationRunSelectedExportManifestRunResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationRunTraceNodeChildrenQuery {
    pub parent_trace_node_id: String,
    pub page_size: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceNodeChildrenPageInfoResponse {
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub page_size: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceNodeChildrenResponse {
    pub projection_status: ApplicationRunTraceProjectionStatusResponse,
    pub items: Vec<ApplicationRunTraceNodeSummaryResponse>,
    pub page_info: ApplicationRunTraceNodeChildrenPageInfoResponse,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceNodeContentResponse {
    pub trace_node_id: String,
    pub node_kind: String,
    pub projection_status: ApplicationRunTraceProjectionStatusResponse,
    pub content_kind: String,
    pub source_refs: serde_json::Value,
    pub detail_refs: serde_json::Value,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceNodeDetailResponse {
    pub trace_node_id: String,
    pub node_kind: String,
    pub projection_status: ApplicationRunTraceProjectionStatusResponse,
    pub detail_ref_id: String,
    pub detail_kind: String,
    pub source_refs: serde_json::Value,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunTraceToolCallbackContentResponse {
    pub trace_node_id: String,
    pub tool_call_id: String,
    pub projection_status: ApplicationRunTraceProjectionStatusResponse,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunResumeTimelineResponse {
    pub flow_run: FlowRunResponse,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDebugStreamResponse {
    pub parts: Vec<RuntimeDebugStreamPartResponse>,
    pub page_size: i64,
    pub next_sequence: Option<i64>,
    pub has_more: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDebugStreamPartResponse {
    pub id: String,
    pub flow_run_id: String,
    pub item_id: Option<String>,
    pub span_id: Option<String>,
    pub part_type: String,
    pub status: String,
    pub trust_level: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeLastRunResponse {
    pub flow_run: FlowRunResponse,
    pub node_run: NodeRunResponse,
    pub checkpoints: Vec<CheckpointResponse>,
    pub events: Vec<RunEventResponse>,
}
