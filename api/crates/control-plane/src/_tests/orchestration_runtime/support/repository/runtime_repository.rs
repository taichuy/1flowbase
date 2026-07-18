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
    async fn upsert_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> Result<domain::CompiledPlanRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let record = domain::CompiledPlanRecord {
            id: Uuid::now_v7(),
            flow_id: input.flow_id,
            draft_id: input.flow_draft_id,
            schema_version: input.schema_version.clone(),
            document_hash: input.document_hash.clone(),
            document_updated_at: input.document_updated_at,
            plan: input.plan.clone(),
            created_by: input.actor_user_id,
            created_at: now,
            updated_at: now,
        };
        inner.compiled_plans_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.compiled_plans_by_id.get(&compiled_plan_id).cloned())
    }

    async fn create_flow_run(&self, input: &CreateFlowRunInput) -> Result<domain::FlowRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = flow_run_record_from_create_input(input);
        inner.flow_runs_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn create_flow_run_shell(
        &self,
        input: &crate::ports::CreateFlowRunShellInput,
    ) -> Result<domain::FlowRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = flow_run_shell_record_from_input(input);
        inner.flow_runs_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn attach_compiled_plan_to_flow_run(
        &self,
        input: &crate::ports::AttachCompiledPlanToFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(compiled) = inner
            .compiled_plans_by_id
            .get(&input.compiled_plan_id)
            .cloned()
        else {
            return Err(anyhow::anyhow!("flow run compiled plan cannot be attached"));
        };
        let Some(record) = inner.flow_runs_by_id.get_mut(&input.flow_run_id) else {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        };
        if record.status != domain::FlowRunStatus::Queued
            || record.compiled_plan_id.is_some()
            || record.flow_schema_version != input.flow_schema_version
            || record.document_hash != input.document_hash
            || compiled.flow_id != record.flow_id
            || compiled.draft_id != record.draft_id
            || compiled.schema_version != record.flow_schema_version
            || compiled.document_hash != record.document_hash
        {
            return Err(anyhow::anyhow!("flow run compiled plan cannot be attached"));
        }
        record.compiled_plan_id = Some(input.compiled_plan_id);
        record.status = input.status;
        record.updated_at = OffsetDateTime::now_utc();
        Ok(record.clone())
    }

    async fn fail_queued_flow_run_shell(
        &self,
        input: &crate::ports::FailQueuedFlowRunShellInput,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(record) = inner.flow_runs_by_id.get_mut(&input.flow_run_id) else {
            return Ok(None);
        };
        if record.status != domain::FlowRunStatus::Queued || record.compiled_plan_id.is_some() {
            return Ok(None);
        }
        record.status = domain::FlowRunStatus::Failed;
        record.output_payload = input.output_payload.clone();
        record.error_payload = Some(input.error_payload.clone());
        record.finished_at = Some(input.finished_at);
        record.updated_at = input.finished_at;
        Ok(Some(record.clone()))
    }

    async fn get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = inner
            .flow_runs_by_id
            .get(&flow_run_id)
            .filter(|record| record.application_id == application_id)
            .cloned();
        if let Some((race_flow_run_id, status)) = inner.status_after_next_get.take() {
            if race_flow_run_id == flow_run_id {
                if let Some(stored) = inner.flow_runs_by_id.get_mut(&flow_run_id) {
                    stored.status = status;
                }
            } else {
                inner.status_after_next_get = Some((race_flow_run_id, status));
            }
        }
        Ok(record)
    }

    async fn create_node_run(&self, input: &CreateNodeRunInput) -> Result<domain::NodeRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = node_run_record_from_create_input(input);
        inner.node_runs_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn update_node_run(&self, input: &UpdateNodeRunInput) -> Result<domain::NodeRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(record) = inner.node_runs_by_id.get_mut(&input.node_run_id) else {
            return Err(ControlPlaneError::NotFound("node_run").into());
        };
        record.status = input.status;
        record.output_payload = input.output_payload.clone();
        record.error_payload = input.error_payload.clone();
        record.metrics_payload = input.metrics_payload.clone();
        record.debug_payload = input.debug_payload.clone();
        record.finished_at = input.finished_at;
        Ok(record.clone())
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
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        force_status_before_next_flow_update(&mut inner, input.flow_run_id);
        let Some(record) = inner.flow_runs_by_id.get_mut(&input.flow_run_id) else {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        };
        record.status = input.status;
        record.output_payload = input.output_payload.clone();
        record.error_payload = input.error_payload.clone();
        record.finished_at = input.finished_at;
        record.updated_at = input.finished_at.unwrap_or_else(OffsetDateTime::now_utc);
        Ok(record.clone())
    }

    async fn update_flow_run_if_status(
        &self,
        input: &UpdateFlowRunInput,
        expected_status: domain::FlowRunStatus,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        force_status_before_next_flow_update(&mut inner, input.flow_run_id);
        let Some(record) = inner.flow_runs_by_id.get_mut(&input.flow_run_id) else {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        };
        if record.status != expected_status {
            return Ok(None);
        }
        record.status = input.status;
        record.output_payload = input.output_payload.clone();
        record.error_payload = input.error_payload.clone();
        record.finished_at = input.finished_at;
        record.updated_at = input.finished_at.unwrap_or_else(OffsetDateTime::now_utc);
        Ok(Some(record.clone()))
    }

    async fn commit_flow_run_terminal(
        &self,
        input: &CommitFlowRunTerminalInput,
    ) -> Result<CommitFlowRunTerminalReceipt> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        force_status_before_next_flow_update(&mut inner, input.flow_run_id);
        force_stream_terminal_failure_before_next_flow_update(&mut inner, input.flow_run_id);
        let Some(existing) = inner.flow_runs_by_id.get(&input.flow_run_id).cloned() else {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        };
        if existing.status != input.expected_status
            || matches!(
                existing.status,
                domain::FlowRunStatus::Succeeded
                    | domain::FlowRunStatus::Incomplete
                    | domain::FlowRunStatus::Failed
                    | domain::FlowRunStatus::Cancelled
            )
        {
            return Ok(CommitFlowRunTerminalReceipt::Loser);
        }

        let mut recovered = existing;
        recovered.status = input.result.status();
        recovered.output_payload = input.result.output_payload().clone();
        recovered.error_payload = input.result.error_payload().cloned();
        recovered.finished_at = Some(input.finished_at);
        recovered.updated_at = input.finished_at;

        let flow_event = domain::RunEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: recovered.id,
            node_run_id: None,
            sequence: inner
                .events_by_flow_run_id
                .get(&recovered.id)
                .map_or(0, Vec::len) as i64
                + 1,
            event_type: input.result.flow_run_event_type().to_string(),
            payload: input.flow_run_event_payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        let runtime_event = domain::RuntimeEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: recovered.id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            sequence: inner
                .runtime_events_by_flow_run_id
                .get(&recovered.id)
                .map_or(0, Vec::len) as i64
                + 1,
            event_type: input.result.runtime_event_type().to_string(),
            layer: domain::RuntimeEventLayer::AgentTransition,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload: input.terminal_event_payload.clone(),
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
            created_at: OffsetDateTime::now_utc(),
        };

        // This test seam models a database error after the statement set is assembled but before
        // the transaction commits. The real PostgreSQL implementation uses one transaction.
        if std::mem::take(&mut inner.fail_next_runtime_event_append) {
            return Err(anyhow::anyhow!("simulated runtime event append failure"));
        }

        inner
            .flow_runs_by_id
            .insert(recovered.id, recovered.clone());
        inner
            .events_by_flow_run_id
            .entry(recovered.id)
            .or_default()
            .push(flow_event);
        inner
            .runtime_events_by_flow_run_id
            .entry(recovered.id)
            .or_default()
            .push(runtime_event);
        if std::mem::take(&mut inner.fail_next_published_stream_terminal_projection) {
            return Ok(
                CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(recovered),
            );
        }
        Ok(CommitFlowRunTerminalReceipt::Winner(recovered))
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
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .checkpoints_by_id
            .get(&checkpoint_id)
            .filter(|record| record.flow_run_id == flow_run_id)
            .cloned())
    }

    async fn create_checkpoint(
        &self,
        input: &CreateCheckpointInput,
    ) -> Result<domain::CheckpointRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::CheckpointRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            status: input.status.clone(),
            reason: input.reason.clone(),
            locator_payload: input.locator_payload.clone(),
            variable_snapshot: input.variable_snapshot.clone(),
            external_ref_payload: input.external_ref_payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner.checkpoints_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn create_callback_task(
        &self,
        input: &CreateCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::CallbackTaskRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            callback_kind: input.callback_kind.clone(),
            status: domain::CallbackTaskStatus::Pending,
            request_payload: input.request_payload.clone(),
            response_payload: None,
            external_ref_payload: input.external_ref_payload.clone(),
            created_at: OffsetDateTime::now_utc(),
            completed_at: None,
        };
        inner.callback_tasks_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(record) = inner.callback_tasks_by_id.get_mut(&input.callback_task_id) else {
            return Err(ControlPlaneError::NotFound("callback_task").into());
        };
        if record.status != domain::CallbackTaskStatus::Pending {
            return Err(ControlPlaneError::Conflict("callback_task_not_pending").into());
        }
        record.status = domain::CallbackTaskStatus::Completed;
        record.response_payload = Some(input.response_payload.clone());
        record.completed_at = Some(input.completed_at);
        let mut completed = record.clone();
        if completed.callback_kind == "llm_tool_calls" {
            completed.request_payload = json!({
                "tool_calls": completed.request_payload.get("tool_calls").cloned()
            });
            completed.external_ref_payload = None;
        }
        Ok(completed)
    }

    async fn get_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.callback_tasks_by_id.get(&callback_task_id).cloned())
    }

    async fn get_callback_resume_context(
        &self,
        application_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<CallbackResumeContext>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(mut callback_task) = inner.callback_tasks_by_id.get(&callback_task_id).cloned()
        else {
            return Ok(None);
        };
        if callback_task.callback_kind == "llm_tool_calls" {
            callback_task.request_payload = json!({
                "tool_calls": callback_task.request_payload.get("tool_calls").cloned()
            });
            callback_task.external_ref_payload = None;
        }
        let Some(flow_run) = inner
            .flow_runs_by_id
            .get(&callback_task.flow_run_id)
            .filter(|flow_run| flow_run.application_id == application_id)
            .cloned()
        else {
            return Ok(None);
        };
        let checkpoint = inner
            .checkpoints_by_id
            .values()
            .filter(|checkpoint| {
                checkpoint.flow_run_id == flow_run.id
                    && checkpoint.node_run_id == Some(callback_task.node_run_id)
            })
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then(left.id.cmp(&right.id))
            })
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("checkpoint not found for callback task"))?;
        let waiting_node = inner
            .node_runs_by_id
            .get(&callback_task.node_run_id)
            .filter(|node_run| node_run.flow_run_id == flow_run.id)
            .ok_or_else(|| anyhow::anyhow!("waiting node run not found for callback task"))?;
        let next_node_started_at = inner
            .node_runs_by_id
            .values()
            .filter(|node_run| node_run.flow_run_id == flow_run.id)
            .map(|node_run| node_run.started_at)
            .max()
            .map(|started_at| started_at + time::Duration::seconds(1))
            .unwrap_or_else(OffsetDateTime::now_utc);

        Ok(Some(CallbackResumeContext {
            flow_run,
            callback_task,
            checkpoint,
            waiting_node: CallbackResumeWaitingNode {
                id: waiting_node.id,
                status: waiting_node.status,
                output_payload: waiting_node.output_payload.clone(),
            },
            next_node_started_at,
        }))
    }

    async fn upsert_debug_variable_cache_entry(
        &self,
        input: &UpsertDebugVariableCacheEntryInput,
    ) -> Result<DebugVariableCacheEntry> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let entry = DebugVariableCacheEntry {
            node_id: input.node_id.clone(),
            variable_key: input.variable_key.clone(),
            value: input.value.clone(),
        };
        inner.debug_variable_cache_entries_by_key.insert(
            (
                input.application_id,
                input.draft_id,
                input.actor_user_id,
                input.node_id.clone(),
                input.variable_key.clone(),
            ),
            entry.clone(),
        );
        Ok(entry)
    }

    async fn list_debug_variable_cache_entries(
        &self,
        application_id: Uuid,
        draft_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<Vec<DebugVariableCacheEntry>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .debug_variable_cache_entries_by_key
            .iter()
            .filter(
                |((cached_application_id, cached_draft_id, cached_actor_user_id, _, _), _)| {
                    *cached_application_id == application_id
                        && *cached_draft_id == draft_id
                        && *cached_actor_user_id == actor_user_id
                },
            )
            .map(|(_, entry)| entry.clone())
            .collect())
    }

    async fn delete_debug_variable_cache_entries(
        &self,
        input: &DeleteDebugVariableCacheEntriesInput,
    ) -> Result<()> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        match &input.keys {
            Some(keys) => {
                for key in keys {
                    inner.debug_variable_cache_entries_by_key.remove(&(
                        input.application_id,
                        input.draft_id,
                        input.actor_user_id,
                        key.node_id.clone(),
                        key.variable_key.clone(),
                    ));
                }
            }
            None => {
                inner.debug_variable_cache_entries_by_key.retain(
                    |(application_id, draft_id, actor_user_id, _, _), _| {
                        *application_id != input.application_id
                            || *draft_id != input.draft_id
                            || *actor_user_id != input.actor_user_id
                    },
                );
            }
        }
        Ok(())
    }

    async fn get_data_model_side_effect_receipt(
        &self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<domain::DataModelSideEffectReceiptRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .data_model_side_effect_receipts_by_idempotency
            .get(&(workspace_id, idempotency_key.to_string()))
            .cloned())
    }

    async fn claim_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<DataModelSideEffectReceiptClaim> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner
            .data_model_side_effect_receipts_by_idempotency
            .get(&key)
        {
            return Ok(DataModelSideEffectReceiptClaim {
                record: record.clone(),
                claimed: false,
            });
        }

        let record = domain::DataModelSideEffectReceiptRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            draft_id: input.draft_id,
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            node_id: input.node_id.clone(),
            action: input.action.clone(),
            model_code: input.model_code.clone(),
            record_id: None,
            deleted_id: None,
            affected_count: 0,
            idempotency_key: input.idempotency_key.clone(),
            payload_hash: input.payload_hash.clone(),
            actor_user_id: input.actor_user_id,
            scope_id: input.scope_id,
            status: "pending".to_string(),
            output_payload: json!({}),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .data_model_side_effect_receipts_by_idempotency
            .insert(key, record.clone());

        Ok(DataModelSideEffectReceiptClaim {
            record,
            claimed: true,
        })
    }

    async fn upsert_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<domain::DataModelSideEffectReceiptRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner
            .data_model_side_effect_receipts_by_idempotency
            .get(&key)
        {
            if record.status != "pending" {
                return Ok(record.clone());
            }
        }

        let record = domain::DataModelSideEffectReceiptRecord {
            id: inner
                .data_model_side_effect_receipts_by_idempotency
                .get(&key)
                .map(|record| record.id)
                .unwrap_or_else(Uuid::now_v7),
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            draft_id: input.draft_id,
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            node_id: input.node_id.clone(),
            action: input.action.clone(),
            model_code: input.model_code.clone(),
            record_id: input.record_id.clone(),
            deleted_id: input.deleted_id.clone(),
            affected_count: input.affected_count,
            idempotency_key: input.idempotency_key.clone(),
            payload_hash: input.payload_hash.clone(),
            actor_user_id: input.actor_user_id,
            scope_id: input.scope_id,
            status: input.status.clone(),
            output_payload: input.output_payload.clone(),
            created_at: inner
                .data_model_side_effect_receipts_by_idempotency
                .get(&key)
                .map(|record| record.created_at)
                .unwrap_or_else(OffsetDateTime::now_utc),
        };
        inner
            .data_model_side_effect_receipts_by_idempotency
            .insert(key, record.clone());

        Ok(record)
    }

    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let events = inner
            .events_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default();
        let event = domain::RunEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            sequence: (events.len() + 1) as i64,
            event_type: input.event_type.clone(),
            payload: input.payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        events.push(event.clone());
        Ok(event)
    }

    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> Result<domain::RuntimeSpanRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let span = domain::RuntimeSpanRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            parent_span_id: input.parent_span_id,
            kind: input.kind,
            name: input.name.clone(),
            status: input.status,
            capability_id: input.capability_id.clone(),
            input_ref: input.input_ref.clone(),
            output_ref: input.output_ref.clone(),
            error_payload: input.error_payload.clone(),
            metadata: input.metadata.clone(),
            started_at: input.started_at,
            finished_at: input.finished_at,
        };
        inner
            .runtime_spans_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(span.clone());
        Ok(span)
    }

    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> Result<domain::RuntimeEventRecord> {
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

    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> Result<domain::RuntimeItemRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let item = domain::RuntimeItemRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            kind: input.kind,
            status: input.status,
            source_event_id: input.source_event_id,
            input_ref: input.input_ref.clone(),
            output_ref: input.output_ref.clone(),
            usage_ledger_id: input.usage_ledger_id,
            trust_level: input.trust_level,
            created_at: now,
            updated_at: now,
        };
        inner
            .runtime_items_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(item.clone());
        Ok(item)
    }

    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> Result<domain::ContextProjectionRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::ContextProjectionRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            llm_turn_span_id: input.llm_turn_span_id,
            projection_kind: input.projection_kind.clone(),
            merge_stage_ref: input.merge_stage_ref.clone(),
            source_transcript_ref: input.source_transcript_ref.clone(),
            source_item_refs: input.source_item_refs.clone(),
            compaction_event_id: input.compaction_event_id,
            summary_version: input.summary_version.clone(),
            model_input_ref: input.model_input_ref.clone(),
            model_input_hash: input.model_input_hash.clone(),
            compacted_summary_ref: input.compacted_summary_ref.clone(),
            previous_projection_id: input.previous_projection_id,
            token_estimate: input.token_estimate,
            provider_continuation_metadata: input.provider_continuation_metadata.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .context_projections_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> Result<domain::UsageLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::UsageLedgerRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            span_id: input.span_id,
            failover_attempt_id: input.failover_attempt_id,
            provider_instance_id: input.provider_instance_id,
            gateway_route_id: input.gateway_route_id,
            model_id: input.model_id.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            upstream_request_id: input.upstream_request_id.clone(),
            input_tokens: input.input_tokens,
            cached_input_tokens: input.cached_input_tokens,
            output_tokens: input.output_tokens,
            reasoning_output_tokens: input.reasoning_output_tokens,
            total_tokens: input.total_tokens,
            input_cache_hit_tokens: input.input_cache_hit_tokens,
            input_cache_miss_tokens: input.input_cache_miss_tokens,
            cache_read_tokens: input.cache_read_tokens,
            cache_write_tokens: input.cache_write_tokens,
            price_snapshot: input.price_snapshot.clone(),
            cost_snapshot: input.cost_snapshot.clone(),
            usage_status: input.usage_status,
            raw_usage: input.raw_usage.clone(),
            normalized_usage: input.normalized_usage.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .usage_ledger_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    async fn append_cost_ledger(
        &self,
        input: &AppendCostLedgerInput,
    ) -> Result<domain::CostLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::CostLedgerRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            usage_ledger_id: input.usage_ledger_id,
            workspace_id: input.workspace_id,
            provider_instance_id: input.provider_instance_id,
            provider_account_id: input.provider_account_id,
            gateway_route_id: input.gateway_route_id,
            model_id: input.model_id.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            price_snapshot: input.price_snapshot.clone(),
            raw_cost: input.raw_cost.clone(),
            normalized_cost: input.normalized_cost.clone(),
            settlement_currency: input.settlement_currency.clone(),
            cost_source: input.cost_source.clone(),
            cost_status: input.cost_status.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        if let Some(flow_run_id) = record.flow_run_id {
            inner
                .cost_ledger_by_flow_run_id
                .entry(flow_run_id)
                .or_default()
                .push(record.clone());
        }
        Ok(record)
    }

    async fn append_credit_ledger(
        &self,
        input: &AppendCreditLedgerInput,
    ) -> Result<domain::CreditLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner.credit_ledger_by_idempotency.get(&key) {
            return Ok(record.clone());
        }
        let record = domain::CreditLedgerRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            user_id: input.user_id,
            application_id: input.application_id,
            agent_id: input.agent_id,
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            cost_ledger_id: input.cost_ledger_id,
            transaction_type: input.transaction_type.clone(),
            amount: input.amount.clone(),
            balance_after: input.balance_after.clone(),
            credit_unit: input.credit_unit.clone(),
            reason: input.reason.clone(),
            idempotency_key: input.idempotency_key.clone(),
            status: input.status.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .credit_ledger_by_idempotency
            .insert(key, record.clone());
        Ok(record)
    }

    async fn append_billing_session(
        &self,
        input: &AppendBillingSessionInput,
    ) -> Result<domain::BillingSessionRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner.billing_sessions_by_idempotency.get(&key) {
            return Ok(record.clone());
        }
        let now = OffsetDateTime::now_utc();
        let record = domain::BillingSessionRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            flow_run_id: input.flow_run_id,
            client_request_id: input.client_request_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            route_id: input.route_id,
            provider_account_id: input.provider_account_id,
            status: input.status,
            reserved_credit_ledger_id: input.reserved_credit_ledger_id,
            settled_credit_ledger_id: input.settled_credit_ledger_id,
            refund_credit_ledger_id: input.refund_credit_ledger_id,
            metadata: input.metadata.clone(),
            created_at: now,
            updated_at: now,
        };
        inner
            .billing_sessions_by_idempotency
            .insert(key, record.clone());
        Ok(record)
    }

    async fn append_audit_hash(
        &self,
        flow_run_id: Uuid,
        fact_table: &str,
        fact_id: Uuid,
        payload: serde_json::Value,
    ) -> Result<domain::AuditHashRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let hashes = inner
            .audit_hashes_by_flow_run_id
            .entry(flow_run_id)
            .or_default();
        let prev_hash = hashes.last().map(|record| record.row_hash.as_str());
        let record = domain::AuditHashRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            fact_table: fact_table.to_string(),
            fact_id,
            prev_hash: prev_hash.map(ToString::to_string),
            row_hash: crate::runtime_observability::audit_row_hash(
                prev_hash, fact_table, fact_id, &payload,
            ),
            created_at: OffsetDateTime::now_utc(),
        };
        hashes.push(record.clone());
        Ok(record)
    }

    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::ModelFailoverAttemptLedgerRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            llm_turn_span_id: input.llm_turn_span_id,
            queue_snapshot_id: input.queue_snapshot_id,
            attempt_index: input.attempt_index,
            provider_instance_id: input.provider_instance_id,
            provider_code: input.provider_code.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            protocol: input.protocol.clone(),
            request_ref: input.request_ref.clone(),
            request_hash: input.request_hash.clone(),
            started_at: input.started_at,
            first_token_at: input.first_token_at,
            finished_at: input.finished_at,
            status: input.status.clone(),
            failed_after_first_token: input.failed_after_first_token,
            upstream_request_id: input.upstream_request_id.clone(),
            error_code: input.error_code.clone(),
            error_message_ref: input.error_message_ref.clone(),
            usage_ledger_id: input.usage_ledger_id,
            cost_ledger_id: input.cost_ledger_id,
            response_ref: input.response_ref.clone(),
        };
        inner
            .model_failover_attempts_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let attempt = inner
            .model_failover_attempts_by_flow_run_id
            .values_mut()
            .flat_map(|attempts| attempts.iter_mut())
            .find(|attempt| attempt.id == input.failover_attempt_id)
            .ok_or_else(|| anyhow::anyhow!("model failover attempt not found"))?;
        attempt.usage_ledger_id = Some(input.usage_ledger_id);
        Ok(attempt.clone())
    }

    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> Result<domain::CapabilityInvocationRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::CapabilityInvocationRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            capability_id: input.capability_id.clone(),
            requested_by_span_id: input.requested_by_span_id,
            requester_kind: input.requester_kind.clone(),
            arguments_ref: input.arguments_ref.clone(),
            authorization_status: input.authorization_status.clone(),
            authorization_reason: input.authorization_reason.clone(),
            result_ref: input.result_ref.clone(),
            normalized_result: input.normalized_result.clone(),
            started_at: input.started_at,
            finished_at: input.finished_at,
            error_payload: input.error_payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .capability_invocations_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    async fn list_runtime_spans(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeSpanRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let mut spans = inner
            .runtime_spans_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default();
        spans.sort_by(|left, right| {
            left.started_at
                .cmp(&right.started_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(spans)
    }

    async fn list_runtime_events(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .runtime_events_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.sequence > after_sequence)
            .collect())
    }

    async fn get_runtime_event_sequence_for_callback_task(
        &self,
        flow_run_id: Uuid,
        callback_task_id: Uuid,
    ) -> Result<Option<i64>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .runtime_events_by_flow_run_id
            .get(&flow_run_id)
            .into_iter()
            .flatten()
            .filter(|event| {
                event
                    .payload
                    .get("callback_task_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    == Some(callback_task_id)
            })
            .map(|event| event.sequence)
            .max())
    }

    async fn list_runtime_items(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeItemRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .runtime_items_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_context_projections(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ContextProjectionRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .context_projections_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_usage_ledger(&self, flow_run_id: Uuid) -> Result<Vec<domain::UsageLedgerRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .usage_ledger_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_model_failover_attempt_ledger(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverAttemptLedgerRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .model_failover_attempts_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_capability_invocations(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::CapabilityInvocationRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .capability_invocations_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_application_runs(
        &self,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunSummary>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let mut runs = inner
            .flow_runs_by_id
            .values()
            .filter(|record| record.application_id == application_id)
            .map(|record| domain::ApplicationRunSummary {
                id: record.id,
                run_mode: record.run_mode,
                status: record.status,
                target_node_id: record.target_node_id.clone(),
                title: record.title.clone(),
                user_id: record.external_user.clone(),
                authorized_account: record.authorized_account.clone(),
                api_key_id: record.api_key_id,
                publication_version_id: record.publication_version_id,
                external_conversation_id: record.external_conversation_id.clone(),
                external_trace_id: record.external_trace_id.clone(),
                compatibility_mode: record.compatibility_mode.clone(),
                idempotency_key: record.idempotency_key.clone(),
                started_at: record.started_at,
                finished_at: record.finished_at,
                created_at: record.created_at,
                updated_at: record.updated_at,
            })
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(runs)
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
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        inner.application_run_detail_read_count += 1;
        let Some(flow_run) = inner.flow_runs_by_id.get(&flow_run_id).cloned() else {
            return Ok(None);
        };
        if flow_run.application_id != application_id {
            return Ok(None);
        }

        let mut node_runs = inner
            .node_runs_by_id
            .values()
            .filter(|record| record.flow_run_id == flow_run.id)
            .cloned()
            .collect::<Vec<_>>();
        node_runs.sort_by(|left, right| {
            left.started_at
                .cmp(&right.started_at)
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(Some(domain::ApplicationRunDetail {
            flow_run,
            node_runs,
            checkpoints: {
                let mut checkpoints = inner
                    .checkpoints_by_id
                    .values()
                    .filter(|record| record.flow_run_id == flow_run_id)
                    .cloned()
                    .collect::<Vec<_>>();
                checkpoints.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.id.cmp(&right.id))
                });
                checkpoints
            },
            callback_tasks: {
                let mut callback_tasks = inner
                    .callback_tasks_by_id
                    .values()
                    .filter(|record| record.flow_run_id == flow_run_id)
                    .cloned()
                    .collect::<Vec<_>>();
                callback_tasks.sort_by(|left, right| {
                    left.created_at
                        .cmp(&right.created_at)
                        .then_with(|| left.id.cmp(&right.id))
                });
                callback_tasks
            },
            events: inner
                .events_by_flow_run_id
                .get(&flow_run_id)
                .cloned()
                .unwrap_or_default(),
            stitched_trace: Vec::new(),
            subagent_traces: Vec::new(),
        }))
    }

    async fn get_latest_node_run(
        &self,
        application_id: Uuid,
        node_id: &str,
    ) -> Result<Option<domain::NodeLastRun>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let mut candidates = inner
            .node_runs_by_id
            .values()
            .filter_map(|node_run| {
                inner
                    .flow_runs_by_id
                    .get(&node_run.flow_run_id)
                    .filter(|flow_run| {
                        flow_run.application_id == application_id && node_run.node_id == node_id
                    })
                    .map(|flow_run| (flow_run.clone(), node_run.clone()))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .1
                .started_at
                .cmp(&left.1.started_at)
                .then_with(|| right.1.id.cmp(&left.1.id))
        });
        let Some((flow_run, node_run)) = candidates.into_iter().next() else {
            return Ok(None);
        };

        let events = inner
            .events_by_flow_run_id
            .get(&flow_run.id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.node_run_id.is_none() || event.node_run_id == Some(node_run.id))
            .collect();

        Ok(Some(domain::NodeLastRun {
            flow_run,
            node_run,
            checkpoints: Vec::new(),
            events,
        }))
    }
}
