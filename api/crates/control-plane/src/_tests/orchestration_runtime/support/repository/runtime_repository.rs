mod billing_state;
mod callback_state;
mod context_state;
mod flow_state;
mod query_state;
mod resume_state;
mod side_effect_state;

use super::runtime_repository_helpers::{
    flow_run_record_from_create_input, flow_run_shell_record_from_input,
    force_status_before_next_flow_update, force_stream_terminal_failure_before_next_flow_update,
    node_run_record_from_create_input,
};
use super::*;
use crate::ports::{
    CommitFlowRunTerminalInput, CommitFlowRunTerminalReceipt, CommitFlowRunTerminalResult,
    FinalizePublishedRunMissingStreamTerminalPersistenceInput,
    FinalizePublishedRunMissingStreamTerminalPersistenceOutcome,
};

use async_trait::async_trait;

#[async_trait]
impl OrchestrationRuntimeRepository for InMemoryOrchestrationRuntimeRepository {
    async fn observe_internal_compaction(
        &self,
        input: &control_plane_contracts::ports::ObserveInternalCompactionInput,
    ) -> Result<control_plane_contracts::ports::InternalCompactionHead> {
        self.memory_observe_internal_compaction(input)
    }

    async fn commit_internal_compaction(
        &self,
        input: &control_plane_contracts::ports::CommitInternalCompactionInput,
    ) -> Result<Option<Uuid>> {
        self.memory_commit_internal_compaction(input)
    }

    async fn get_flow_run_node_usages(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<control_plane_contracts::application_public_runtime::PublishedRunNodeUsage>>
    {
        self.memory_get_flow_run_node_usages(flow_run_id)
    }
    async fn upsert_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> Result<domain::CompiledPlanRecord> {
        self.memory_upsert_compiled_plan(input)
    }

    async fn get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        self.memory_get_compiled_plan(compiled_plan_id)
    }

    async fn create_flow_run(&self, input: &CreateFlowRunInput) -> Result<domain::FlowRunRecord> {
        self.memory_create_flow_run(input)
    }

    async fn create_flow_run_shell(
        &self,
        input: &crate::ports::CreateFlowRunShellInput,
    ) -> Result<domain::FlowRunRecord> {
        self.memory_create_flow_run_shell(input)
    }

    async fn attach_compiled_plan_to_flow_run(
        &self,
        input: &crate::ports::AttachCompiledPlanToFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        self.memory_attach_compiled_plan_to_flow_run(input)
    }

    async fn fail_queued_flow_run_shell(
        &self,
        input: &crate::ports::FailQueuedFlowRunShellInput,
    ) -> Result<Option<domain::FlowRunRecord>> {
        self.memory_fail_queued_flow_run_shell(input)
    }

    async fn get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        self.memory_get_flow_run(application_id, flow_run_id)
    }

    async fn create_node_run(&self, input: &CreateNodeRunInput) -> Result<domain::NodeRunRecord> {
        self.memory_create_node_run(input)
    }

    async fn update_node_run(&self, input: &UpdateNodeRunInput) -> Result<domain::NodeRunRecord> {
        self.memory_update_node_run(input)
    }

    async fn complete_node_run(
        &self,
        input: &CompleteNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
        self.update_node_run(&UpdateNodeRunInput {
            node_run_id: input.node_run_id,
            status: input.status,
            output_payload: input.output_payload.clone(),
            error_payload: input.error_payload.clone(),
            metrics_payload: input.metrics_payload.clone(),
            debug_payload: input.debug_payload.clone(),
            finished_at: Some(input.finished_at),
        })
        .await
    }

    async fn update_flow_run(&self, input: &UpdateFlowRunInput) -> Result<domain::FlowRunRecord> {
        self.memory_update_flow_run(input)
    }

    async fn update_flow_run_if_status(
        &self,
        input: &UpdateFlowRunInput,
        expected_status: domain::FlowRunStatus,
    ) -> Result<Option<domain::FlowRunRecord>> {
        self.memory_update_flow_run_if_status(input, expected_status)
    }

    async fn commit_flow_run_terminal(
        &self,
        input: &CommitFlowRunTerminalInput,
    ) -> Result<CommitFlowRunTerminalReceipt> {
        self.memory_commit_flow_run_terminal(input)
    }

    async fn finalize_published_run_missing_stream_terminal(
        &self,
        input: &FinalizePublishedRunMissingStreamTerminalPersistenceInput,
    ) -> Result<FinalizePublishedRunMissingStreamTerminalPersistenceOutcome> {
        let receipt = self
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: input.flow_run_id,
                expected_status: input.expected_status,
                result: CommitFlowRunTerminalResult::Failed {
                    output_payload: input.output_payload.clone(),
                    error_payload: input.error_payload.clone(),
                },
                flow_run_event_payload: input.error_payload.clone(),
                terminal_event_payload: input.terminal_event_payload.clone(),
                finished_at: input.finished_at,
            })
            .await?;
        Ok(match receipt {
            CommitFlowRunTerminalReceipt::Winner(flow_run) => {
                FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::Finalized(flow_run)
            }
            CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(flow_run) => {
                FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::FinalizedWithPostCommitProjectionWarning(flow_run)
            }
            CommitFlowRunTerminalReceipt::Loser => {
                FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::CasMiss
            }
        })
    }

    async fn complete_flow_run(
        &self,
        input: &CompleteFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        self.update_flow_run(&UpdateFlowRunInput {
            flow_run_id: input.flow_run_id,
            status: input.status,
            output_payload: input.output_payload.clone(),
            error_payload: input.error_payload.clone(),
            finished_at: Some(input.finished_at),
        })
        .await
    }

    async fn get_checkpoint(
        &self,
        flow_run_id: Uuid,
        checkpoint_id: Uuid,
    ) -> Result<Option<domain::CheckpointRecord>> {
        self.memory_get_checkpoint(flow_run_id, checkpoint_id)
    }

    async fn create_checkpoint(
        &self,
        input: &CreateCheckpointInput,
    ) -> Result<domain::CheckpointRecord> {
        self.memory_create_checkpoint(input)
    }

    async fn create_callback_task(
        &self,
        input: &CreateCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        self.memory_create_callback_task(input)
    }

    async fn complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        self.memory_complete_callback_task(input)
    }

    async fn get_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        self.memory_get_callback_task(callback_task_id)
    }

    async fn get_callback_resume_context(
        &self,
        application_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<CallbackResumeContext>> {
        self.memory_get_callback_resume_context(application_id, callback_task_id)
    }

    async fn upsert_debug_variable_cache_entry(
        &self,
        input: &UpsertDebugVariableCacheEntryInput,
    ) -> Result<DebugVariableCacheEntry> {
        self.memory_upsert_debug_variable_cache_entry(input)
    }

    async fn list_debug_variable_cache_entries(
        &self,
        application_id: Uuid,
        draft_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<Vec<DebugVariableCacheEntry>> {
        self.memory_list_debug_variable_cache_entries(application_id, draft_id, actor_user_id)
    }

    async fn delete_debug_variable_cache_entries(
        &self,
        input: &DeleteDebugVariableCacheEntriesInput,
    ) -> Result<()> {
        self.memory_delete_debug_variable_cache_entries(input)
    }

    async fn get_data_model_side_effect_receipt(
        &self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<domain::DataModelSideEffectReceiptRecord>> {
        self.memory_get_data_model_side_effect_receipt(workspace_id, idempotency_key)
    }

    async fn claim_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<DataModelSideEffectReceiptClaim> {
        self.memory_claim_data_model_side_effect_receipt(input)
    }

    async fn upsert_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<domain::DataModelSideEffectReceiptRecord> {
        self.memory_upsert_data_model_side_effect_receipt(input)
    }

    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        self.memory_append_run_event(input)
    }

    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> Result<domain::RuntimeSpanRecord> {
        self.memory_append_runtime_span(input)
    }

    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> Result<domain::RuntimeEventRecord> {
        if input.event_type == crate::ports::LlmInputSelection::EVENT_TYPE {
            let gate = self.inner.lock().unwrap().llm_selection_append_gate.take();
            if let Some((entered, release, fail)) = gate {
                entered.notify_one();
                release.notified().await;
                if fail {
                    return Err(anyhow::anyhow!("simulated selection commit failure"));
                }
            }
        }
        let gate = self
            .inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .next_runtime_event_append_gate
            .take();
        if let Some((entered, release)) = gate {
            entered.notify_one();
            release.notified().await;
        }
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        if std::mem::take(&mut inner.fail_next_runtime_event_append) {
            return Err(anyhow::anyhow!("simulated runtime event append failure"));
        }
        let events = inner
            .runtime_events_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default();
        let event = domain::RuntimeEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            span_id: input.span_id,
            parent_span_id: input.parent_span_id,
            sequence: (events.len() + 1) as i64,
            event_type: input.event_type.clone(),
            layer: input.layer,
            source: input.source,
            trust_level: input.trust_level,
            item_id: input.item_id,
            ledger_ref: input.ledger_ref.clone(),
            payload: input.payload.clone(),
            visibility: input.visibility,
            durability: input.durability,
            created_at: OffsetDateTime::now_utc(),
        };
        events.push(event.clone());
        Ok(event)
    }

    async fn claim_runtime_event_deliveries(
        &self,
        input: &crate::ports::ClaimRuntimeEventDeliveriesInput,
    ) -> Result<Vec<crate::ports::RuntimeEventDeliveryClaim>> {
        self.memory_claim_runtime_event_deliveries(input)
    }

    async fn ack_runtime_event_delivery(
        &self,
        input: &crate::ports::AckRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        self.memory_ack_runtime_event_delivery(input)
    }

    async fn release_runtime_event_delivery(
        &self,
        input: &crate::ports::ReleaseRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        self.memory_release_runtime_event_delivery(input)
    }

    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> Result<domain::RuntimeItemRecord> {
        self.memory_append_runtime_item(input)
    }

    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> Result<domain::ContextProjectionRecord> {
        self.memory_append_context_projection(input)
    }

    async fn put_canonical_runtime_content(
        &self,
        input: &crate::ports::PutCanonicalRuntimeContentInput,
    ) -> Result<domain::CanonicalRuntimeContentRecord> {
        self.memory_put_canonical_runtime_content(input)
    }

    async fn append_context_version(
        &self,
        input: &crate::ports::AppendContextVersionInput,
    ) -> Result<domain::ContextVersionRecord> {
        self.memory_append_context_version(input)
    }

    async fn bind_invocation_context(
        &self,
        input: &crate::ports::BindInvocationContextInput,
    ) -> Result<domain::InvocationContextBindingRecord> {
        self.memory_bind_invocation_context(input)
    }

    async fn append_provider_invocation_context(
        &self,
        input: &crate::ports::AppendProviderInvocationContextInput,
    ) -> Result<domain::ContextVersionRecord> {
        self.memory_append_provider_invocation_context(input)
    }

    async fn append_recovery_history(
        &self,
        input: &crate::ports::AppendRecoveryHistoryInput,
    ) -> Result<domain::RecoveryHistoryRecord> {
        self.memory_append_recovery_history(input)
    }

    async fn load_runtime_context_content_lineage(
        &self,
        context_version_id: Uuid,
    ) -> Result<Vec<crate::ports::RuntimeContextContentVersion>> {
        self.memory_load_runtime_context_content_lineage(context_version_id)
    }

    async fn persist_waiting_state(
        &self,
        input: &crate::ports::PersistWaitingStateInput,
    ) -> Result<Option<crate::ports::PersistedWaitingState>> {
        self.memory_persist_waiting_state(input)
    }

    async fn acquire_resume_claim(
        &self,
        input: &crate::ports::AcquireResumeClaimInput,
    ) -> Result<crate::ports::AcquireResumeClaimOutput> {
        self.memory_acquire_resume_claim(input)
    }

    async fn commit_tool_callback_results(
        &self,
        input: &crate::ports::CommitToolCallbackResultsInput,
    ) -> Result<crate::ports::CommitToolCallbackResultsOutput> {
        self.memory_commit_tool_callback_results(input)
    }

    async fn renew_resume_claim(
        &self,
        input: &crate::ports::RenewResumeClaimInput,
    ) -> Result<bool> {
        self.memory_renew_resume_claim(input)
    }

    async fn finish_resume_claim(
        &self,
        input: &crate::ports::FinishResumeClaimInput,
    ) -> Result<crate::ports::ResumeClaimRecord> {
        self.memory_finish_resume_claim(input)
    }

    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> Result<domain::UsageLedgerRecord> {
        self.memory_append_usage_ledger(input)
    }

    async fn append_cost_ledger(
        &self,
        input: &AppendCostLedgerInput,
    ) -> Result<domain::CostLedgerRecord> {
        self.memory_append_cost_ledger(input)
    }

    async fn append_credit_ledger(
        &self,
        input: &AppendCreditLedgerInput,
    ) -> Result<domain::CreditLedgerRecord> {
        self.memory_append_credit_ledger(input)
    }

    async fn append_billing_session(
        &self,
        input: &AppendBillingSessionInput,
    ) -> Result<domain::BillingSessionRecord> {
        self.memory_append_billing_session(input)
    }

    async fn model_billing_enabled_at(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Option<OffsetDateTime>> {
        self.memory_model_billing_enabled_at(_workspace_id)
    }

    async fn model_billing_match_pricing_rules(
        &self,
        provider_code: &str,
        upstream_model_id: &str,
        _at: OffsetDateTime,
    ) -> Result<Vec<crate::billing::PricingRule>> {
        self.memory_model_billing_match_pricing_rules(provider_code, upstream_model_id, _at)
    }

    async fn model_billing_admit(
        &self,
        input: &crate::ports::ModelBillingAdmissionInput,
    ) -> Result<crate::ports::ModelBillingAdmission> {
        if !self
            .model_billing_enabled_at_value()
            .is_some_and(|at| input.started_at >= at)
        {
            return Ok(crate::ports::ModelBillingAdmission::Disabled);
        }
        let prepared = input
            .reservation
            .as_ref()
            .ok_or(ControlPlaneError::Conflict(
                "model_billing_preparation_required",
            ))?;
        if !prepared.charge_enabled_default {
            return Ok(crate::ports::ModelBillingAdmission::ObserveOnly);
        }
        Ok(crate::ports::ModelBillingAdmission::Reserved(
            self.model_billing_reserve_credit(prepared).await?,
        ))
    }

    async fn record_model_observation(
        &self,
        input: &crate::ports::RecordModelObservationInput,
    ) -> Result<crate::ports::RecordedModelObservation> {
        let usage = self.append_usage_ledger(&input.usage).await?;
        let mut cost_input = input.cost.clone();
        cost_input.usage_ledger_id = Some(usage.id);
        cost_input.billing_session_id = None;
        let cost = self.append_cost_ledger(&cost_input).await?;
        Ok(crate::ports::RecordedModelObservation { usage, cost })
    }

    async fn model_billing_reserve_credit(
        &self,
        input: &crate::ports::ReserveCreditInput,
    ) -> Result<crate::ports::CreditReservation> {
        self.memory_model_billing_reserve_credit(input)
    }

    async fn model_billing_release_credit(
        &self,
        billing_session_id: Uuid,
        reason: &str,
    ) -> Result<Option<crate::ports::CreditTransactionRecord>> {
        self.memory_model_billing_release_credit(billing_session_id, reason)
    }

    async fn model_billing_heartbeat_credit_reservation(
        &self,
        _billing_session_id: Uuid,
        _reservation_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        self.memory_model_billing_heartbeat_credit_reservation(
            _billing_session_id,
            _reservation_expires_at,
        )
    }

    async fn finalize_model_billing(
        &self,
        input: &crate::ports::FinalizeModelBillingInput,
    ) -> Result<crate::ports::FinalizedModelBilling> {
        self.record_model_billing_finalize_attempt();
        if !self
            .inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .model_billing_finalize_succeeds
        {
            anyhow::bail!("test finalize stub only records settlement attempts")
        }
        let usage = self.append_usage_ledger(&input.usage).await?;
        let mut cost_input = input.cost.clone();
        cost_input.usage_ledger_id = Some(usage.id);
        cost_input.billing_session_id = Some(input.settlement.billing_session_id);
        let cost = self.append_cost_ledger(&cost_input).await?;
        Ok(crate::ports::FinalizedModelBilling {
            usage,
            cost,
            credit: crate::ports::CreditTransactionRecord {
                id: Uuid::now_v7(),
                transaction_id: Uuid::now_v7(),
                account_id: Uuid::nil(),
                workspace_id: cost_input.workspace_id,
                user_id: Uuid::nil(),
                billing_session_id: Some(input.settlement.billing_session_id),
                actor_user_id: None,
                actor_plugin_id: None,
                transaction_type: "settle".to_string(),
                amount: input.settlement.actual_amount.clone(),
                balance_after: "0".to_string(),
                reserved_after: "0".to_string(),
                credit_unit: "credit".to_string(),
                reason: "test_finalize".to_string(),
                source_type: None,
                source_id: None,
                idempotency_key: input.settlement.billing_session_id.to_string(),
                status: "posted".to_string(),
                metadata: json!({}),
                created_at: OffsetDateTime::now_utc(),
            },
        })
    }

    async fn append_audit_hash(
        &self,
        flow_run_id: Uuid,
        fact_table: &str,
        fact_id: Uuid,
        payload: serde_json::Value,
    ) -> Result<domain::AuditHashRecord> {
        self.memory_append_audit_hash(flow_run_id, fact_table, fact_id, payload)
    }

    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        self.memory_append_model_failover_attempt_ledger(input)
    }

    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        self.memory_link_usage_ledger_to_model_failover_attempt(input)
    }

    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> Result<domain::CapabilityInvocationRecord> {
        self.memory_append_capability_invocation(input)
    }

    async fn list_runtime_spans(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeSpanRecord>> {
        self.memory_list_runtime_spans(flow_run_id)
    }

    async fn list_runtime_events(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        self.memory_list_runtime_events(flow_run_id, after_sequence)
    }

    async fn get_runtime_event_sequence_for_callback_task(
        &self,
        flow_run_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<i64>> {
        self.memory_get_runtime_event_sequence_for_callback_task(flow_run_id, callback_task_id)
    }

    async fn list_runtime_items(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeItemRecord>> {
        self.memory_list_runtime_items(flow_run_id)
    }

    async fn list_context_projections(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ContextProjectionRecord>> {
        self.memory_list_context_projections(flow_run_id)
    }

    async fn list_usage_ledger(&self, flow_run_id: Uuid) -> Result<Vec<domain::UsageLedgerRecord>> {
        self.memory_list_usage_ledger(flow_run_id)
    }

    async fn list_model_failover_attempt_ledger(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverAttemptLedgerRecord>> {
        self.memory_list_model_failover_attempt_ledger(flow_run_id)
    }

    async fn list_capability_invocations(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::CapabilityInvocationRecord>> {
        self.memory_list_capability_invocations(flow_run_id)
    }

    async fn list_application_runs(
        &self,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunSummary>> {
        self.memory_list_application_runs(application_id)
    }

    async fn list_application_runs_page(
        &self,
        application_id: Uuid,
        input: control_plane::ports::ListApplicationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunSummaryPage> {
        let page = input.page.max(1);
        let page_size = input.page_size.clamp(1, 100);
        let offset = ((page - 1) * page_size) as usize;
        let mut runs = self.list_application_runs(application_id).await?;
        if let Some(created_after) = input.created_after {
            runs.retain(|run| run.created_at >= created_after);
        }
        runs.sort_by(|left, right| {
            let sort_by = input
                .sort_by
                .as_deref()
                .unwrap_or("created_at")
                .to_ascii_lowercase();
            let sort_order = input
                .sort_order
                .as_deref()
                .unwrap_or("desc")
                .to_ascii_lowercase();
            let sort_by = sort_by.as_str();
            let sort_order = sort_order.as_str();

            let order = match sort_order {
                "asc" => std::cmp::Ordering::Less,
                "desc" => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Greater,
            };
            let field_order = match sort_by {
                "started_at" => match order {
                    std::cmp::Ordering::Less => left.started_at.cmp(&right.started_at),
                    std::cmp::Ordering::Greater => right.started_at.cmp(&left.started_at),
                    _ => std::cmp::Ordering::Equal,
                },
                "finished_at" => match order {
                    std::cmp::Ordering::Less => left.finished_at.cmp(&right.finished_at),
                    std::cmp::Ordering::Greater => right.finished_at.cmp(&left.finished_at),
                    _ => std::cmp::Ordering::Equal,
                },
                "updated_at" => match order {
                    std::cmp::Ordering::Less => left.updated_at.cmp(&right.updated_at),
                    std::cmp::Ordering::Greater => right.updated_at.cmp(&left.updated_at),
                    _ => std::cmp::Ordering::Equal,
                },
                _ => match order {
                    std::cmp::Ordering::Less => left.created_at.cmp(&right.created_at),
                    std::cmp::Ordering::Greater => right.created_at.cmp(&left.created_at),
                    _ => std::cmp::Ordering::Equal,
                },
            };

            if field_order == std::cmp::Ordering::Equal {
                match order {
                    std::cmp::Ordering::Less => left.id.cmp(&right.id),
                    std::cmp::Ordering::Greater => right.id.cmp(&left.id),
                    _ => std::cmp::Ordering::Equal,
                }
            } else {
                field_order
            }
        });
        let total = runs.len() as i64;
        let items = runs
            .drain(offset.min(runs.len())..)
            .take(page_size as usize)
            .collect::<Vec<_>>();

        Ok(control_plane::ports::ApplicationRunSummaryPage {
            items,
            total,
            page,
            page_size,
        })
    }

    async fn get_application_run_detail(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunDetail>> {
        self.memory_get_application_run_detail(application_id, flow_run_id)
    }

    async fn get_latest_node_run(
        &self,
        application_id: Uuid,
        node_id: &str,
    ) -> Result<Option<domain::NodeLastRun>> {
        self.memory_get_latest_node_run(application_id, node_id)
    }
}

fn test_model_pricing_rule(
    provider_code: &str,
    upstream_model_id: &str,
) -> crate::billing::PricingRule {
    let now = OffsetDateTime::now_utc();
    crate::billing::PricingRule {
        id: Uuid::now_v7(),
        provider_code: provider_code.to_string(),
        upstream_model_id: upstream_model_id.to_string(),
        input_token_unit_size: 1000,
        input_token_unit_price: rust_decimal::Decimal::new(1, 3),
        output_token_unit_size: 1000,
        output_token_unit_price: rust_decimal::Decimal::new(2, 3),
        cache_hit_token_unit_size: 1000,
        cache_hit_token_unit_price: rust_decimal::Decimal::new(1, 4),
        cache_write_token_unit_size: 1_000_000,
        cache_write_token_unit_price: rust_decimal::Decimal::ONE,
        currency_code: "USD".to_string(),
        effective_from: now - time::Duration::days(1),
        effective_to: None,
        timezone: "UTC".to_string(),
        weekday_mask: 127,
        local_time_start: None,
        local_time_end: None,
        priority: 0,
        enabled: true,
        rules: serde_json::json!([]),
        source_kind: "manual".to_string(),
        source_catalog_id: None,
        source_version: None,
        source_checksum: None,
        extensions: serde_json::Value::Null,
        created_by: None,
        created_at: now,
        updated_at: now,
    }
}
