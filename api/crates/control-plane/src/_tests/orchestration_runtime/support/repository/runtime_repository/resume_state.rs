use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_persist_waiting_state(
        &self,
        input: &crate::ports::PersistWaitingStateInput,
    ) -> Result<Option<crate::ports::PersistedWaitingState>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        // Match PostgreSQL's (flow_run_id, idempotency_key) recovery uniqueness.
        if inner
            .recovery_history_by_flow_run_id
            .get(&input.flow_run_id)
            .is_some_and(|records| {
                records
                    .iter()
                    .any(|record| record.idempotency_key == input.recovery_idempotency_key)
            })
        {
            return Err(anyhow::anyhow!("duplicate recovery idempotency key"));
        }
        force_status_before_next_flow_update(&mut inner, input.flow_run_id);
        let target_status = match input.kind {
            crate::ports::PersistWaitingKind::Human => domain::FlowRunStatus::WaitingHuman,
            crate::ports::PersistWaitingKind::Callback(_) => domain::FlowRunStatus::WaitingCallback,
        };
        let flow_run = inner
            .flow_runs_by_id
            .get_mut(&input.flow_run_id)
            .ok_or_else(|| anyhow::anyhow!("flow run not found"))?;
        if flow_run.status != input.expected_status {
            return Ok(None);
        }
        flow_run.status = target_status;
        flow_run.output_payload = input.output_payload.clone();
        flow_run.updated_at = OffsetDateTime::now_utc();
        let flow_run = flow_run.clone();
        let serialized = serde_json::to_string(&input.context_content)?;
        let content_key = (input.application_id, serialized.clone());
        let content_id = match inner
            .canonical_runtime_content_ids_by_value
            .get(&content_key)
            .copied()
        {
            Some(id) => id,
            None => {
                let content = domain::CanonicalRuntimeContentRecord {
                    id: Uuid::now_v7(),
                    scope_id: input.scope_id,
                    application_id: input.application_id,
                    content_hash: format!("sha256:{:064x}", serialized.len()),
                    content: input.context_content.clone(),
                    byte_size: i64::try_from(serialized.len())?,
                    created_at: OffsetDateTime::now_utc(),
                };
                inner
                    .canonical_runtime_content_ids_by_value
                    .insert(content_key, content.id);
                inner
                    .canonical_runtime_contents_by_id
                    .insert(content.id, content.clone());
                content.id
            }
        };
        let context_sequence = inner
            .context_versions_by_id
            .values()
            .filter(|version| version.flow_run_id == input.flow_run_id)
            .map(|version| version.sequence)
            .max()
            .unwrap_or(-1)
            + 1;
        let context_version = domain::ContextVersionRecord {
            id: Uuid::now_v7(),
            scope_id: input.scope_id,
            application_id: input.application_id,
            flow_run_id: input.flow_run_id,
            parent_context_version_id: input.parent_context_version_id,
            sequence: context_sequence,
            transition_kind: input.context_transition_kind,
            transition_actor: domain::ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: content_id,
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .context_versions_by_id
            .insert(context_version.id, context_version.clone());
        let mut locator_payload = input.locator_payload.clone();
        locator_payload["context_version_id"] = json!(context_version.id);
        let mut variable_snapshot = input.variable_snapshot.clone();
        variable_snapshot["__runtime_recovery_context"]["context_version_id"] =
            json!(context_version.id);
        variable_snapshot["__runtime_recovery_context"]["sequence"] = json!(context_sequence);
        let checkpoint = domain::CheckpointRecord {
            id: input.checkpoint_id,
            flow_run_id: input.flow_run_id,
            node_run_id: Some(input.node_run_id),
            status: input.checkpoint_status.clone(),
            reason: input.checkpoint_reason.clone(),
            locator_payload,
            variable_snapshot,
            external_ref_payload: input.checkpoint_external_ref_payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .checkpoints_by_id
            .insert(checkpoint.id, checkpoint.clone());
        let callback_task = match &input.kind {
            crate::ports::PersistWaitingKind::Human => None,
            crate::ports::PersistWaitingKind::Callback(callback) => {
                let record = domain::CallbackTaskRecord {
                    id: callback.id,
                    flow_run_id: input.flow_run_id,
                    node_run_id: input.node_run_id,
                    callback_kind: callback.callback_kind.clone(),
                    status: domain::CallbackTaskStatus::Pending,
                    request_payload: callback.request_payload.clone(),
                    response_payload: None,
                    external_ref_payload: callback.external_ref_payload.clone(),
                    created_at: OffsetDateTime::now_utc(),
                    completed_at: None,
                };
                inner.callback_tasks_by_id.insert(record.id, record.clone());
                if callback.callback_kind == "llm_tool_calls" {
                    let rows = callback
                        .request_payload
                        .get("tool_calls")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            anyhow::anyhow!("llm tool callback request is missing tool_calls")
                        })?
                        .iter()
                        .map(|tool_call| {
                            tool_call
                                .get("call_id")
                                .or_else(|| tool_call.get("id"))
                                .and_then(Value::as_str)
                                .filter(|id| !id.is_empty())
                                .map(|id| (id.to_string(), None))
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "llm tool callback request has an unstable tool call id"
                                    )
                                })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    inner.tool_callback_inbox_by_round.insert(record.id, rows);
                }
                Some(record)
            }
        };
        let events = inner
            .runtime_events_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default();
        let mut tool_delivery_events = Vec::with_capacity(input.tool_delivery_events.len());
        for event in &input.tool_delivery_events {
            let record = domain::RuntimeEventRecord {
                id: Uuid::now_v7(),
                flow_run_id: input.flow_run_id,
                node_run_id: event.node_run_id,
                span_id: event.span_id,
                parent_span_id: event.parent_span_id,
                sequence: i64::try_from(events.len() + 1)?,
                event_type: event.event_type.clone(),
                layer: event.layer,
                source: event.source,
                trust_level: event.trust_level,
                item_id: event.item_id,
                ledger_ref: event.ledger_ref.clone(),
                payload: event.payload.clone(),
                visibility: event.visibility,
                durability: event.durability,
                created_at: OffsetDateTime::now_utc(),
            };
            events.push(record.clone());
            tool_delivery_events.push(record);
        }
        let waiting_event = domain::RuntimeEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.waiting_event.node_run_id,
            span_id: input.waiting_event.span_id,
            parent_span_id: input.waiting_event.parent_span_id,
            sequence: i64::try_from(events.len() + 1)?,
            event_type: input.waiting_event.event_type.clone(),
            layer: input.waiting_event.layer,
            source: input.waiting_event.source,
            trust_level: input.waiting_event.trust_level,
            item_id: input.waiting_event.item_id,
            ledger_ref: input.waiting_event.ledger_ref.clone(),
            payload: input.waiting_event.payload.clone(),
            visibility: input.waiting_event.visibility,
            durability: input.waiting_event.durability,
            created_at: OffsetDateTime::now_utc(),
        };
        events.push(waiting_event.clone());
        let recovery_records = inner
            .recovery_history_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default();
        let recovery_history = domain::RecoveryHistoryRecord {
            id: Uuid::now_v7(),
            scope_id: input.scope_id,
            application_id: input.application_id,
            flow_run_id: input.flow_run_id,
            node_run_id: Some(input.node_run_id),
            sequence: i64::try_from(recovery_records.len())?,
            state_code: match target_status {
                domain::FlowRunStatus::WaitingHuman => domain::RecoveryStateCode::WaitingHuman,
                _ => domain::RecoveryStateCode::WaitingCallback,
            },
            coordinate: domain::RecoveryCoordinate {
                node_sequence: input
                    .locator_payload
                    .get("next_node_index")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                iteration_index: 0,
                attempt_index: 0,
                resume_sequence: i64::try_from(recovery_records.len())?,
                event_sequence: waiting_event.sequence,
            },
            context_version_id: context_version.id,
            recovery_content_id: Some(content_id),
            idempotency_key: input.recovery_idempotency_key.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        recovery_records.push(recovery_history.clone());
        match (input.resume_claim_id, input.resume_claim_token) {
            (Some(claim_id), Some(claim_token)) => {
                let claim = inner
                    .resume_claims_by_target
                    .values_mut()
                    .find(|claim| claim.id == claim_id && claim.claim_token == claim_token)
                    .ok_or(crate::errors::ControlPlaneError::Conflict(
                        "resume_claim_not_owned",
                    ))?;
                if claim.status != crate::ports::ResumeClaimStatus::Processing {
                    return Err(crate::errors::ControlPlaneError::Conflict(
                        "resume_claim_not_owned",
                    )
                    .into());
                }
                claim.status = crate::ports::ResumeClaimStatus::Succeeded;
                claim.completed_at = Some(OffsetDateTime::now_utc());
            }
            (None, None) => {}
            _ => {
                return Err(anyhow::anyhow!(
                    "resume claim id and token must be provided together"
                ));
            }
        }
        Ok(Some(crate::ports::PersistedWaitingState {
            flow_run,
            checkpoint,
            callback_task,
            waiting_event,
            tool_delivery_events,
            recovery_history,
        }))
    }

    pub(super) fn memory_acquire_resume_claim(
        &self,
        input: &crate::ports::AcquireResumeClaimInput,
    ) -> Result<crate::ports::AcquireResumeClaimOutput> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        inner.resume_claim_acquire_count += 1;
        let target_id = input.callback_task_id.unwrap_or(input.checkpoint_id);
        let flow_status = inner
            .flow_runs_by_id
            .get(&input.flow_run_id)
            .ok_or_else(|| anyhow::anyhow!("flow run not found"))?
            .status;
        if let Some(existing) = inner.resume_claims_by_target.get_mut(&target_id) {
            if existing.flow_run_id != input.flow_run_id
                || existing.checkpoint_id != input.checkpoint_id
                || existing.callback_task_id != input.callback_task_id
                || existing.kind != input.kind
                || existing.request_payload != input.request_payload
            {
                return Err(crate::errors::ControlPlaneError::Conflict(
                    "resume_claim_payload_conflict",
                )
                .into());
            }
            let waiting_status = match input.kind {
                crate::ports::ResumeClaimKind::Human => domain::FlowRunStatus::WaitingHuman,
                crate::ports::ResumeClaimKind::Callback => domain::FlowRunStatus::WaitingCallback,
            };
            if existing.status == crate::ports::ResumeClaimStatus::Succeeded
                || flow_status != waiting_status
            {
                existing.status = crate::ports::ResumeClaimStatus::Succeeded;
                existing
                    .completed_at
                    .get_or_insert_with(OffsetDateTime::now_utc);
                return Ok(crate::ports::AcquireResumeClaimOutput {
                    claim: existing.clone(),
                    disposition: crate::ports::ResumeClaimDisposition::Completed,
                });
            }
            if existing.status == crate::ports::ResumeClaimStatus::Processing
                && existing.lease_expires_at > OffsetDateTime::now_utc()
            {
                return Ok(crate::ports::AcquireResumeClaimOutput {
                    claim: existing.clone(),
                    disposition: crate::ports::ResumeClaimDisposition::InProgress,
                });
            }
            existing.status = crate::ports::ResumeClaimStatus::Processing;
            existing.claim_token = Uuid::now_v7();
            existing.generation += 1;
            existing.lease_expires_at = OffsetDateTime::now_utc() + time::Duration::minutes(5);
            existing.error_payload = None;
            existing.completed_at = None;
            return Ok(crate::ports::AcquireResumeClaimOutput {
                claim: existing.clone(),
                disposition: crate::ports::ResumeClaimDisposition::Acquired,
            });
        }
        let waiting_status = match input.kind {
            crate::ports::ResumeClaimKind::Human => domain::FlowRunStatus::WaitingHuman,
            crate::ports::ResumeClaimKind::Callback => domain::FlowRunStatus::WaitingCallback,
        };
        if flow_status != waiting_status {
            return Err(
                crate::errors::ControlPlaneError::Conflict("resume_claim_not_waiting").into(),
            );
        }
        let claim = crate::ports::ResumeClaimRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            checkpoint_id: input.checkpoint_id,
            callback_task_id: input.callback_task_id,
            kind: input.kind,
            status: crate::ports::ResumeClaimStatus::Processing,
            request_payload: input.request_payload.clone(),
            claim_token: Uuid::now_v7(),
            generation: 0,
            lease_expires_at: OffsetDateTime::now_utc() + time::Duration::minutes(5),
            error_payload: None,
            completed_at: None,
        };
        inner
            .resume_claims_by_target
            .insert(target_id, claim.clone());
        Ok(crate::ports::AcquireResumeClaimOutput {
            claim,
            disposition: crate::ports::ResumeClaimDisposition::Acquired,
        })
    }

    pub(super) fn memory_commit_tool_callback_results(
        &self,
        input: &crate::ports::CommitToolCallbackResultsInput,
    ) -> Result<crate::ports::CommitToolCallbackResultsOutput> {
        if input.results.is_empty() {
            return Err(crate::errors::ControlPlaneError::InvalidInput("tool_results").into());
        }
        let mut submitted = std::collections::BTreeSet::new();
        if input
            .results
            .iter()
            .any(|result| !submitted.insert(result.tool_call_id.as_str()))
        {
            return Err(crate::errors::ControlPlaneError::Conflict(
                "tool_callback_result_duplicate",
            )
            .into());
        }
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let callback = inner
            .callback_tasks_by_id
            .get(&input.callback_task_id)
            .cloned()
            .ok_or(crate::errors::ControlPlaneError::Conflict(
                "tool_callback_round_not_owned",
            ))?;
        let flow_run = inner
            .flow_runs_by_id
            .get(&input.flow_run_id)
            .filter(|run| {
                run.application_id == input.application_id && callback.flow_run_id == run.id
            })
            .ok_or(crate::errors::ControlPlaneError::Conflict(
                "tool_callback_round_not_owned",
            ))?;
        if callback.status == domain::CallbackTaskStatus::Pending
            && flow_run.status != domain::FlowRunStatus::WaitingCallback
        {
            return Err(crate::errors::ControlPlaneError::Conflict(
                "flow_run_not_waiting_callback",
            )
            .into());
        }
        let rows = inner
            .tool_callback_inbox_by_round
            .get_mut(&input.callback_task_id)
            .ok_or(crate::errors::ControlPlaneError::Conflict(
                "tool_callback_round_invalid",
            ))?;
        for result in &input.results {
            let row = rows
                .iter_mut()
                .find(|(tool_call_id, _)| tool_call_id == &result.tool_call_id)
                .ok_or(crate::errors::ControlPlaneError::Conflict(
                    "tool_callback_result_unknown",
                ))?;
            match &row.1 {
                Some((fingerprint, _)) if fingerprint != &result.result_fingerprint => {
                    return Err(crate::errors::ControlPlaneError::Conflict(
                        "tool_callback_result_conflict",
                    )
                    .into());
                }
                Some(_) => {}
                None => {
                    row.1 = Some((
                        result.result_fingerprint.clone(),
                        result.result_payload.clone(),
                    ));
                }
            }
        }
        if rows.iter().any(|(_, result)| result.is_none()) {
            return Ok(crate::ports::CommitToolCallbackResultsOutput {
                callback_task: callback,
                claim: None,
                disposition: crate::ports::ToolCallbackRoundDisposition::WaitingForResults,
            });
        }
        let mut response_payload = json!({
            "tool_results": rows
                .iter()
                .filter_map(|(_, result)| result.as_ref().map(|(_, payload)| payload.clone()))
                .collect::<Vec<_>>()
        });
        if let Some(continuation) = &input.responses_continuation {
            response_payload["responses_continuation"] = serde_json::to_value(continuation)?;
        }
        if callback.status == domain::CallbackTaskStatus::Completed {
            if callback
                .response_payload
                .as_ref()
                .and_then(|payload| payload.get("responses_continuation"))
                != response_payload.get("responses_continuation")
            {
                return Err(crate::errors::ControlPlaneError::Conflict(
                    "responses_continuation_conflict",
                )
                .into());
            }
            let claim = inner
                .resume_claims_by_target
                .get(&input.callback_task_id)
                .cloned();
            return Ok(crate::ports::CommitToolCallbackResultsOutput {
                callback_task: callback,
                claim,
                disposition: crate::ports::ToolCallbackRoundDisposition::Completed,
            });
        }
        let callback = inner
            .callback_tasks_by_id
            .get_mut(&input.callback_task_id)
            .expect("callback still exists");
        callback.status = domain::CallbackTaskStatus::Completed;
        callback.response_payload = Some(response_payload.clone());
        callback.completed_at = Some(OffsetDateTime::now_utc());
        let callback = callback.clone();
        let claim = crate::ports::ResumeClaimRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            checkpoint_id: input.checkpoint_id,
            callback_task_id: Some(input.callback_task_id),
            kind: crate::ports::ResumeClaimKind::Callback,
            status: crate::ports::ResumeClaimStatus::Processing,
            request_payload: response_payload,
            claim_token: Uuid::now_v7(),
            generation: 0,
            lease_expires_at: OffsetDateTime::now_utc() + time::Duration::minutes(5),
            error_payload: None,
            completed_at: None,
        };
        inner
            .resume_claims_by_target
            .insert(input.callback_task_id, claim.clone());
        Ok(crate::ports::CommitToolCallbackResultsOutput {
            callback_task: callback,
            claim: Some(claim),
            disposition: crate::ports::ToolCallbackRoundDisposition::Acquired,
        })
    }

    pub(super) fn memory_renew_resume_claim(
        &self,
        input: &crate::ports::RenewResumeClaimInput,
    ) -> Result<bool> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(claim) = inner
            .resume_claims_by_target
            .values_mut()
            .find(|claim| claim.id == input.claim_id)
        else {
            return Ok(false);
        };
        if claim.claim_token != input.claim_token
            || claim.generation != input.expected_generation
            || claim.status != crate::ports::ResumeClaimStatus::Processing
            || claim.lease_expires_at <= OffsetDateTime::now_utc()
        {
            return Ok(false);
        }
        claim.lease_expires_at = OffsetDateTime::now_utc() + time::Duration::minutes(5);
        Ok(true)
    }

    pub(super) fn memory_finish_resume_claim(
        &self,
        input: &crate::ports::FinishResumeClaimInput,
    ) -> Result<crate::ports::ResumeClaimRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let claim = inner
            .resume_claims_by_target
            .values_mut()
            .find(|claim| claim.id == input.claim_id)
            .ok_or_else(|| anyhow::anyhow!("resume claim not found"))?;
        if claim.claim_token != input.claim_token
            || claim.generation != input.expected_generation
            || (claim.status != crate::ports::ResumeClaimStatus::Processing
                && claim.status != input.status)
            || input.status == crate::ports::ResumeClaimStatus::Processing
        {
            return Err(
                crate::errors::ControlPlaneError::Conflict("resume_claim_not_owned").into(),
            );
        }
        if claim.status == crate::ports::ResumeClaimStatus::Processing {
            claim.error_payload = input.error_payload.clone();
            claim.completed_at = Some(input.completed_at);
        }
        claim.status = input.status;
        Ok(claim.clone())
    }
}
