use super::*;

impl ApplicationPublicApiTestRepository {
    pub async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        FlowRepository::get_or_create_editor_state(
            self,
            workspace_id,
            application_id,
            actor_user_id,
        )
        .await
    }

    pub async fn get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        ApplicationCompiledPlanRepository::get_application_compiled_plan(self, compiled_plan_id)
            .await
    }

    pub fn set_active_publication_document_snapshot(
        &self,
        application_id: Uuid,
        document_snapshot: serde_json::Value,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let publication = inner
            .publications
            .values_mut()
            .find(|publication| publication.application_id == application_id && publication.active)
            .expect("active publication fixture must exist");
        publication.document_snapshot = document_snapshot;
    }

    pub async fn get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .flow_runs
            .get(&flow_run_id)
            .filter(|run| run.application_id == application_id)
            .cloned())
    }

    pub fn get_node_run(&self, node_run_id: Uuid) -> Option<domain::NodeRunRecord> {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .node_runs
            .get(&node_run_id)
            .cloned()
    }

    pub fn seed_provider_response_id(&self, flow_run_id: Uuid, provider_response_id: &str) {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let node_run_id = Uuid::now_v7();
        inner.node_runs.insert(
            node_run_id,
            domain::NodeRunRecord {
                id: node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({ "response_id": provider_response_id }),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({}),
                started_at: OffsetDateTime::now_utc(),
                finished_at: Some(OffsetDateTime::now_utc()),
            },
        );
    }

    pub fn clear_native_run_results(&self) {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .native_runs
            .clear();
    }

    pub fn set_flow_run_compatibility_mode_for_test(
        &self,
        flow_run_id: Uuid,
        compatibility_mode: Option<&str>,
    ) {
        if let Some(flow_run) = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .flow_runs
            .get_mut(&flow_run_id)
        {
            flow_run.compatibility_mode = compatibility_mode.map(ToOwned::to_owned);
        }
    }

    pub fn conversation_record_id_for_run(&self, flow_run_id: Uuid) -> Option<Uuid> {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .run_conversations
            .get(&flow_run_id)
            .copied()
    }

    pub fn seed_pending_callback_task(&self, flow_run_id: Uuid) -> domain::CallbackTaskRecord {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let node_run_id = Uuid::now_v7();
        if let Some(flow_run) = inner.flow_runs.get_mut(&flow_run_id) {
            flow_run.status = domain::FlowRunStatus::WaitingCallback;
        }
        inner.node_runs.insert(
            node_run_id,
            domain::NodeRunRecord {
                id: node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM".to_string(),
                status: domain::NodeRunStatus::WaitingCallback,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({}),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({}),
                started_at: OffsetDateTime::now_utc(),
                finished_at: None,
            },
        );
        let task = domain::CallbackTaskRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            node_run_id,
            callback_kind: "external_callback".to_string(),
            status: domain::CallbackTaskStatus::Pending,
            request_payload: serde_json::json!({ "prompt": "approve" }),
            response_payload: None,
            external_ref_payload: None,
            created_at: OffsetDateTime::now_utc(),
            completed_at: None,
        };
        inner.callback_tasks.insert(task.id, task.clone());
        task
    }

    pub fn seed_pending_llm_tool_callback_task(
        &self,
        flow_run_id: Uuid,
        request_payload: serde_json::Value,
    ) -> domain::CallbackTaskRecord {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let node_run_id = Uuid::now_v7();
        if let Some(flow_run) = inner.flow_runs.get_mut(&flow_run_id) {
            flow_run.status = domain::FlowRunStatus::WaitingCallback;
        }
        inner.node_runs.insert(
            node_run_id,
            domain::NodeRunRecord {
                id: node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM".to_string(),
                status: domain::NodeRunStatus::WaitingCallback,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({}),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({}),
                started_at: OffsetDateTime::now_utc(),
                finished_at: None,
            },
        );
        let task = domain::CallbackTaskRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Pending,
            request_payload,
            response_payload: None,
            external_ref_payload: None,
            created_at: OffsetDateTime::now_utc(),
            completed_at: None,
        };
        inner.callback_tasks.insert(task.id, task.clone());
        task
    }

    pub fn run_event_types(&self, flow_run_id: Uuid) -> Vec<String> {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .run_events
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|event| event.event_type)
            .collect()
    }

    pub fn run_events(&self, flow_run_id: Uuid) -> Vec<domain::RunEventRecord> {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .run_events
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn callback_resume_attempts(&self) -> Vec<domain::FlowRunCallbackResumeAttemptRecord> {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .callback_resume_attempts
            .values()
            .cloned()
            .collect()
    }

    pub fn complete_callback_task_for_test(&self, callback_task_id: Uuid) {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let task = inner
            .callback_tasks
            .get_mut(&callback_task_id)
            .expect("callback task fixture must exist");
        task.status = domain::CallbackTaskStatus::Completed;
        task.completed_at = Some(OffsetDateTime::now_utc());
    }

    pub fn flow_run_count(&self) -> usize {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .flow_runs
            .len()
    }

    pub fn conversation_count(&self) -> usize {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .conversations
            .len()
    }

    pub fn reset_editor_state_read_count(&self) {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .editor_state_read_count = 0;
    }

    pub fn editor_state_read_count(&self) -> usize {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .editor_state_read_count
    }
}
