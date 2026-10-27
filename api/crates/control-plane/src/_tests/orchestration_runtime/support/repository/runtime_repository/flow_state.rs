use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_get_flow_run_node_usages(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<control_plane_contracts::application_public_runtime::PublishedRunNodeUsage>>
    {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .node_runs_by_id
            .values()
            .filter(|node| node.flow_run_id == flow_run_id)
            .map(|node| {
                control_plane_contracts::application_public_runtime::PublishedRunNodeUsage {
                    metrics_usage: node.metrics_payload.get("usage").cloned(),
                    output_usage: node.output_payload.get("usage").cloned(),
                }
            })
            .collect())
    }

    pub(super) fn memory_upsert_compiled_plan(
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

    pub(super) fn memory_get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.compiled_plans_by_id.get(&compiled_plan_id).cloned())
    }

    pub(super) fn memory_create_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = flow_run_record_from_create_input(input);
        inner.flow_runs_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    pub(super) fn memory_create_flow_run_shell(
        &self,
        input: &crate::ports::CreateFlowRunShellInput,
    ) -> Result<domain::FlowRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = flow_run_shell_record_from_input(input);
        inner.flow_runs_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    pub(super) fn memory_attach_compiled_plan_to_flow_run(
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

    pub(super) fn memory_fail_queued_flow_run_shell(
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

    pub(super) fn memory_get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        if std::mem::take(&mut inner.initial_run_read_fault_once) {
            anyhow::bail!("injected initial run read failure");
        }
        let record = inner
            .flow_runs_by_id
            .get(&flow_run_id)
            .filter(|record| record.application_id == application_id)
            .cloned();
        if inner.waiting_read_fault.0
            && record.as_ref().is_some_and(|run| {
                matches!(
                    run.status,
                    domain::FlowRunStatus::WaitingCallback | domain::FlowRunStatus::WaitingHuman
                )
            })
        {
            anyhow::bail!("injected waiting run read failure");
        }
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

    pub(super) fn memory_create_node_run(
        &self,
        input: &CreateNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = node_run_record_from_create_input(input);
        inner.node_runs_by_id.insert(record.id, record.clone());
        Ok(record)
    }

    pub(super) fn memory_update_node_run(
        &self,
        input: &UpdateNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
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
        let record = record.clone();
        if let Some(parent) = inner.flow_runs_by_id.get_mut(&record.flow_run_id) {
            parent.updated_at = input.finished_at.unwrap_or_else(OffsetDateTime::now_utc);
        }
        Ok(record)
    }

    pub(super) fn memory_update_flow_run(
        &self,
        input: &UpdateFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
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

    pub(super) fn memory_update_flow_run_if_status(
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

    pub(super) fn memory_commit_flow_run_terminal(
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
            || (input.require_initial_execution
                && (inner
                    .checkpoints_by_id
                    .values()
                    .any(|checkpoint| checkpoint.flow_run_id == input.flow_run_id)
                    || inner
                        .resume_claims_by_target
                        .values()
                        .any(|claim| claim.flow_run_id == input.flow_run_id)))
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
        if std::mem::take(&mut inner.fail_next_terminal_runtime_event_append)
            || std::mem::take(&mut inner.fail_next_runtime_event_append)
        {
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

    pub(super) fn memory_get_checkpoint(
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

    pub(super) fn memory_create_checkpoint(
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
}
