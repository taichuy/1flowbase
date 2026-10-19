use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_list_runtime_spans(
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

    pub(super) fn memory_list_runtime_events(
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

    pub(super) fn memory_get_runtime_event_sequence_for_callback_task(
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

    pub(super) fn memory_list_runtime_items(
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

    pub(super) fn memory_list_context_projections(
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

    pub(super) fn memory_list_usage_ledger(&self, flow_run_id: Uuid) -> Result<Vec<domain::UsageLedgerRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .usage_ledger_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default())
    }

    pub(super) fn memory_list_model_failover_attempt_ledger(
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

    pub(super) fn memory_list_capability_invocations(
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

    pub(super) fn memory_list_application_runs(
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
                created_by: Some(record.created_by),
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

    pub(super) fn memory_get_application_run_detail(
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
            native_messages: Vec::new(),
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

            task_rounds: Vec::new(),

            child_task_traces: Vec::new(),
        }))
    }

    pub(super) fn memory_get_latest_node_run(
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
