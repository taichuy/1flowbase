use super::*;
use crate::application_public_api::callback_resume;
use serde_json::json;

#[async_trait]
impl ApplicationJsDependencySelectionRepository for ApplicationPublicApiTestRepository {
    async fn list_application_js_dependency_selections(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationJsDependencySelection>> {
        let mut selections = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .js_dependency_selections
            .values()
            .filter(|selection| {
                selection.workspace_id == workspace_id && selection.application_id == application_id
            })
            .cloned()
            .collect::<Vec<_>>();
        selections.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then(left.target.cmp(&right.target))
        });
        Ok(selections)
    }

    async fn replace_application_js_dependency_selection(
        &self,
        input: &ReplaceApplicationJsDependencySelectionInput,
    ) -> Result<domain::ApplicationJsDependencySelection> {
        let selection = domain::ApplicationJsDependencySelection {
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            installation_id: input.installation_id,
            provider_code: input.provider_code.clone(),
            plugin_id: input.plugin_id.clone(),
            plugin_version: input.plugin_version.clone(),
            alias: input.alias.clone(),
            package: input.package.clone(),
            version: input.version.clone(),
            target: input.target.clone(),
            artifact_path: input.artifact_path.clone(),
            artifact_hash: input.artifact_hash.clone(),
            integrity: input.integrity.clone(),
            permissions: input.permissions.clone(),
        };
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .js_dependency_selections
            .insert(
                (
                    input.application_id,
                    input.alias.clone(),
                    input.target.clone(),
                ),
                selection.clone(),
            );
        Ok(selection)
    }
}

#[async_trait]
impl conversations::ApplicationPublicConversationRepository for ApplicationPublicApiTestRepository {
    async fn bind_application_public_conversation(
        &self,
        input: &conversations::BindApplicationPublicConversationInput,
    ) -> Result<conversations::ApplicationPublicConversationRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let key = (
            input.application_id,
            input.api_key_id,
            input.external_user.clone(),
            input.external_conversation_id.clone(),
        );
        let now = OffsetDateTime::now_utc();
        if let Some(record) = inner.conversations.get_mut(&key) {
            record.updated_at = now;
            return Ok(record.clone());
        }

        let record = conversations::ApplicationPublicConversationRecord {
            id: Uuid::now_v7(),
            application_id: input.application_id,
            api_key_id: input.api_key_id,
            external_user: input.external_user.clone(),
            external_conversation_id: input.external_conversation_id.clone(),
            created_at: now,
            updated_at: now,
        };
        inner.conversations.insert(key, record.clone());
        Ok(record)
    }

    async fn list_application_public_conversation_messages(
        &self,
        _input: &conversations::ListApplicationPublicConversationMessagesInput,
    ) -> Result<Vec<conversations::ApplicationPublicConversationMessageRecord>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl run_service::ApplicationPublishedFlowRunRepository for ApplicationPublicApiTestRepository {
    async fn create_published_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<run_service::CreatePublishedFlowRunResult> {
        let record = domain::FlowRunRecord {
            id: Uuid::now_v7(),
            application_id: input.application_id,
            flow_id: input.flow_id,
            draft_id: input.flow_draft_id,
            compiled_plan_id: Some(input.compiled_plan_id),
            debug_session_id: input.debug_session_id.clone(),
            flow_schema_version: input.flow_schema_version.clone(),
            document_hash: input.document_hash.clone(),
            run_mode: input.run_mode,
            target_node_id: input.target_node_id.clone(),
            title: input.title.clone(),
            status: input.status,
            input_payload: input.input_payload.clone(),
            output_payload: serde_json::json!({}),
            error_payload: None,
            created_by: input.actor_user_id,
            authorized_account: None,
            api_key_id: input.api_key_id,
            publication_version_id: input.publication_version_id,
            external_user: input.external_user.clone(),
            external_conversation_id: input.external_conversation_id.clone(),
            external_trace_id: input.external_trace_id.clone(),
            compatibility_mode: input.compatibility_mode.clone(),
            idempotency_key: input.idempotency_key.clone(),
            started_at: input.started_at,
            finished_at: None,
            created_at: input.started_at,
            updated_at: input.started_at,
        };
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        if let (Some(api_key_id), Some(idempotency_key)) =
            (record.api_key_id, record.idempotency_key.as_deref())
        {
            if let Some(existing) = inner
                .flow_runs
                .values()
                .find(|run| {
                    run.application_id == record.application_id
                        && run.api_key_id == Some(api_key_id)
                        && run.idempotency_key.as_deref() == Some(idempotency_key)
                        && run.run_mode == domain::FlowRunMode::PublishedApiRun
                })
                .cloned()
            {
                return Ok(run_service::CreatePublishedFlowRunResult {
                    flow_run: existing,
                    created: false,
                });
            }
        }
        if let (Some(api_key_id), Some(external_user), Some(external_conversation_id)) = (
            record.api_key_id,
            record.external_user.as_ref(),
            record.external_conversation_id.as_ref(),
        ) {
            let key = (
                record.application_id,
                api_key_id,
                external_user.clone(),
                external_conversation_id.clone(),
            );
            if let Some(conversation_id) = inner.conversations.get(&key).map(|record| record.id) {
                inner.run_conversations.insert(record.id, conversation_id);
            }
        }
        inner.flow_runs.insert(record.id, record.clone());
        Ok(run_service::CreatePublishedFlowRunResult {
            flow_run: record,
            created: true,
        })
    }

    async fn find_published_flow_run_by_idempotency_key(
        &self,
        application_id: Uuid,
        api_key_id: Option<Uuid>,
        idempotency_key: &str,
    ) -> Result<Option<domain::FlowRunRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .flow_runs
            .values()
            .find(|run| {
                run.application_id == application_id
                    && run.api_key_id == api_key_id
                    && run.idempotency_key.as_deref() == Some(idempotency_key)
                    && run.run_mode == domain::FlowRunMode::PublishedApiRun
            })
            .cloned())
    }

    async fn append_published_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let events = inner.run_events.entry(input.flow_run_id).or_default();
        let record = domain::RunEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            sequence: (events.len() + 1) as i64,
            event_type: input.event_type.clone(),
            payload: input.payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        events.push(record.clone());
        Ok(record)
    }
}

#[async_trait]
impl run_service::ApplicationPublishedRunControlRepository for ApplicationPublicApiTestRepository {
    async fn get_published_flow_run(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .flow_runs
            .get(&flow_run_id)
            .filter(|run| run.run_mode == domain::FlowRunMode::PublishedApiRun)
            .cloned())
    }

    async fn cancel_published_flow_run(
        &self,
        input: &run_service::CancelPublishedFlowRunInput,
    ) -> Result<crate::ports::CommitFlowRunTerminalReceipt> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let Some(record) = inner.flow_runs.get_mut(&input.flow_run_id) else {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        };
        if record.status != input.from_status
            || matches!(
                record.status,
                domain::FlowRunStatus::Succeeded
                    | domain::FlowRunStatus::Incomplete
                    | domain::FlowRunStatus::Failed
                    | domain::FlowRunStatus::Cancelled
            )
        {
            return Ok(crate::ports::CommitFlowRunTerminalReceipt::Loser);
        }
        record.status = domain::FlowRunStatus::Cancelled;
        record.output_payload = input.output_payload.clone();
        record.error_payload = input.error_payload.clone();
        record.finished_at = Some(input.finished_at);
        let cancelled = record.clone();
        let events = inner.run_events.entry(input.flow_run_id).or_default();
        events.push(domain::RunEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: None,
            sequence: (events.len() + 1) as i64,
            event_type: "flow_run_cancelled".to_string(),
            payload: input.flow_run_event_payload.clone(),
            created_at: input.finished_at,
        });
        Ok(crate::ports::CommitFlowRunTerminalReceipt::Winner(
            cancelled,
        ))
    }

    async fn cancel_published_pending_callback_tasks_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let mut cancelled = Vec::new();
        for task in inner.callback_tasks.values_mut() {
            if task.flow_run_id == flow_run_id && task.status == domain::CallbackTaskStatus::Pending
            {
                task.status = domain::CallbackTaskStatus::Cancelled;
                task.completed_at = Some(completed_at);
                cancelled.push(task.clone());
            }
        }
        Ok(cancelled)
    }

    async fn list_waiting_callback_published_flow_run_ids_for_conversation(
        &self,
        input: &run_service::ListWaitingCallbackPublishedRunsInput,
    ) -> Result<Vec<Uuid>> {
        let mut runs = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .flow_runs
            .values()
            .filter(|run| {
                run.run_mode == domain::FlowRunMode::PublishedApiRun
                    && run.status == domain::FlowRunStatus::WaitingCallback
                    && run.application_id == input.application_id
                    && run.api_key_id == Some(input.api_key_id)
                    && run.external_user.as_deref() == Some(input.external_user.as_str())
                    && run.external_conversation_id.as_deref()
                        == Some(input.external_conversation_id.as_str())
            })
            .map(|run| (run.started_at, run.id))
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        Ok(runs.into_iter().map(|(_, id)| id).collect())
    }

    async fn get_published_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .callback_tasks
            .get(&callback_task_id)
            .cloned()
            .map(|mut task| {
                if task.callback_kind == "llm_tool_calls" {
                    task.request_payload = json!({
                        "tool_calls": task.request_payload.get("tool_calls").cloned()
                    });
                    task.external_ref_payload = None;
                }
                task
            }))
    }

    async fn get_published_run_stream_state(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<run_service::PublishedRunStreamState>> {
        let inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let Some(flow_run) = inner.flow_runs.get(&flow_run_id).filter(|run| {
            run.application_id == application_id
                && run.run_mode == domain::FlowRunMode::PublishedApiRun
        }) else {
            return Ok(None);
        };
        let mut node_runs = inner
            .node_runs
            .values()
            .filter(|node_run| node_run.flow_run_id == flow_run_id)
            .collect::<Vec<_>>();
        node_runs.sort_by(|left, right| {
            left.started_at
                .cmp(&right.started_at)
                .then(left.id.cmp(&right.id))
        });
        let node_usages = node_runs
            .into_iter()
            .map(|node_run| run_service::PublishedRunNodeUsage {
                metrics_usage: node_run.metrics_payload.get("usage").cloned(),
                output_usage: node_run.output_payload.get("usage").cloned(),
            })
            .collect();
        let latest_pending_callback = inner
            .callback_tasks
            .values()
            .filter(|task| {
                task.flow_run_id == flow_run_id
                    && task.status == domain::CallbackTaskStatus::Pending
            })
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then(left.id.cmp(&right.id))
            })
            .map(|task| run_service::PublishedRunPendingCallback {
                id: task.id,
                flow_run_id: task.flow_run_id,
                node_run_id: task.node_run_id,
                callback_kind: task.callback_kind.clone(),
                request_payload: (task.callback_kind != "llm_tool_calls")
                    .then(|| task.request_payload.clone()),
                tool_calls: (task.callback_kind == "llm_tool_calls")
                    .then(|| task.request_payload.get("tool_calls").cloned())
                    .flatten(),
            });

        Ok(Some(run_service::PublishedRunStreamState {
            status: flow_run.status,
            output_payload: flow_run.output_payload.clone(),
            error_payload: flow_run.error_payload.clone(),
            node_usages,
            latest_pending_callback,
        }))
    }
}

#[async_trait]
impl callback_resume::ApplicationPublishedCallbackAttemptRepository
    for ApplicationPublicApiTestRepository
{
    async fn record_published_callback_resume_attempt(
        &self,
        input: &crate::ports::RecordFlowRunCallbackResumeAttemptInput,
    ) -> Result<crate::ports::RecordFlowRunCallbackResumeAttemptOutput> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        if let Some(existing) = inner
            .callback_resume_attempts
            .get(&input.idempotency_key)
            .cloned()
        {
            return Ok(crate::ports::RecordFlowRunCallbackResumeAttemptOutput {
                attempt: existing,
                inserted: false,
            });
        }
        let now = OffsetDateTime::now_utc();
        let attempt = domain::FlowRunCallbackResumeAttemptRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            callback_task_id: input.callback_task_id,
            source: input.source.clone(),
            status: domain::FlowRunCallbackResumeAttemptStatus::Processing,
            response_payload: input.response_payload.clone(),
            idempotency_key: input.idempotency_key.clone(),
            error_payload: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        inner
            .callback_resume_attempts
            .insert(attempt.idempotency_key.clone(), attempt.clone());
        Ok(crate::ports::RecordFlowRunCallbackResumeAttemptOutput {
            attempt,
            inserted: true,
        })
    }

    async fn get_published_callback_resume_attempt(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .callback_resume_attempts
            .values()
            .find(|attempt| attempt.callback_task_id == callback_task_id)
            .cloned())
    }

    async fn finish_published_callback_resume_attempt(
        &self,
        input: &crate::ports::FinishFlowRunCallbackResumeAttemptInput,
    ) -> Result<domain::FlowRunCallbackResumeAttemptRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let Some(attempt) = inner
            .callback_resume_attempts
            .values_mut()
            .find(|attempt| attempt.id == input.attempt_id)
        else {
            anyhow::bail!("callback resume attempt not found");
        };
        attempt.status = input.status;
        attempt.error_payload = input.error_payload.clone();
        attempt.completed_at = Some(input.completed_at);
        attempt.updated_at = OffsetDateTime::now_utc();
        Ok(attempt.clone())
    }

    async fn cancel_published_callback_resume_attempts_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::FlowRunCallbackResumeAttemptRecord>> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let mut cancelled = Vec::new();
        for attempt in inner.callback_resume_attempts.values_mut() {
            if attempt.flow_run_id == flow_run_id
                && matches!(
                    attempt.status,
                    domain::FlowRunCallbackResumeAttemptStatus::Received
                        | domain::FlowRunCallbackResumeAttemptStatus::Processing
                )
            {
                attempt.status = domain::FlowRunCallbackResumeAttemptStatus::Cancelled;
                attempt.completed_at = Some(completed_at);
                attempt.updated_at = OffsetDateTime::now_utc();
                cancelled.push(attempt.clone());
            }
        }
        Ok(cancelled)
    }

    async fn fail_waiting_callback_published_run(
        &self,
        flow_run_id: Uuid,
        error_payload: serde_json::Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let Some(record) = inner.flow_runs.get_mut(&flow_run_id) else {
            return Ok(None);
        };
        if record.status != domain::FlowRunStatus::WaitingCallback {
            return Ok(None);
        }
        record.status = domain::FlowRunStatus::Failed;
        record.error_payload = Some(error_payload);
        record.finished_at = Some(finished_at);
        Ok(Some(record.clone()))
    }

    async fn complete_waiting_callback_published_internal_run(
        &self,
        flow_run_id: Uuid,
        output_payload: serde_json::Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let Some(record) = inner.flow_runs.get_mut(&flow_run_id) else {
            return Ok(None);
        };
        if record.status != domain::FlowRunStatus::WaitingCallback {
            return Ok(None);
        }
        let completed = {
            record.status = domain::FlowRunStatus::Succeeded;
            record.output_payload = output_payload.clone();
            record.error_payload = None;
            record.finished_at = Some(finished_at);
            record.clone()
        };
        for node_run in inner.node_runs.values_mut() {
            if node_run.flow_run_id == flow_run_id
                && node_run.status == domain::NodeRunStatus::WaitingCallback
            {
                node_run.status = domain::NodeRunStatus::Succeeded;
                node_run.output_payload = output_payload.clone();
                node_run.error_payload = None;
                node_run.finished_at = Some(finished_at);
            }
        }
        Ok(Some(completed))
    }
}

#[async_trait]
impl native::NativeRunRepository for ApplicationPublicApiTestRepository {
    async fn create_native_run_result(
        &self,
        run: &native::NativeRunResult,
    ) -> Result<native::NativeRunResult> {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .native_runs
            .insert(run.id, run.clone());
        Ok(run.clone())
    }

    async fn get_native_run_result(&self, run_id: Uuid) -> Result<Option<native::NativeRunResult>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .native_runs
            .get(&run_id)
            .cloned())
    }
}
