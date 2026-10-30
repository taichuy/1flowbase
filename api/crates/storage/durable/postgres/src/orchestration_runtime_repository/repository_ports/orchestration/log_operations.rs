macro_rules! with_log_operations {
    ($($operations:tt)*) => {
        #[async_trait]
        impl OrchestrationRuntimeRepository for PgControlPlaneStore {
async fn next_agent_log_pricing_record(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        after: Option<Uuid>,
    ) -> Result<Option<control_plane_contracts::ports::AgentLogPricingRecord>> {
        agent_logs::reprice::next_record(self, application_id, scope_id, after).await
    }
    async fn reprice_agent_log_record(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        record_id: Uuid,
        costs: &[(String, Option<String>)],
    ) -> Result<u64> {
        agent_logs::reprice::apply(self, application_id, scope_id, record_id, costs).await
    }
    async fn preview_agent_logs_delete(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        scope: &control_plane_contracts::ports::AgentLogsDeleteScope,
    ) -> Result<control_plane_contracts::ports::AgentLogsDeletePreview> {
        agent_logs::deletion_jobs::preview(self, application_id, scope_id, scope).await
    }
    async fn create_agent_logs_delete_job(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        input: &control_plane_contracts::ports::AgentLogsDeleteJobCreate,
    ) -> Result<control_plane_contracts::ports::AgentLogsDeleteJob> {
        agent_logs::deletion_jobs::create(self, application_id, scope_id, input).await
    }
    async fn get_agent_logs_delete_job(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        job_id: Option<Uuid>,
    ) -> Result<Option<control_plane_contracts::ports::AgentLogsDeleteJob>> {
        agent_logs::deletion_jobs::get(self, application_id, scope_id, job_id).await
    }
    async fn stop_agent_logs_delete_job(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        job_id: Uuid,
    ) -> Result<Option<control_plane_contracts::ports::AgentLogsDeleteJob>> {
        agent_logs::deletion_jobs::stop(self, application_id, scope_id, job_id).await
    }
    async fn next_agent_logs_delete_job(
        &self,
    ) -> Result<Option<control_plane_contracts::ports::AgentLogsDeleteJob>> {
        agent_logs::deletion_jobs::next(self).await
    }
    async fn advance_agent_logs_delete_job(&self, job_id: Uuid) -> Result<()> {
        agent_logs::deletion_jobs::advance(self, job_id).await
    }
    async fn fail_agent_logs_delete_job(
        &self,
        job_id: Uuid,
        expected_deleted_records: u64,
    ) -> Result<()> {
        agent_logs::deletion_jobs::fail(self, job_id, expected_deleted_records).await
    }
    async fn delete_agent_logs(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        scope: &control_plane_contracts::ports::AgentLogsDeleteScope,
    ) -> Result<control_plane_contracts::ports::AgentLogsDeleteReceipt> {
        agent_logs_delete::delete(self, application_id, scope_id, scope).await
    }
    async fn ingest_agent_logs(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        api_key_id: Option<Uuid>,
        batch: &control_plane_contracts::ports::AgentLogsBatch,
        costs: &[Option<String>],
    ) -> Result<control_plane_contracts::ports::AgentLogsReceipt> {
        agent_logs::ingest(self, application_id, scope_id, api_key_id, batch, costs).await
    }
    async fn read_imported_agent_logs(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        record_ids: &[Uuid],
    ) -> Result<Vec<control_plane_contracts::ports::AgentLogsBatch>> {
        agent_logs::reader::read(self, application_id, scope_id, record_ids).await
    }
    async fn read_imported_agent_logs_selection(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        selector: &control_plane_contracts::ports::AgentLogsExportSelector,
    ) -> Result<control_plane_contracts::ports::AgentLogsExportFacts> {
        agent_logs::reader::read_selection(self, application_id, scope_id, selector).await
    }
    async fn read_native_agent_log_messages(
        &self,
        scope_id: Uuid,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<Vec<Value>> {
        // Keep the restored original as JSON: JSONB rejects valid NUL characters.
        let rows = sqlx::query(r#"
            select json_build_object('id',m.id,'source_item_key',m.source_item_key,
                'source_kind',m.source_kind,'role',m.role,'started_at',m.started_at,
                'display_sequence',m.display_sequence,
                'native_message',runtime_original_json(m.native_message,m.raw_json_payloads,'native_message')) as native_unit,
                ('v'||progress.projection_version::text||':'||progress.source_revision::text) as published_revision
            from application_run_conversation_message_items m
            left join application_run_native_projection_progress progress on progress.flow_run_id=m.flow_run_id
                and progress.projection_version=m.projection_version
            where m.scope_id=$1 and m.application_id=$2 and m.flow_run_id=$3
            order by m.display_sequence,m.id
        "#).bind(scope_id).bind(application_id).bind(run_id).fetch_all(self.pool()).await?;
        rows.into_iter().map(|row| {
            let mut unit: Value = row.try_get("native_unit")?;
            apply_published_native_revision(&mut unit["native_message"], row.try_get("published_revision")?);
            Ok(unit)
        }).collect()
    }
    async fn query_application_log_records(
        &self,
        scope_id: Uuid,
        application_ids: &[Uuid],
        query: &control_plane_contracts::ports::ApplicationLogRecordsQuery,
    ) -> Result<control_plane_contracts::ports::ApplicationLogRecordsPage> {
        log_query::records(self, scope_id, application_ids, query).await
    }
    async fn query_conversation_client_trajectory(
        &self,
        application_id: Uuid,
        log_conversation_id: Uuid,
        query: &control_plane_contracts::ports::ConversationClientTrajectoryQuery,
    ) -> Result<control_plane_contracts::ports::ConversationClientTrajectoryQueryPage> {
        log_query::conversation_trajectory(self, application_id, log_conversation_id, query).await
    }
    async fn read_conversation_client_trajectory_section(
        &self,
        application_id: Uuid,
        log_conversation_id: Uuid,
        step_id: Uuid,
        section: &str,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<Option<control_plane_contracts::ports::ClientTrajectorySection>> {
        log_query::conversation_section(
            self,
            application_id,
            log_conversation_id,
            step_id,
            section,
            cursor,
            limit,
        )
        .await
    }
    async fn query_record_client_trajectory(
        &self,
        application_id: Uuid,
        record_id: Uuid,
        query: &control_plane_contracts::ports::RecordClientTrajectoryQuery,
    ) -> Result<control_plane_contracts::ports::RecordClientTrajectoryQueryPage> {
        log_query::trajectory(self, application_id, record_id, query).await
    }
    async fn application_log_record_conversation_page(
        &self,
        application_id: Uuid,
        record_id: Uuid,
        before: Option<Uuid>,
        after: Option<Uuid>,
        limit: i64,
    ) -> Result<control_plane_contracts::ports::ApplicationLogRecordConversationPage> {
        agent_logs::conversation::page(self, application_id, record_id, before, after, limit).await
    }

    async fn query_application_log_conversations(
        &self,
        scope_id: Uuid,
        application_ids: &[Uuid],
        query: &control_plane_contracts::ports::ApplicationLogConversationsQuery,
    ) -> Result<control_plane_contracts::ports::ApplicationLogConversationsPage> {
        log_query::conversations(self, scope_id, application_ids, query).await
    }
    async fn application_log_record_trace_tree(
        &self,
        application_id: Uuid,
        record_id: Uuid,
    ) -> Result<control_plane_contracts::ports::ApplicationLogRecordTraceTree> {
        agent_logs::trace::tree(self, application_id, record_id).await
    }
    async fn application_log_record_trace_children(
        &self,
        application_id: Uuid,
        record_id: Uuid,
        trace_node_id: Uuid,
        cursor: Option<String>,
        page_size: i64,
    ) -> Result<control_plane_contracts::ports::ApplicationLogRecordTraceChildren> {
        agent_logs::trace::children(
            self,
            application_id,
            record_id,
            trace_node_id,
            cursor,
            page_size,
        )
        .await
    }
    async fn application_log_record(
        &self,
        application_id: Uuid,
        record_id: Uuid,
    ) -> Result<Option<control_plane_contracts::ports::ApplicationLogRecordOverview>> {
        agent_logs::overview(self, application_id, record_id).await
    }
    async fn record_client_trajectory_page(
        &self,
        application_id: Uuid,
        record_id: Uuid,
        cursor: Option<String>,
        limit: i64,
    ) -> Result<control_plane_contracts::ports::RecordClientTrajectoryPage> {
        agent_logs::page(self, application_id, record_id, cursor, limit).await
    }
    async fn record_client_trajectory_section(
        &self,
        application_id: Uuid,
        record_id: Uuid,
        step_id: Uuid,
        section: &str,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<Option<control_plane_contracts::ports::ClientTrajectorySection>> {
        agent_logs::section(
            self,
            application_id,
            record_id,
            step_id,
            section,
            cursor,
            limit,
        )
        .await
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

    async fn get_original_callback_waiting_boundary(
        &self,
        flow_run_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<RuntimeCallbackWaitingBoundary>> {
        PgControlPlaneStore::get_original_callback_waiting_boundary(self, flow_run_id, callback_task_id).await
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

    async fn list_runtime_event_durable_page(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
        through_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        PgControlPlaneStore::list_runtime_event_durable_page(
            self,
            flow_run_id,
            after_sequence,
            through_sequence,
            limit,
        )
        .await
    }

    async fn get_runtime_event_replay_window(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<RuntimeEventReplayWindow>> {
        PgControlPlaneStore::get_runtime_event_replay_window(self, flow_run_id).await
    }

    async fn has_runtime_event_terminal_after(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
        event_type: &str,
        generation_id: Uuid,
    ) -> Result<bool> {
        PgControlPlaneStore::has_runtime_event_terminal_after(
            self,
            flow_run_id,
            after_sequence,
            event_type,
            generation_id,
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
        records: &[control_plane_contracts::ports::ProviderRequestLogTask],
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
        input: control_plane_contracts::ports::ListApplicationRunsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationRunSummaryPage> {
        PgControlPlaneStore::list_application_runs_page(self, application_id, input).await
    }

    async fn expand_application_run_log_tasks(
        &self,
        application_id: Uuid,
        flow_run_ids: &[Uuid],
    ) -> Result<Vec<Uuid>> {
        PgControlPlaneStore::expand_application_run_log_tasks(self, application_id, flow_run_ids)
            .await
    }

    async fn list_application_run_logs_page(
        &self,
        application_id: Uuid,
        input: control_plane_contracts::ports::ListApplicationRunsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationRunLogSummaryPage> {
        PgControlPlaneStore::list_application_run_logs_page(self, application_id, input).await
    }

    async fn get_application_run_log_task(
        &self,
        application_id: Uuid,
        task_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunLogTask>> {
        PgControlPlaneStore::get_application_run_log_task(self, application_id, task_run_id).await
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
    ) -> Result<control_plane_contracts::ports::ApplicationRunMonitoringReport> {
        PgControlPlaneStore::get_application_run_monitoring_report(self, application_id, input)
            .await
    }

    async fn list_application_conversation_runs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationConversationRunsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationConversationRunsPage> {
        PgControlPlaneStore::list_application_conversation_runs_page(self, application_id, input)
            .await
    }

    async fn list_application_run_conversation_message_items_page(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        input: ListApplicationRunConversationMessageItemsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationRunConversationMessageItemsPage> {
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

    async fn get_flow_run_metadata(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<control_plane_contracts::ports::FlowRunMetadataReadModel>> {
        PgControlPlaneStore::get_flow_run_metadata(self, application_id, flow_run_id).await
    }

    async fn get_application_run_overview(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<ApplicationRunOverviewReadModel>> {
        PgControlPlaneStore::get_application_run_overview(self, application_id, flow_run_id).await
    }

    async fn get_application_run_resume_timeline(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<ApplicationRunResumeTimelineReadModel>> {
        PgControlPlaneStore::get_application_run_resume_timeline(self, application_id, flow_run_id)
            .await
    }

    async fn get_application_run_resume_timeline_summary(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<ApplicationRunResumeTimelineSummaryReadModel>> {
        PgControlPlaneStore::get_application_run_resume_timeline_summary(
            self,
            application_id,
            flow_run_id,
        )
        .await
    }

    async fn list_application_run_trace_checkpoints(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        node_run_ids: Vec<Uuid>,
    ) -> Result<Vec<domain::CheckpointRecord>> {
        PgControlPlaneStore::list_application_run_trace_checkpoints(
            self,
            application_id,
            flow_run_id,
            node_run_ids,
        )
        .await
    }

    async fn list_application_run_trace_events(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        node_run_ids: Vec<Uuid>,
    ) -> Result<Vec<domain::RunEventRecord>> {
        PgControlPlaneStore::list_application_run_trace_events(
            self,
            application_id,
            flow_run_id,
            node_run_ids,
        )
        .await
    }

    async fn get_application_run_trace_projection_source(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunTraceProjectionSource>> {
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

    async fn settle_next_application_log_projection(&self) -> Result<bool> {
        sqlx::query_scalar("select settle_next_application_log_projection()")
            .fetch_one(self.pool())
            .await
            .map_err(Into::into)
    }

    async fn claim_application_run_trace_refresh(
        &self,
    ) -> Result<Option<ApplicationRunTraceRefreshJob>> {
        PgControlPlaneStore::claim_application_run_trace_refresh(self).await
    }
    async fn finish_application_run_trace_refresh(
        &self,
        job: &ApplicationRunTraceRefreshJob,
        succeeded: bool,
    ) -> Result<()> {
        PgControlPlaneStore::finish_application_run_trace_refresh(self, job, succeeded).await
    }
    async fn get_application_run_trace_read_status(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        projection_version: i32,
    ) -> Result<Option<domain::ApplicationRunTraceProjectionStatusRecord>> {
        PgControlPlaneStore::get_application_run_trace_read_status(
            self,
            application_id,
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

    async fn list_application_run_trace_node_run_sections(
        &self,
        flow_run_id: Uuid,
        node_run_ids: Vec<Uuid>,
        section: &str,
    ) -> Result<Vec<domain::NodeRunRecord>> {
        PgControlPlaneStore::list_application_run_trace_node_run_sections(
            self,
            flow_run_id,
            node_run_ids,
            section,
        )
        .await
    }
    async fn list_trace_enrichment_events(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        PgControlPlaneStore::list_trace_enrichment_events(self, flow_run_id).await
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

$($operations)*
}
};
}
pub(super) use with_log_operations;
