use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    application_public_api::{
        callback_resume::ApplicationPublishedCallbackAttemptRepository,
        conversations::{
            ApplicationPublicConversationMessageRecord, ApplicationPublicConversationRecord,
            ApplicationPublicConversationRepository, BindApplicationPublicConversationInput,
            ListApplicationPublicConversationMessagesInput,
        },
        run_service::{
            ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
            CancelPublishedFlowRunInput, CreatePublishedFlowRunResult,
            ListWaitingCallbackPublishedRunsInput, PublishedRunNodeUsage,
            PublishedRunPendingCallback, PublishedRunStreamState,
        },
    },
    errors::ControlPlaneError,
    ports::{
        AppendBillingSessionInput, AppendCapabilityInvocationInput, AppendContextProjectionInput,
        AppendCostLedgerInput, AppendCreditLedgerInput, AppendModelFailoverAttemptLedgerInput,
        AppendRunEventInput, AppendRuntimeEventInput, AppendRuntimeItemInput,
        AppendRuntimeSpanInput, AppendUsageLedgerInput, ApplicationRunCountTokensResult,
        ApplicationRunTraceChildrenCursor, ApplicationRunTraceProjectionStatistics,
        AttachCompiledPlanToFlowRunInput, CallbackResumeContext, CallbackResumeWaitingNode,
        ClearModelProviderRequestLogsBatchInput, ClearModelProviderRequestLogsBatchResult,
        CommitFlowRunTerminalInput, CommitFlowRunTerminalReceipt, CommitFlowRunTerminalResult,
        CompleteCallbackTaskInput, CompleteFlowRunInput, CompleteNodeRunInput,
        CreateCallbackTaskInput, CreateCheckpointInput, CreateFlowRunInput,
        CreateFlowRunShellInput, CreateNodeRunInput, CreateRuntimeDebugArtifactInput,
        DataModelSideEffectReceiptClaim, DebugVariableCacheEntry,
        DeleteDebugVariableCacheEntriesInput, DeleteModelProviderRequestLogsInput,
        FailQueuedFlowRunShellInput, FinalizePublishedRunMissingStreamTerminalPersistenceInput,
        FinalizePublishedRunMissingStreamTerminalPersistenceOutcome,
        FinishFlowRunCallbackResumeAttemptInput, GetApplicationRunMonitoringReportInput,
        GetRuntimeDebugArtifactInput, LinkUsageLedgerToModelFailoverAttemptInput,
        ListApplicationConversationRunsPageInput,
        ListApplicationRunConversationMessageItemsPageInput, ListApplicationRunTraceChildrenPage,
        ListApplicationRunTraceChildrenPageInput, ListApplicationRunsPageInput,
        ListModelProviderRequestLogsPageInput, ModelProviderRequestLogsPage,
        OrchestrationRuntimeRepository, RecordFlowRunCallbackResumeAttemptInput,
        RecordFlowRunCallbackResumeAttemptOutput, ReplaceApplicationRunTraceProjectionInput,
        UpdateCallbackTaskPayloadsInput, UpdateCheckpointPayloadsInput, UpdateFlowRunInput,
        UpdateFlowRunPayloadsInput, UpdateNodeRunInput, UpdateNodeRunPayloadsInput,
        UpdateRunEventPayloadInput, UpsertApplicationRunTraceProjectionStatusInput,
        UpsertCompiledPlanInput, UpsertDataModelSideEffectReceiptInput,
        UpsertDebugVariableCacheEntryInput,
    },
};
use sqlx::{Postgres, QueryBuilder, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

mod detail_queries;
mod record_mappers;
mod sequencing;

use detail_queries::*;
use record_mappers::*;
use sequencing::*;

include!("event_methods.rs");
include!("artifact_methods.rs");
include!("application_run_log_methods.rs");
include!("application_run_logs/assistant_conversation_methods.rs");
include!("application_run_logs/run_conversation_message_item_methods.rs");
include!("application_run_trace_projection_methods.rs");
include!("application_run_monitoring_methods.rs");
include!("debug_variable_cache_methods.rs");
include!("flow_run_methods.rs");
include!("flow_run_callback_resume_attempt_methods.rs");
include!("ledger_methods.rs");
include!("read_methods.rs");
include!("request_log_methods.rs");
include!("side_effect_receipt_methods.rs");

#[async_trait]
impl OrchestrationRuntimeRepository for PgControlPlaneStore {
    async fn upsert_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> Result<domain::CompiledPlanRecord> {
        PgControlPlaneStore::upsert_compiled_plan(self, input).await
    }

    async fn get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        PgControlPlaneStore::get_compiled_plan(self, compiled_plan_id).await
    }

    async fn create_flow_run(&self, input: &CreateFlowRunInput) -> Result<domain::FlowRunRecord> {
        PgControlPlaneStore::create_flow_run(self, input).await
    }

    async fn create_flow_run_shell(
        &self,
        input: &CreateFlowRunShellInput,
    ) -> Result<domain::FlowRunRecord> {
        PgControlPlaneStore::create_flow_run_shell(self, input).await
    }

    async fn attach_compiled_plan_to_flow_run(
        &self,
        input: &AttachCompiledPlanToFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        PgControlPlaneStore::attach_compiled_plan_to_flow_run(self, input).await
    }

    async fn fail_queued_flow_run_shell(
        &self,
        input: &FailQueuedFlowRunShellInput,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::fail_queued_flow_run_shell(self, input).await
    }

    async fn get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::get_flow_run(self, application_id, flow_run_id).await
    }

    async fn create_node_run(&self, input: &CreateNodeRunInput) -> Result<domain::NodeRunRecord> {
        PgControlPlaneStore::create_node_run(self, input).await
    }

    async fn update_node_run(&self, input: &UpdateNodeRunInput) -> Result<domain::NodeRunRecord> {
        PgControlPlaneStore::update_node_run(self, input).await
    }

    async fn complete_node_run(
        &self,
        input: &CompleteNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
        PgControlPlaneStore::complete_node_run(self, input).await
    }

    async fn update_flow_run(&self, input: &UpdateFlowRunInput) -> Result<domain::FlowRunRecord> {
        PgControlPlaneStore::update_flow_run(self, input).await
    }

    async fn update_flow_run_if_status(
        &self,
        input: &UpdateFlowRunInput,
        expected_status: domain::FlowRunStatus,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::update_flow_run_if_status(self, input, expected_status).await
    }

    async fn commit_flow_run_terminal(
        &self,
        input: &CommitFlowRunTerminalInput,
    ) -> Result<CommitFlowRunTerminalReceipt> {
        PgControlPlaneStore::commit_flow_run_terminal(self, input).await
    }

    async fn finalize_published_run_missing_stream_terminal(
        &self,
        input: &FinalizePublishedRunMissingStreamTerminalPersistenceInput,
    ) -> Result<FinalizePublishedRunMissingStreamTerminalPersistenceOutcome> {
        PgControlPlaneStore::finalize_published_run_missing_stream_terminal(self, input).await
    }

    async fn complete_flow_run(
        &self,
        input: &CompleteFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        PgControlPlaneStore::complete_flow_run(self, input).await
    }

    async fn get_checkpoint(
        &self,
        flow_run_id: Uuid,
        checkpoint_id: Uuid,
    ) -> Result<Option<domain::CheckpointRecord>> {
        PgControlPlaneStore::get_checkpoint(self, flow_run_id, checkpoint_id).await
    }

    async fn create_checkpoint(
        &self,
        input: &CreateCheckpointInput,
    ) -> Result<domain::CheckpointRecord> {
        PgControlPlaneStore::create_checkpoint(self, input).await
    }

    async fn create_callback_task(
        &self,
        input: &CreateCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        PgControlPlaneStore::create_callback_task(self, input).await
    }

    async fn get_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        PgControlPlaneStore::get_callback_task(self, callback_task_id).await
    }

    async fn get_callback_resume_context(
        &self,
        application_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<CallbackResumeContext>> {
        PgControlPlaneStore::get_callback_resume_context(self, application_id, callback_task_id)
            .await
    }

    async fn complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        PgControlPlaneStore::complete_callback_task(self, input).await
    }

    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        PgControlPlaneStore::append_run_event(self, input).await
    }

    async fn append_run_events(
        &self,
        inputs: &[AppendRunEventInput],
    ) -> Result<Vec<domain::RunEventRecord>> {
        PgControlPlaneStore::append_run_events(self, inputs).await
    }

    async fn update_flow_run_payloads(
        &self,
        input: &UpdateFlowRunPayloadsInput,
    ) -> Result<domain::FlowRunRecord> {
        PgControlPlaneStore::update_flow_run_payloads(self, input).await
    }

    async fn update_node_run_payloads(
        &self,
        input: &UpdateNodeRunPayloadsInput,
    ) -> Result<domain::NodeRunRecord> {
        PgControlPlaneStore::update_node_run_payloads(self, input).await
    }

    async fn update_run_event_payload(
        &self,
        input: &UpdateRunEventPayloadInput,
    ) -> Result<domain::RunEventRecord> {
        PgControlPlaneStore::update_run_event_payload(self, input).await
    }

    async fn update_checkpoint_payloads(
        &self,
        input: &UpdateCheckpointPayloadsInput,
    ) -> Result<domain::CheckpointRecord> {
        PgControlPlaneStore::update_checkpoint_payloads(self, input).await
    }

    async fn update_callback_task_payloads(
        &self,
        input: &UpdateCallbackTaskPayloadsInput,
    ) -> Result<domain::CallbackTaskRecord> {
        PgControlPlaneStore::update_callback_task_payloads(self, input).await
    }

    async fn record_flow_run_callback_resume_attempt(
        &self,
        input: &RecordFlowRunCallbackResumeAttemptInput,
    ) -> Result<RecordFlowRunCallbackResumeAttemptOutput> {
        PgControlPlaneStore::record_flow_run_callback_resume_attempt(self, input).await
    }

    async fn get_flow_run_callback_resume_attempt_by_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::get_flow_run_callback_resume_attempt_by_callback_task(
            self,
            callback_task_id,
        )
        .await
    }

    async fn finish_flow_run_callback_resume_attempt(
        &self,
        input: &FinishFlowRunCallbackResumeAttemptInput,
    ) -> Result<domain::FlowRunCallbackResumeAttemptRecord> {
        PgControlPlaneStore::finish_flow_run_callback_resume_attempt(self, input).await
    }

    async fn upsert_debug_variable_cache_entry(
        &self,
        input: &UpsertDebugVariableCacheEntryInput,
    ) -> Result<DebugVariableCacheEntry> {
        PgControlPlaneStore::upsert_debug_variable_cache_entry(self, input).await
    }

    async fn list_debug_variable_cache_entries(
        &self,
        application_id: Uuid,
        draft_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<Vec<DebugVariableCacheEntry>> {
        PgControlPlaneStore::list_debug_variable_cache_entries(
            self,
            application_id,
            draft_id,
            actor_user_id,
        )
        .await
    }

    async fn delete_debug_variable_cache_entries(
        &self,
        input: &DeleteDebugVariableCacheEntriesInput,
    ) -> Result<()> {
        PgControlPlaneStore::delete_debug_variable_cache_entries(self, input).await
    }

    async fn create_runtime_debug_artifact(
        &self,
        input: &CreateRuntimeDebugArtifactInput,
    ) -> Result<domain::RuntimeDebugArtifactRecord> {
        PgControlPlaneStore::create_runtime_debug_artifact(self, input).await
    }

    async fn get_runtime_debug_artifact(
        &self,
        input: &GetRuntimeDebugArtifactInput,
    ) -> Result<Option<domain::RuntimeDebugArtifactRecord>> {
        PgControlPlaneStore::get_runtime_debug_artifact(self, input).await
    }

    async fn get_data_model_side_effect_receipt(
        &self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<domain::DataModelSideEffectReceiptRecord>> {
        PgControlPlaneStore::get_data_model_side_effect_receipt(self, workspace_id, idempotency_key)
            .await
    }

    async fn claim_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<DataModelSideEffectReceiptClaim> {
        PgControlPlaneStore::claim_data_model_side_effect_receipt(self, input).await
    }

    async fn upsert_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<domain::DataModelSideEffectReceiptRecord> {
        PgControlPlaneStore::upsert_data_model_side_effect_receipt(self, input).await
    }

    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> Result<domain::RuntimeSpanRecord> {
        PgControlPlaneStore::append_runtime_span(self, input).await
    }

    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> Result<domain::RuntimeEventRecord> {
        PgControlPlaneStore::append_runtime_event(self, input).await
    }

    async fn append_runtime_events(
        &self,
        inputs: &[AppendRuntimeEventInput],
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        PgControlPlaneStore::append_runtime_events(self, inputs).await
    }

    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> Result<domain::RuntimeItemRecord> {
        PgControlPlaneStore::append_runtime_item(self, input).await
    }

    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> Result<domain::ContextProjectionRecord> {
        PgControlPlaneStore::append_context_projection(self, input).await
    }

    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> Result<domain::UsageLedgerRecord> {
        PgControlPlaneStore::append_usage_ledger(self, input).await
    }

    async fn append_cost_ledger(
        &self,
        input: &AppendCostLedgerInput,
    ) -> Result<domain::CostLedgerRecord> {
        PgControlPlaneStore::append_cost_ledger(self, input).await
    }

    async fn append_credit_ledger(
        &self,
        input: &AppendCreditLedgerInput,
    ) -> Result<domain::CreditLedgerRecord> {
        PgControlPlaneStore::append_credit_ledger(self, input).await
    }

    async fn append_billing_session(
        &self,
        input: &AppendBillingSessionInput,
    ) -> Result<domain::BillingSessionRecord> {
        PgControlPlaneStore::append_billing_session(self, input).await
    }

    async fn append_audit_hash(
        &self,
        flow_run_id: Uuid,
        fact_table: &str,
        fact_id: Uuid,
        payload: serde_json::Value,
    ) -> Result<domain::AuditHashRecord> {
        PgControlPlaneStore::append_audit_hash(self, flow_run_id, fact_table, fact_id, payload)
            .await
    }

    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        PgControlPlaneStore::append_model_failover_attempt_ledger(self, input).await
    }

    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        PgControlPlaneStore::link_usage_ledger_to_model_failover_attempt(self, input).await
    }

    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> Result<domain::CapabilityInvocationRecord> {
        PgControlPlaneStore::append_capability_invocation(self, input).await
    }

    async fn list_runtime_spans(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeSpanRecord>> {
        PgControlPlaneStore::list_runtime_spans(self, flow_run_id).await
    }

    async fn list_runtime_events(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        PgControlPlaneStore::list_runtime_events(self, flow_run_id, after_sequence).await
    }

    async fn get_runtime_event_sequence_for_callback_task(
        &self,
        flow_run_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<i64>> {
        PgControlPlaneStore::get_runtime_event_sequence_for_callback_task(
            self,
            flow_run_id,
            callback_task_id,
        )
        .await
    }

    async fn list_runtime_event_backfill_page(
        &self,
        flow_run_id: Uuid,
        after_stream_sequence: i64,
        limit: usize,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        PgControlPlaneStore::list_runtime_event_backfill_page(
            self,
            flow_run_id,
            after_stream_sequence,
            limit,
        )
        .await
    }

    async fn list_runtime_items(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeItemRecord>> {
        PgControlPlaneStore::list_runtime_items(self, flow_run_id).await
    }

    async fn list_context_projections(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ContextProjectionRecord>> {
        PgControlPlaneStore::list_context_projections(self, flow_run_id).await
    }

    async fn list_usage_ledger(&self, flow_run_id: Uuid) -> Result<Vec<domain::UsageLedgerRecord>> {
        PgControlPlaneStore::list_usage_ledger(self, flow_run_id).await
    }

    async fn list_model_failover_attempt_ledger(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverAttemptLedgerRecord>> {
        PgControlPlaneStore::list_model_failover_attempt_ledger(self, flow_run_id).await
    }

    async fn insert_model_provider_request_logs_batch(
        &self,
        records: &[control_plane::ports::ProviderRequestLogTask],
    ) -> Result<()> {
        PgControlPlaneStore::insert_model_provider_request_logs_batch(self, records).await
    }

    async fn list_model_provider_request_logs_page(
        &self,
        input: ListModelProviderRequestLogsPageInput,
    ) -> Result<ModelProviderRequestLogsPage> {
        PgControlPlaneStore::list_model_provider_request_logs_page(self, input).await
    }

    async fn delete_model_provider_request_logs(
        &self,
        input: DeleteModelProviderRequestLogsInput,
    ) -> Result<u64> {
        PgControlPlaneStore::delete_model_provider_request_logs(self, input).await
    }

    async fn clear_model_provider_request_logs_batch(
        &self,
        input: ClearModelProviderRequestLogsBatchInput,
    ) -> Result<ClearModelProviderRequestLogsBatchResult> {
        PgControlPlaneStore::clear_model_provider_request_logs_batch(self, input).await
    }

    async fn list_capability_invocations(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::CapabilityInvocationRecord>> {
        PgControlPlaneStore::list_capability_invocations(self, flow_run_id).await
    }

    async fn list_application_runs(
        &self,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunSummary>> {
        PgControlPlaneStore::list_application_runs(self, application_id).await
    }

    async fn list_application_runs_page(
        &self,
        application_id: Uuid,
        input: control_plane::ports::ListApplicationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunSummaryPage> {
        PgControlPlaneStore::list_application_runs_page(self, application_id, input).await
    }

    async fn list_application_run_logs_page(
        &self,
        application_id: Uuid,
        input: control_plane::ports::ListApplicationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunLogSummaryPage> {
        PgControlPlaneStore::list_application_run_logs_page(self, application_id, input).await
    }

    async fn list_application_run_count_tokens_results(
        &self,
        flow_run_ids: &[Uuid],
    ) -> Result<Vec<ApplicationRunCountTokensResult>> {
        PgControlPlaneStore::list_application_run_count_tokens_results(self, flow_run_ids).await
    }

    async fn get_application_run_monitoring_report(
        &self,
        application_id: Uuid,
        input: GetApplicationRunMonitoringReportInput,
    ) -> Result<control_plane::ports::ApplicationRunMonitoringReport> {
        PgControlPlaneStore::get_application_run_monitoring_report(self, application_id, input)
            .await
    }

    async fn list_application_conversation_runs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationConversationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationConversationRunsPage> {
        PgControlPlaneStore::list_application_conversation_runs_page(self, application_id, input)
            .await
    }

    async fn list_application_run_conversation_message_items_page(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        input: ListApplicationRunConversationMessageItemsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunConversationMessageItemsPage> {
        PgControlPlaneStore::list_application_run_conversation_message_items_page(
            self,
            application_id,
            flow_run_id,
            input,
        )
        .await
    }

    async fn get_application_run_conversation_current_item(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunConversationMessageItem>> {
        PgControlPlaneStore::get_application_run_conversation_current_item(
            self,
            application_id,
            flow_run_id,
        )
        .await
    }

    async fn get_application_run_detail(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunDetail>> {
        PgControlPlaneStore::get_application_run_detail(self, application_id, flow_run_id).await
    }

    async fn get_application_run_trace_projection_source(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunDetail>> {
        PgControlPlaneStore::get_application_run_trace_projection_source(
            self,
            application_id,
            flow_run_id,
        )
        .await
    }

    async fn get_application_run_trace_projection_source_watermark(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<String>> {
        PgControlPlaneStore::get_application_run_trace_projection_source_watermark(
            self,
            application_id,
            flow_run_id,
        )
        .await
    }

    async fn replace_application_run_trace_projection(
        &self,
        input: &ReplaceApplicationRunTraceProjectionInput,
    ) -> Result<()> {
        PgControlPlaneStore::replace_application_run_trace_projection(self, input).await
    }

    async fn upsert_application_run_trace_projection_status(
        &self,
        input: &UpsertApplicationRunTraceProjectionStatusInput,
    ) -> Result<()> {
        PgControlPlaneStore::upsert_application_run_trace_projection_status(self, input).await
    }

    async fn get_application_run_trace_projection_status(
        &self,
        flow_run_id: Uuid,
        projection_version: i32,
    ) -> Result<Option<domain::ApplicationRunTraceProjectionStatusRecord>> {
        PgControlPlaneStore::get_application_run_trace_projection_status(
            self,
            flow_run_id,
            projection_version,
        )
        .await
    }

    async fn list_application_run_trace_roots(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunTraceNodeRecord>> {
        PgControlPlaneStore::list_application_run_trace_roots(self, flow_run_id).await
    }

    async fn get_application_run_trace_statistics(
        &self,
        flow_run_id: Uuid,
    ) -> Result<ApplicationRunTraceProjectionStatistics> {
        PgControlPlaneStore::get_application_run_trace_statistics(self, flow_run_id).await
    }

    async fn list_application_run_trace_children_page(
        &self,
        input: ListApplicationRunTraceChildrenPageInput,
    ) -> Result<ListApplicationRunTraceChildrenPage> {
        PgControlPlaneStore::list_application_run_trace_children_page(self, input).await
    }

    async fn get_application_run_trace_node(
        &self,
        flow_run_id: Uuid,
        trace_node_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunTraceNodeRecord>> {
        PgControlPlaneStore::get_application_run_trace_node(self, flow_run_id, trace_node_id).await
    }

    async fn get_application_run_trace_node_by_locator(
        &self,
        flow_run_id: Uuid,
        stable_locator: &str,
    ) -> Result<Option<domain::ApplicationRunTraceNodeRecord>> {
        PgControlPlaneStore::get_application_run_trace_node_by_locator(
            self,
            flow_run_id,
            stable_locator,
        )
        .await
    }

    async fn get_application_run_trace_node_content(
        &self,
        flow_run_id: Uuid,
        trace_node_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunTraceNodeContentRecord>> {
        PgControlPlaneStore::get_application_run_trace_node_content(
            self,
            flow_run_id,
            trace_node_id,
        )
        .await
    }

    async fn list_application_run_trace_node_run_details(
        &self,
        flow_run_id: Uuid,
        node_run_ids: Vec<Uuid>,
    ) -> Result<Vec<domain::NodeRunRecord>> {
        PgControlPlaneStore::list_application_run_trace_node_run_details(
            self,
            flow_run_id,
            node_run_ids,
        )
        .await
    }

    async fn get_latest_node_run(
        &self,
        application_id: Uuid,
        node_id: &str,
    ) -> Result<Option<domain::NodeLastRun>> {
        PgControlPlaneStore::get_latest_node_run(self, application_id, node_id).await
    }
}

#[async_trait]
impl ApplicationPublishedFlowRunRepository for PgControlPlaneStore {
    async fn create_published_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<CreatePublishedFlowRunResult> {
        PgControlPlaneStore::create_published_flow_run(self, input).await
    }

    async fn find_published_flow_run_by_idempotency_key(
        &self,
        application_id: Uuid,
        api_key_id: Option<Uuid>,
        idempotency_key: &str,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::find_published_flow_run_by_idempotency_key(
            self,
            application_id,
            api_key_id,
            idempotency_key,
        )
        .await
    }

    async fn append_published_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        PgControlPlaneStore::append_run_event(self, input).await
    }

    async fn create_assistant_conversation(
        &self,
        input: &control_plane::application_public_api::run_service::CreateAssistantConversationInput,
    ) -> Result<control_plane::application_public_api::run_service::AssistantConversationRecord>
    {
        PgControlPlaneStore::create_assistant_conversation(self, input).await
    }

    async fn get_assistant_conversation(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<
        Option<control_plane::application_public_api::run_service::AssistantConversationRecord>,
    > {
        PgControlPlaneStore::get_assistant_conversation(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            conversation_id,
        )
        .await
    }

    async fn list_assistant_conversations(
        &self,
        input: &control_plane::application_public_api::run_service::ListAssistantConversationsInput,
    ) -> Result<control_plane::application_public_api::run_service::AssistantConversationPage> {
        PgControlPlaneStore::list_assistant_conversations(self, input).await
    }

    async fn list_assistant_conversation_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<control_plane::application_public_api::run_service::AssistantConversationMessage>>
    {
        PgControlPlaneStore::list_assistant_conversation_messages(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            conversation_id,
        )
        .await
    }

    async fn list_assistant_legacy_snapshot_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Vec<control_plane::application_public_api::run_service::AssistantConversationMessage>>
    {
        PgControlPlaneStore::list_assistant_legacy_snapshot_messages(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            flow_run_id,
        )
        .await
    }
}

#[async_trait]
impl ApplicationPublishedRunControlRepository for PgControlPlaneStore {
    async fn get_published_flow_run(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                input_payload,
                output_payload,
                error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            from flow_runs
            where id = $1
              and run_mode = 'published_api_run'
            "#,
        )
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_flow_run_record).transpose()
    }

    async fn find_published_flow_run_by_provider_response_id(
        &self,
        application_id: Uuid,
        api_key_id: Uuid,
        provider_response_id: &str,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let row = sqlx::query(
            r#"
            select
                flow_runs.id,
                flow_runs.application_id,
                flow_runs.flow_id,
                flow_runs.flow_draft_id,
                flow_runs.compiled_plan_id,
                flow_runs.debug_session_id,
                flow_runs.flow_schema_version,
                flow_runs.document_hash,
                flow_runs.run_mode,
                flow_runs.target_node_id,
                flow_runs.title,
                flow_runs.status,
                flow_runs.input_payload,
                flow_runs.output_payload,
                flow_runs.error_payload,
                flow_runs.created_by,
                null::text as authorized_account,
                flow_runs.api_key_id,
                flow_runs.publication_version_id,
                flow_runs.external_user,
                flow_runs.external_conversation_id,
                flow_runs.external_trace_id,
                flow_runs.compatibility_mode,
                flow_runs.idempotency_key,
                flow_runs.started_at,
                flow_runs.finished_at,
                flow_runs.created_at,
                flow_runs.updated_at
            from flow_runs
            join node_runs on node_runs.flow_run_id = flow_runs.id
            where flow_runs.application_id = $1
              and flow_runs.api_key_id = $2
              and flow_runs.run_mode = 'published_api_run'
              and node_runs.output_payload ->> 'response_id' = $3
            order by node_runs.finished_at desc nulls last, node_runs.id desc
            limit 1
            "#,
        )
        .bind(application_id)
        .bind(api_key_id)
        .bind(provider_response_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_flow_run_record).transpose()
    }

    async fn cancel_published_flow_run(
        &self,
        input: &CancelPublishedFlowRunInput,
    ) -> Result<CommitFlowRunTerminalReceipt> {
        PgControlPlaneStore::commit_flow_run_terminal(
            self,
            &CommitFlowRunTerminalInput {
                flow_run_id: input.flow_run_id,
                expected_status: input.from_status,
                result: CommitFlowRunTerminalResult::Cancelled {
                    output_payload: input.output_payload.clone(),
                    error_payload: input.error_payload.clone(),
                },
                flow_run_event_payload: input.flow_run_event_payload.clone(),
                terminal_event_payload: input.terminal_event_payload.clone(),
                finished_at: input.finished_at,
            },
        )
        .await
    }

    async fn cancel_published_pending_callback_tasks_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let rows = sqlx::query(
            r#"
            update flow_run_callback_tasks
            set status = 'cancelled',
                completed_at = $2
            where flow_run_id = $1
              and status = 'pending'
            returning
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                request_payload,
                response_payload,
                external_ref_payload,
                created_at,
                completed_at
            "#,
        )
        .bind(flow_run_id)
        .bind(completed_at)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_callback_task_record).collect()
    }

    async fn list_waiting_callback_published_flow_run_ids_for_conversation(
        &self,
        input: &ListWaitingCallbackPublishedRunsInput,
    ) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            select id
            from flow_runs
            where application_id = $1
              and api_key_id = $2
              and external_user = $3
              and external_conversation_id = $4
              and run_mode = 'published_api_run'
              and status = 'waiting_callback'
            order by started_at asc, id asc
            "#,
        )
        .bind(input.application_id)
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    async fn get_published_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                case
                    when callback_kind = 'llm_tool_calls'
                    then jsonb_build_object('tool_calls', request_payload -> 'tool_calls')
                    else request_payload
                end as request_payload,
                response_payload,
                case
                    when callback_kind = 'llm_tool_calls' then null
                    else external_ref_payload
                end as external_ref_payload,
                created_at,
                completed_at
            from flow_run_callback_tasks
            where id = $1
            "#,
        )
        .bind(callback_task_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_callback_task_record).transpose()
    }

    async fn get_published_run_stream_state(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<PublishedRunStreamState>> {
        let row = sqlx::query(
            r#"
            select
                status,
                output_payload,
                error_payload
            from flow_runs
            where application_id = $1
              and id = $2
              and run_mode = 'published_api_run'
              and (
                  import_job_id is null
                  or exists (
                      select 1
                      from run_archive_import_jobs import_jobs
                      where import_jobs.id = flow_runs.import_job_id
                        and import_jobs.status = 'succeeded'
                  )
              )
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let node_usage_rows = sqlx::query(
            r#"
            select
                metrics_payload -> 'usage' as metrics_usage,
                output_payload -> 'usage' as output_usage
            from node_runs
            where flow_run_id = $1
            order by started_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;
        let node_usages = node_usage_rows
            .into_iter()
            .map(|row| PublishedRunNodeUsage {
                metrics_usage: row.get("metrics_usage"),
                output_usage: row.get("output_usage"),
            })
            .collect();
        let latest_pending_callback = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                case
                    when callback_kind = 'llm_tool_calls' then null
                    else request_payload
                end as request_payload,
                case
                    when callback_kind = 'llm_tool_calls' then request_payload -> 'tool_calls'
                    else null
                end as tool_calls
            from flow_run_callback_tasks
            where flow_run_id = $1
              and status = 'pending'
            order by created_at desc, id desc
            limit 1
            "#,
        )
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?
        .map(|row| PublishedRunPendingCallback {
            id: row.get("id"),
            flow_run_id: row.get("flow_run_id"),
            node_run_id: row.get("node_run_id"),
            callback_kind: row.get("callback_kind"),
            request_payload: row.get("request_payload"),
            tool_calls: row.get("tool_calls"),
        });

        Ok(Some(PublishedRunStreamState {
            status: crate::mappers::orchestration_runtime_mapper::parse_flow_run_status(
                row.get::<String, _>("status").as_str(),
            )?,
            output_payload: row.get("output_payload"),
            error_payload: row.get("error_payload"),
            node_usages,
            latest_pending_callback,
        }))
    }
}

#[async_trait]
impl ApplicationPublishedCallbackAttemptRepository for PgControlPlaneStore {
    async fn record_published_callback_resume_attempt(
        &self,
        input: &RecordFlowRunCallbackResumeAttemptInput,
    ) -> Result<RecordFlowRunCallbackResumeAttemptOutput> {
        PgControlPlaneStore::record_flow_run_callback_resume_attempt(self, input).await
    }

    async fn get_published_callback_resume_attempt(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::get_flow_run_callback_resume_attempt_by_callback_task(
            self,
            callback_task_id,
        )
        .await
    }

    async fn finish_published_callback_resume_attempt(
        &self,
        input: &FinishFlowRunCallbackResumeAttemptInput,
    ) -> Result<domain::FlowRunCallbackResumeAttemptRecord> {
        PgControlPlaneStore::finish_flow_run_callback_resume_attempt(self, input).await
    }

    async fn cancel_published_callback_resume_attempts_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::cancel_flow_run_callback_resume_attempts_for_run(
            self,
            flow_run_id,
            completed_at,
        )
        .await
    }

    async fn fail_waiting_callback_published_run(
        &self,
        flow_run_id: Uuid,
        error_payload: serde_json::Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::update_flow_run_if_status(
            self,
            &UpdateFlowRunInput {
                flow_run_id,
                status: domain::FlowRunStatus::Failed,
                output_payload: serde_json::json!({}),
                error_payload: Some(error_payload),
                finished_at: Some(finished_at),
            },
            domain::FlowRunStatus::WaitingCallback,
        )
        .await
    }

    async fn complete_waiting_callback_published_internal_run(
        &self,
        flow_run_id: Uuid,
        output_payload: serde_json::Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let completed = PgControlPlaneStore::update_flow_run_if_status(
            self,
            &UpdateFlowRunInput {
                flow_run_id,
                status: domain::FlowRunStatus::Succeeded,
                output_payload: output_payload.clone(),
                error_payload: None,
                finished_at: Some(finished_at),
            },
            domain::FlowRunStatus::WaitingCallback,
        )
        .await?;

        if completed.is_some() {
            sqlx::query(
                r#"
                update node_runs
                set status = 'succeeded',
                    output_payload = $2,
                    error_payload = null,
                    finished_at = $3
                where flow_run_id = $1
                  and status = 'waiting_callback'
                "#,
            )
            .bind(flow_run_id)
            .bind(&output_payload)
            .bind(finished_at)
            .execute(self.pool())
            .await?;
        }

        Ok(completed)
    }
}

#[async_trait]
impl ApplicationPublicConversationRepository for PgControlPlaneStore {
    async fn bind_application_public_conversation(
        &self,
        input: &BindApplicationPublicConversationInput,
    ) -> Result<ApplicationPublicConversationRecord> {
        sqlx::query(
            r#"
            insert into application_conversations (
                id,
                scope_id,
                application_id,
                api_key_id,
                external_user,
                external_conversation_id
            )
            select
                $1,
                applications.workspace_id,
                applications.id,
                $2,
                $3,
                $4
            from applications
            where applications.id = $5
            on conflict (application_id, api_key_id, external_user, external_conversation_id)
            do update set updated_at = now()
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .bind(input.application_id)
        .execute(self.pool())
        .await?;

        let row = sqlx::query(
            r#"
            insert into application_public_conversations (
                id,
                application_id,
                api_key_id,
                external_user,
                external_conversation_id
            ) values ($1, $2, $3, $4, $5)
            on conflict (application_id, api_key_id, external_user, external_conversation_id)
            do update set updated_at = now()
            returning
                id,
                application_id,
                api_key_id,
                external_user,
                external_conversation_id,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.application_id)
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .fetch_one(self.pool())
        .await?;

        Ok(ApplicationPublicConversationRecord {
            id: row.get("id"),
            application_id: row.get("application_id"),
            api_key_id: row.get("api_key_id"),
            external_user: row.get("external_user"),
            external_conversation_id: row.get("external_conversation_id"),
            created_at: row.get::<OffsetDateTime, _>("created_at"),
            updated_at: row.get::<OffsetDateTime, _>("updated_at"),
        })
    }

    async fn list_application_public_conversation_messages(
        &self,
        input: &ListApplicationPublicConversationMessagesInput,
    ) -> Result<Vec<ApplicationPublicConversationMessageRecord>> {
        let limit = input.limit.clamp(1, 200);
        let hidden_internal_run_filter = hidden_anthropic_claude_code_internal_run_sql("runs");
        let rows = sqlx::query(&format!(
            r#"
            with conversation_messages as (
                select
                    messages.id,
                    messages.flow_run_id,
                    messages.role,
                    messages.content,
                    messages.sequence,
                    messages.created_at,
                    coalesce(messages.started_at, messages.created_at) as occurred_at
                from application_conversations conversations
                join application_conversation_messages messages
                  on messages.conversation_id = conversations.id
                join flow_runs runs
                  on runs.id = messages.flow_run_id
                where conversations.application_id = $1
                  and conversations.api_key_id = $2
                  and conversations.external_user = $3
                  and conversations.external_conversation_id = $4
                  and messages.flow_run_id is not null
                  and messages.role in ('user', 'assistant')
                  and btrim(messages.content) <> ''
                  and not ({hidden_internal_run_filter})
            ),
            current_turn_boundaries as (
                select
                    flow_run_id,
                    max(sequence) filter (where role = 'user') as current_user_sequence
                from conversation_messages
                group by flow_run_id
            ),
            turn_messages as (
                select
                    messages.id,
                    messages.role,
                    messages.content,
                    messages.sequence,
                    messages.created_at,
                    messages.occurred_at
                from conversation_messages messages
                join current_turn_boundaries boundaries
                  on boundaries.flow_run_id = messages.flow_run_id
                where boundaries.current_user_sequence is not null
                  and (
                      (messages.role = 'user'
                       and messages.sequence = boundaries.current_user_sequence)
                      or
                      (messages.role = 'assistant'
                       and messages.sequence > boundaries.current_user_sequence)
                  )
            )
            select role, content, sequence
            from (
                select
                    id,
                    role,
                    content,
                    sequence,
                    created_at,
                    occurred_at
                from turn_messages
                order by
                    occurred_at desc,
                    sequence desc,
                    created_at desc,
                    id desc
                limit $5
            ) recent
            order by occurred_at asc, sequence asc, created_at asc, id asc
            "#,
            hidden_internal_run_filter = hidden_internal_run_filter
        ))
        .bind(input.application_id)
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ApplicationPublicConversationMessageRecord {
                role: row.get("role"),
                content: row.get("content"),
                sequence: row.get("sequence"),
            })
            .collect())
    }
}
