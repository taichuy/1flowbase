use super::*;

#[path = "orchestration/dispatch_lease.rs"]
mod dispatch_lease;
#[path = "orchestration/log_operations.rs"]
mod log_operations;

log_operations::with_log_operations! {
    async fn try_acquire_flow_run_dispatch_lease(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<Box<dyn control_plane_contracts::ports::FlowRunDispatchLease>>> {
        dispatch_lease::acquire(self, application_id, flow_run_id).await
    }
    async fn flow_run_has_resume_claims(&self, flow_run_id: Uuid) -> Result<bool> {
        Ok(sqlx::query_scalar("select exists(select 1 from flow_run_resume_claims where flow_run_id=$1)")
            .bind(flow_run_id).fetch_one(self.pool()).await?)
    }

    async fn get_flow_run_node_usages(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<PublishedRunNodeUsage>> {
        self.read_flow_run_node_usages(flow_run_id).await
    }
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

    async fn get_flow_run_assistant_conversation_id(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<Uuid>> {
        sqlx::query_scalar("select assistant_conversation_id from flow_runs where id = $1")
            .bind(flow_run_id)
            .fetch_optional(self.pool())
            .await
            .map(|value| value.flatten())
            .map_err(Into::into)
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

    async fn append_client_trajectory(
        &self,
        input: &control_plane_contracts::ports::AppendClientTrajectoryInput,
    ) -> Result<()> {
        self.append_client_trajectory_fact(input).await
    }
    async fn append_client_trajectory_archive(
        &self,
        input: &control_plane_contracts::ports::AppendClientTrajectoryArchiveInput,
    ) -> Result<control_plane_contracts::ports::ClientTrajectoryArchiveReceipt> {
        self.append_client_trajectory_archive_part(input).await
    }
    async fn discard_unbound_client_trajectory_archive(&self, request_id: Uuid) -> Result<()> {
        self.discard_unbound_client_trajectory_archive_parts(request_id)
            .await
    }
    async fn read_client_trajectory_archive(
        &self,
        request_id: Uuid,
        cursor: i64,
        limit: i64,
    ) -> Result<Vec<control_plane_contracts::ports::ClientTrajectoryArchiveFrame>> {
        self.read_client_trajectory_archive_frames(request_id, cursor, limit)
            .await
    }
    async fn client_trajectory_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<control_plane_contracts::ports::ClientTrajectoryPage> {
        self.read_client_trajectory_page(flow_run_id, node_run_id, cursor, limit)
            .await
    }
    async fn client_trajectory_section(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        step_id: Uuid,
        section: &str,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<Option<control_plane_contracts::ports::ClientTrajectorySection>> {
        self.read_client_trajectory_section(
            flow_run_id,
            node_run_id,
            step_id,
            section,
            cursor,
            limit,
        )
        .await
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

    async fn claim_runtime_event_deliveries(
        &self,
        input: &ClaimRuntimeEventDeliveriesInput,
    ) -> Result<Vec<RuntimeEventDeliveryClaim>> {
        PgControlPlaneStore::claim_runtime_event_deliveries(self, input).await
    }

    async fn ack_runtime_event_delivery(
        &self,
        input: &AckRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        PgControlPlaneStore::ack_runtime_event_delivery(self, input).await
    }

    async fn release_runtime_event_delivery(
        &self,
        input: &ReleaseRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        PgControlPlaneStore::release_runtime_event_delivery(self, input).await
    }

    async fn mark_runtime_event_delivery_uncertain(
        &self,
        input: &MarkRuntimeEventDeliveryUncertainInput,
    ) -> Result<domain::RuntimeEventRecord> {
        PgControlPlaneStore::mark_runtime_event_delivery_uncertain(self, input).await
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

    async fn put_canonical_runtime_content(
        &self,
        input: &PutCanonicalRuntimeContentInput,
    ) -> Result<domain::CanonicalRuntimeContentRecord> {
        PgControlPlaneStore::put_canonical_runtime_content(self, input).await
    }

    async fn append_context_version(
        &self,
        input: &AppendContextVersionInput,
    ) -> Result<domain::ContextVersionRecord> {
        PgControlPlaneStore::append_context_version(self, input).await
    }

    async fn bind_invocation_context(
        &self,
        input: &BindInvocationContextInput,
    ) -> Result<domain::InvocationContextBindingRecord> {
        PgControlPlaneStore::bind_invocation_context(self, input).await
    }

    async fn append_provider_invocation_context(
        &self,
        input: &AppendProviderInvocationContextInput,
    ) -> Result<domain::ContextVersionRecord> {
        PgControlPlaneStore::append_provider_invocation_context(self, input).await
    }

    async fn append_recovery_history(
        &self,
        input: &AppendRecoveryHistoryInput,
    ) -> Result<domain::RecoveryHistoryRecord> {
        PgControlPlaneStore::append_recovery_history(self, input).await
    }

    async fn convert_legacy_runtime_shadow_batch(
        &self,
        input: &ConvertLegacyRuntimeShadowBatchInput,
    ) -> Result<ConvertLegacyRuntimeShadowBatchResult> {
        PgControlPlaneStore::convert_legacy_runtime_shadow_batch(self, input).await
    }

    async fn rollback_legacy_runtime_shadow(
        &self,
        input: &RollbackLegacyRuntimeShadowInput,
    ) -> Result<RollbackLegacyRuntimeShadowResult> {
        PgControlPlaneStore::rollback_legacy_runtime_shadow(self, input).await
    }

    async fn load_runtime_context_content_lineage(
        &self,
        context_version_id: Uuid,
    ) -> Result<Vec<RuntimeContextContentVersion>> {
        PgControlPlaneStore::load_runtime_context_content_lineage(self, context_version_id).await
    }

    async fn persist_waiting_state(
        &self,
        input: &PersistWaitingStateInput,
    ) -> Result<Option<PersistedWaitingState>> {
        PgControlPlaneStore::persist_waiting_state(self, input).await
    }

    async fn acquire_resume_claim(
        &self,
        input: &AcquireResumeClaimInput,
    ) -> Result<AcquireResumeClaimOutput> {
        PgControlPlaneStore::acquire_resume_claim(self, input).await
    }

    async fn acquire_resume_claim_with_admission(
        &self,
        input: &AcquireResumeClaimInput,
        admission: &CallbackResumeAdmissionFence,
    ) -> Result<AcquireResumeClaimOutput> {
        PgControlPlaneStore::acquire_resume_claim_with_admission(self, input, admission).await
    }

    async fn renew_resume_claim(&self, input: &RenewResumeClaimInput) -> Result<bool> {
        PgControlPlaneStore::renew_resume_claim(self, input).await
    }

    async fn finish_resume_claim(
        &self,
        input: &FinishResumeClaimInput,
    ) -> Result<ResumeClaimRecord> {
        PgControlPlaneStore::finish_resume_claim(self, input).await
    }

    async fn commit_tool_callback_results(
        &self,
        input: &CommitToolCallbackResultsInput,
    ) -> Result<CommitToolCallbackResultsOutput> {
        PgControlPlaneStore::commit_tool_callback_results(self, input).await
    }

    async fn commit_tool_callback_results_with_admission(
        &self,
        input: &CommitToolCallbackResultsInput,
        admission: &CallbackResumeAdmissionFence,
    ) -> Result<CommitToolCallbackResultsOutput> {
        PgControlPlaneStore::commit_tool_callback_results_with_admission(self, input, admission)
            .await
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

    async fn record_model_observation(
        &self,
        input: &RecordModelObservationInput,
    ) -> Result<RecordedModelObservation> {
        PgControlPlaneStore::record_model_observation(self, input).await
    }

    async fn finalize_model_billing(
        &self,
        input: &FinalizeModelBillingInput,
    ) -> Result<FinalizedModelBilling> {
        PgControlPlaneStore::finalize_model_billing(self, input).await
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

    async fn model_billing_enabled_at(&self, workspace_id: Uuid) -> Result<Option<OffsetDateTime>> {
        BillingRepository::billing_enabled_at(self, workspace_id).await
    }

    async fn model_billing_match_pricing_rules(
        &self,
        provider_code: &str,
        upstream_model_id: &str,
        at: OffsetDateTime,
    ) -> Result<Vec<control_plane_contracts::billing::PricingRule>> {
        BillingRepository::match_pricing_rules(self, provider_code, upstream_model_id, at).await
    }

    async fn model_billing_list_pricing_rules(
        &self,
        provider_code: &str,
        upstream_model_id: &str,
    ) -> Result<Vec<control_plane_contracts::billing::PricingRule>> {
        let mut candidates = BillingRepository::list_pricing_rules(
            self,
            &ListPricingRulesInput {
                provider_code: Some(provider_code.to_string()),
                upstream_model_id: Some(upstream_model_id.to_string()),
                enabled: Some(true),
                source_kind: None,
                page_size: 500,
                offset: 0,
            },
        )
        .await
        .map(|page| page.items)?;
        candidates.retain(|rule| {
            rule.provider_code == provider_code && rule.upstream_model_id == upstream_model_id
        });
        if provider_code != control_plane_contracts::billing::GLOBAL_ZERO_PROVIDER_CODE
            || upstream_model_id != control_plane_contracts::billing::GLOBAL_ZERO_MODEL_ID
        {
            let mut fallback = BillingRepository::list_pricing_rules(
                self,
                &ListPricingRulesInput {
                    provider_code: Some(
                        control_plane_contracts::billing::GLOBAL_ZERO_PROVIDER_CODE.to_string(),
                    ),
                    upstream_model_id: Some(
                        control_plane_contracts::billing::GLOBAL_ZERO_MODEL_ID.to_string(),
                    ),
                    enabled: Some(true),
                    source_kind: None,
                    page_size: 500,
                    offset: 0,
                },
            )
            .await?
            .items;
            fallback.retain(|rule| {
                rule.provider_code == control_plane_contracts::billing::GLOBAL_ZERO_PROVIDER_CODE
                    && rule.upstream_model_id
                        == control_plane_contracts::billing::GLOBAL_ZERO_MODEL_ID
            });
            candidates.extend(fallback);
        }
        Ok(candidates)
    }

    async fn model_billing_admit(
        &self,
        input: &ModelBillingAdmissionInput,
    ) -> Result<ModelBillingAdmission> {
        self.admit_model_billing(input).await
    }

    async fn model_billing_reserve_credit(
        &self,
        input: &ReserveCreditInput,
    ) -> Result<CreditReservation> {
        BillingRepository::reserve_credit(self, input).await
    }

    async fn model_billing_settle_credit(
        &self,
        input: &SettleCreditInput,
    ) -> Result<CreditTransactionRecord> {
        BillingRepository::settle_credit(self, input).await
    }

    async fn model_billing_release_credit(
        &self,
        billing_session_id: Uuid,
        reason: &str,
    ) -> Result<Option<CreditTransactionRecord>> {
        BillingRepository::release_credit(self, billing_session_id, reason).await
    }

    async fn model_billing_heartbeat_credit_reservation(
        &self,
        billing_session_id: Uuid,
        reservation_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        BillingRepository::heartbeat_credit_reservation(
            self,
            billing_session_id,
            reservation_expires_at,
        )
        .await
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


}
