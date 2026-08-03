use super::*;

#[async_trait]
pub trait OrchestrationRuntimeRepository: Send + Sync {
    async fn upsert_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> anyhow::Result<domain::CompiledPlanRecord>;
    async fn get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> anyhow::Result<Option<domain::CompiledPlanRecord>>;
    async fn create_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> anyhow::Result<domain::FlowRunRecord>;
    async fn create_flow_run_shell(
        &self,
        input: &CreateFlowRunShellInput,
    ) -> anyhow::Result<domain::FlowRunRecord>;
    async fn attach_compiled_plan_to_flow_run(
        &self,
        input: &AttachCompiledPlanToFlowRunInput,
    ) -> anyhow::Result<domain::FlowRunRecord>;
    async fn fail_queued_flow_run_shell(
        &self,
        input: &FailQueuedFlowRunShellInput,
    ) -> anyhow::Result<Option<domain::FlowRunRecord>>;
    async fn get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<domain::FlowRunRecord>>;
    async fn create_node_run(
        &self,
        input: &CreateNodeRunInput,
    ) -> anyhow::Result<domain::NodeRunRecord>;
    async fn update_node_run(
        &self,
        input: &UpdateNodeRunInput,
    ) -> anyhow::Result<domain::NodeRunRecord>;
    async fn complete_node_run(
        &self,
        input: &CompleteNodeRunInput,
    ) -> anyhow::Result<domain::NodeRunRecord>;
    async fn update_flow_run(
        &self,
        input: &UpdateFlowRunInput,
    ) -> anyhow::Result<domain::FlowRunRecord>;
    async fn update_flow_run_if_status(
        &self,
        input: &UpdateFlowRunInput,
        expected_status: domain::FlowRunStatus,
    ) -> anyhow::Result<Option<domain::FlowRunRecord>>;
    async fn commit_flow_run_terminal(
        &self,
        input: &CommitFlowRunTerminalInput,
    ) -> anyhow::Result<CommitFlowRunTerminalReceipt>;
    async fn finalize_published_run_missing_stream_terminal(
        &self,
        input: &FinalizePublishedRunMissingStreamTerminalPersistenceInput,
    ) -> anyhow::Result<FinalizePublishedRunMissingStreamTerminalPersistenceOutcome>;
    async fn complete_flow_run(
        &self,
        input: &CompleteFlowRunInput,
    ) -> anyhow::Result<domain::FlowRunRecord>;
    async fn get_checkpoint(
        &self,
        flow_run_id: Uuid,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<Option<domain::CheckpointRecord>>;
    async fn create_checkpoint(
        &self,
        input: &CreateCheckpointInput,
    ) -> anyhow::Result<domain::CheckpointRecord>;
    async fn create_callback_task(
        &self,
        input: &CreateCallbackTaskInput,
    ) -> anyhow::Result<domain::CallbackTaskRecord>;
    async fn get_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> anyhow::Result<Option<domain::CallbackTaskRecord>> {
        let _ = callback_task_id;
        anyhow::bail!("get_callback_task not implemented")
    }
    async fn get_callback_resume_context(
        &self,
        application_id: Uuid,
        callback_task_id: Uuid,
    ) -> anyhow::Result<Option<CallbackResumeContext>> {
        let _ = (application_id, callback_task_id);
        anyhow::bail!("get_callback_resume_context not implemented")
    }
    async fn complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> anyhow::Result<domain::CallbackTaskRecord>;
    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> anyhow::Result<domain::RunEventRecord>;
    async fn append_run_events(
        &self,
        inputs: &[AppendRunEventInput],
    ) -> anyhow::Result<Vec<domain::RunEventRecord>> {
        let mut records = Vec::with_capacity(inputs.len());
        for input in inputs {
            records.push(self.append_run_event(input).await?);
        }
        Ok(records)
    }
    async fn update_flow_run_payloads(
        &self,
        input: &UpdateFlowRunPayloadsInput,
    ) -> anyhow::Result<domain::FlowRunRecord> {
        let _ = input;
        anyhow::bail!("update_flow_run_payloads not implemented")
    }
    async fn update_node_run_payloads(
        &self,
        input: &UpdateNodeRunPayloadsInput,
    ) -> anyhow::Result<domain::NodeRunRecord> {
        let _ = input;
        anyhow::bail!("update_node_run_payloads not implemented")
    }
    async fn update_run_event_payload(
        &self,
        input: &UpdateRunEventPayloadInput,
    ) -> anyhow::Result<domain::RunEventRecord> {
        let _ = input;
        anyhow::bail!("update_run_event_payload not implemented")
    }
    async fn update_checkpoint_payloads(
        &self,
        input: &UpdateCheckpointPayloadsInput,
    ) -> anyhow::Result<domain::CheckpointRecord> {
        let _ = input;
        anyhow::bail!("update_checkpoint_payloads not implemented")
    }
    async fn update_callback_task_payloads(
        &self,
        input: &UpdateCallbackTaskPayloadsInput,
    ) -> anyhow::Result<domain::CallbackTaskRecord> {
        let _ = input;
        anyhow::bail!("update_callback_task_payloads not implemented")
    }
    async fn record_flow_run_callback_resume_attempt(
        &self,
        input: &RecordFlowRunCallbackResumeAttemptInput,
    ) -> anyhow::Result<RecordFlowRunCallbackResumeAttemptOutput> {
        let _ = input;
        anyhow::bail!("record_flow_run_callback_resume_attempt not implemented")
    }
    async fn get_flow_run_callback_resume_attempt_by_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> anyhow::Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        let _ = callback_task_id;
        anyhow::bail!("get_flow_run_callback_resume_attempt_by_callback_task not implemented")
    }
    async fn finish_flow_run_callback_resume_attempt(
        &self,
        input: &FinishFlowRunCallbackResumeAttemptInput,
    ) -> anyhow::Result<domain::FlowRunCallbackResumeAttemptRecord> {
        let _ = input;
        anyhow::bail!("finish_flow_run_callback_resume_attempt not implemented")
    }
    async fn upsert_debug_variable_cache_entry(
        &self,
        input: &UpsertDebugVariableCacheEntryInput,
    ) -> anyhow::Result<DebugVariableCacheEntry> {
        let _ = input;
        anyhow::bail!("upsert_debug_variable_cache_entry not implemented")
    }
    async fn list_debug_variable_cache_entries(
        &self,
        application_id: Uuid,
        draft_id: Uuid,
        actor_user_id: Uuid,
    ) -> anyhow::Result<Vec<DebugVariableCacheEntry>> {
        let _ = (application_id, draft_id, actor_user_id);
        anyhow::bail!("list_debug_variable_cache_entries not implemented")
    }
    async fn delete_debug_variable_cache_entries(
        &self,
        input: &DeleteDebugVariableCacheEntriesInput,
    ) -> anyhow::Result<()> {
        let _ = input;
        anyhow::bail!("delete_debug_variable_cache_entries not implemented")
    }
    async fn create_runtime_debug_artifact(
        &self,
        input: &CreateRuntimeDebugArtifactInput,
    ) -> anyhow::Result<domain::RuntimeDebugArtifactRecord> {
        let _ = input;
        anyhow::bail!("create_runtime_debug_artifact not implemented")
    }
    async fn get_runtime_debug_artifact(
        &self,
        input: &GetRuntimeDebugArtifactInput,
    ) -> anyhow::Result<Option<domain::RuntimeDebugArtifactRecord>> {
        let _ = input;
        anyhow::bail!("get_runtime_debug_artifact not implemented")
    }
    async fn get_data_model_side_effect_receipt(
        &self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<domain::DataModelSideEffectReceiptRecord>> {
        let _ = (workspace_id, idempotency_key);
        anyhow::bail!("get_data_model_side_effect_receipt not implemented")
    }
    async fn claim_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> anyhow::Result<DataModelSideEffectReceiptClaim> {
        let _ = input;
        anyhow::bail!("claim_data_model_side_effect_receipt not implemented")
    }
    async fn upsert_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> anyhow::Result<domain::DataModelSideEffectReceiptRecord> {
        let _ = input;
        anyhow::bail!("upsert_data_model_side_effect_receipt not implemented")
    }
    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> anyhow::Result<domain::RuntimeSpanRecord>;
    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> anyhow::Result<domain::RuntimeEventRecord>;
    async fn append_runtime_events(
        &self,
        inputs: &[AppendRuntimeEventInput],
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
        let mut records = Vec::with_capacity(inputs.len());
        for input in inputs {
            records.push(self.append_runtime_event(input).await?);
        }
        Ok(records)
    }
    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> anyhow::Result<domain::RuntimeItemRecord>;
    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> anyhow::Result<domain::ContextProjectionRecord>;
    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> anyhow::Result<domain::UsageLedgerRecord>;
    async fn append_cost_ledger(
        &self,
        input: &AppendCostLedgerInput,
    ) -> anyhow::Result<domain::CostLedgerRecord>;
    async fn append_credit_ledger(
        &self,
        input: &AppendCreditLedgerInput,
    ) -> anyhow::Result<domain::CreditLedgerRecord>;
    async fn append_billing_session(
        &self,
        input: &AppendBillingSessionInput,
    ) -> anyhow::Result<domain::BillingSessionRecord>;
    async fn append_audit_hash(
        &self,
        flow_run_id: Uuid,
        fact_table: &str,
        fact_id: Uuid,
        payload: serde_json::Value,
    ) -> anyhow::Result<domain::AuditHashRecord>;
    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord>;
    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord>;
    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> anyhow::Result<domain::CapabilityInvocationRecord>;
    async fn list_runtime_spans(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::RuntimeSpanRecord>>;
    async fn list_runtime_events(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>>;
    async fn get_runtime_event_sequence_for_callback_task(
        &self,
        flow_run_id: Uuid,
        callback_task_id: Uuid,
    ) -> anyhow::Result<Option<i64>>;
    async fn list_runtime_event_backfill_page(
        &self,
        flow_run_id: Uuid,
        after_stream_sequence: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
        let mut records = self
            .list_runtime_events(flow_run_id, after_stream_sequence)
            .await?;
        records.truncate(limit.max(1));
        Ok(records)
    }
    async fn list_runtime_items(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::RuntimeItemRecord>>;
    async fn list_context_projections(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ContextProjectionRecord>>;
    async fn list_usage_ledger(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::UsageLedgerRecord>>;
    async fn list_model_failover_attempt_ledger(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ModelFailoverAttemptLedgerRecord>>;
    async fn insert_model_provider_request_logs_batch(
        &self,
        records: &[ProviderRequestLogTask],
    ) -> anyhow::Result<()> {
        let _ = records;
        anyhow::bail!("insert_model_provider_request_logs_batch not implemented")
    }
    async fn list_model_provider_request_logs_page(
        &self,
        input: ListModelProviderRequestLogsPageInput,
    ) -> anyhow::Result<ModelProviderRequestLogsPage> {
        let _ = input;
        anyhow::bail!("list_model_provider_request_logs_page not implemented")
    }
    async fn delete_model_provider_request_logs(
        &self,
        input: DeleteModelProviderRequestLogsInput,
    ) -> anyhow::Result<u64> {
        let _ = input;
        anyhow::bail!("delete_model_provider_request_logs not implemented")
    }
    async fn clear_model_provider_request_logs_batch(
        &self,
        input: ClearModelProviderRequestLogsBatchInput,
    ) -> anyhow::Result<ClearModelProviderRequestLogsBatchResult> {
        let _ = input;
        anyhow::bail!("clear_model_provider_request_logs_batch not implemented")
    }
    async fn list_capability_invocations(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::CapabilityInvocationRecord>>;
    async fn list_application_runs(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ApplicationRunSummary>>;
    async fn list_application_runs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationRunsPageInput,
    ) -> anyhow::Result<ApplicationRunSummaryPage>;
    async fn list_application_run_logs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationRunsPageInput,
    ) -> anyhow::Result<ApplicationRunLogSummaryPage> {
        let _ = (application_id, input);
        anyhow::bail!("list_application_run_logs_page not implemented")
    }
    async fn list_application_run_count_tokens_results(
        &self,
        flow_run_ids: &[Uuid],
    ) -> anyhow::Result<Vec<ApplicationRunCountTokensResult>> {
        let _ = flow_run_ids;
        anyhow::bail!("list_application_run_count_tokens_results not implemented")
    }
    async fn get_application_run_monitoring_report(
        &self,
        application_id: Uuid,
        input: GetApplicationRunMonitoringReportInput,
    ) -> anyhow::Result<ApplicationRunMonitoringReport> {
        let _ = (application_id, input);
        anyhow::bail!("get_application_run_monitoring_report not implemented")
    }
    async fn list_application_conversation_runs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationConversationRunsPageInput,
    ) -> anyhow::Result<ApplicationConversationRunsPage> {
        let _ = (application_id, input);
        anyhow::bail!("list_application_conversation_runs_page not implemented")
    }
    async fn list_application_run_conversation_message_items_page(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        input: ListApplicationRunConversationMessageItemsPageInput,
    ) -> anyhow::Result<ApplicationRunConversationMessageItemsPage> {
        let _ = (application_id, flow_run_id, input);
        anyhow::bail!("list_application_run_conversation_message_items_page not implemented")
    }
    async fn get_application_run_conversation_current_item(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<domain::ApplicationRunConversationMessageItem>> {
        let _ = (application_id, flow_run_id);
        anyhow::bail!("get_application_run_conversation_current_item not implemented")
    }
    async fn get_application_run_detail(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<domain::ApplicationRunDetail>>;
    async fn get_application_run_trace_projection_source(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<domain::ApplicationRunDetail>> {
        let _ = (application_id, flow_run_id);
        anyhow::bail!("get_application_run_trace_projection_source not implemented")
    }
    async fn get_application_run_trace_projection_source_watermark(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Option<String>> {
        let _ = (application_id, flow_run_id);
        anyhow::bail!("get_application_run_trace_projection_source_watermark not implemented")
    }
    async fn replace_application_run_trace_projection(
        &self,
        input: &ReplaceApplicationRunTraceProjectionInput,
    ) -> anyhow::Result<()> {
        let _ = input;
        anyhow::bail!("replace_application_run_trace_projection not implemented")
    }
    async fn upsert_application_run_trace_projection_status(
        &self,
        input: &UpsertApplicationRunTraceProjectionStatusInput,
    ) -> anyhow::Result<()> {
        let _ = input;
        anyhow::bail!("upsert_application_run_trace_projection_status not implemented")
    }
    async fn get_application_run_trace_projection_status(
        &self,
        flow_run_id: Uuid,
        projection_version: i32,
    ) -> anyhow::Result<Option<domain::ApplicationRunTraceProjectionStatusRecord>> {
        let _ = (flow_run_id, projection_version);
        anyhow::bail!("get_application_run_trace_projection_status not implemented")
    }
    async fn list_application_run_trace_roots(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ApplicationRunTraceNodeRecord>> {
        let _ = flow_run_id;
        anyhow::bail!("list_application_run_trace_roots not implemented")
    }
    async fn get_application_run_trace_statistics(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<ApplicationRunTraceProjectionStatistics> {
        let _ = flow_run_id;
        anyhow::bail!("get_application_run_trace_statistics not implemented")
    }
    async fn list_application_run_trace_children_page(
        &self,
        input: ListApplicationRunTraceChildrenPageInput,
    ) -> anyhow::Result<ListApplicationRunTraceChildrenPage> {
        let _ = input;
        anyhow::bail!("list_application_run_trace_children_page not implemented")
    }
    async fn get_application_run_trace_node(
        &self,
        flow_run_id: Uuid,
        trace_node_id: Uuid,
    ) -> anyhow::Result<Option<domain::ApplicationRunTraceNodeRecord>> {
        let _ = (flow_run_id, trace_node_id);
        anyhow::bail!("get_application_run_trace_node not implemented")
    }
    async fn get_application_run_trace_node_by_locator(
        &self,
        flow_run_id: Uuid,
        stable_locator: &str,
    ) -> anyhow::Result<Option<domain::ApplicationRunTraceNodeRecord>> {
        let _ = (flow_run_id, stable_locator);
        anyhow::bail!("get_application_run_trace_node_by_locator not implemented")
    }
    async fn get_application_run_trace_node_content(
        &self,
        flow_run_id: Uuid,
        trace_node_id: Uuid,
    ) -> anyhow::Result<Option<domain::ApplicationRunTraceNodeContentRecord>> {
        let _ = (flow_run_id, trace_node_id);
        anyhow::bail!("get_application_run_trace_node_content not implemented")
    }
    async fn list_application_run_trace_node_run_details(
        &self,
        flow_run_id: Uuid,
        node_run_ids: Vec<Uuid>,
    ) -> anyhow::Result<Vec<domain::NodeRunRecord>> {
        let _ = (flow_run_id, node_run_ids);
        anyhow::bail!("list_application_run_trace_node_run_details not implemented")
    }
    async fn get_latest_node_run(
        &self,
        application_id: Uuid,
        node_id: &str,
    ) -> anyhow::Result<Option<domain::NodeLastRun>>;
}
