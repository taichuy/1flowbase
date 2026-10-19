use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_create_callback_task(
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

    pub(super) fn memory_complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let Some(record) = inner.callback_tasks_by_id.get_mut(&input.callback_task_id) else {
            return Err(ControlPlaneError::NotFound("callback_task").into());
        };
        if record.status == domain::CallbackTaskStatus::Completed
            && record.response_payload.as_ref() == Some(&input.response_payload)
        {
            return Ok(record.clone());
        }
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

    pub(super) fn memory_get_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.callback_tasks_by_id.get(&callback_task_id).cloned())
    }

    pub(super) fn memory_get_callback_resume_context(
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
}
