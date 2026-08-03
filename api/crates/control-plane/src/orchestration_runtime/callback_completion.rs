use super::*;

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: ApplicationRepository
        + FlowRepository
        + OrchestrationRuntimeRepository
        + ModelDefinitionRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone,
{
    pub async fn complete_callback_task(
        &self,
        command: CompleteCallbackTaskCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: crate::ports::FileManagementRepository,
    {
        let application_id = command.application_id;
        let flow_run = self.complete_callback_task_run(command).await?;
        self.repository
            .get_application_run_detail(application_id, flow_run.id)
            .await?
            .ok_or_else(|| anyhow!("flow run detail not found"))
    }

    pub(crate) async fn complete_callback_task_run(
        &self,
        mut command: CompleteCallbackTaskCommand,
    ) -> Result<domain::FlowRunRecord>
    where
        R: crate::ports::FileManagementRepository,
    {
        command.response_payload = escape_json_nul_characters(command.response_payload);
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        let ApplicationRunContext { actor, application } = context;
        let resume_context = self
            .repository
            .get_callback_resume_context(command.application_id, command.callback_task_id)
            .await?;
        let resume_context = match resume_context {
            Some(context) => context,
            None => {
                if self
                    .repository
                    .get_callback_task(command.callback_task_id)
                    .await?
                    .is_some()
                {
                    return Err(anyhow!("flow run not found for callback task"));
                }
                return Err(ControlPlaneError::NotFound("callback_task").into());
            }
        };
        let pending_callback_task = &resume_context.callback_task;
        if pending_callback_task.callback_kind == "data_model_side_effect_confirmation" {
            let confirmation_payload = pending_callback_task
                .external_ref_payload
                .as_ref()
                .unwrap_or(&pending_callback_task.request_payload);
            ensure_data_model_side_effect_confirmation_approved(&command.response_payload)?;
            ensure_data_model_side_effect_confirmation_metadata(&actor, confirmation_payload)?;
        }
        if pending_callback_task.callback_kind == "llm_tool_calls" {
            ensure_llm_tool_callback_results_complete(
                &pending_callback_task.request_payload,
                &command.response_payload,
            )?;
        }
        let checkpoint = resume_context.checkpoint;
        let flow_run = resume_context.flow_run;
        let waiting_node = resume_context.waiting_node;
        let base_started_at = resume_context.next_node_started_at;
        let compiled_plan_id = flow_run
            .compiled_plan_id
            .ok_or_else(|| anyhow!("flow run compiled plan is not attached"))?;
        let compiled_record = self
            .repository
            .get_compiled_plan(compiled_plan_id)
            .await?
            .ok_or_else(|| anyhow!("compiled plan not found"))?;
        let compiled_plan: orchestration_runtime::compiled_plan::CompiledPlan =
            serde_json::from_value(compiled_record.plan.clone())?;
        ensure_compiled_plan_runnable(&compiled_plan)?;
        let callback_task = self
            .repository
            .complete_callback_task(&CompleteCallbackTaskInput {
                callback_task_id: command.callback_task_id,
                response_payload: command.response_payload.clone(),
                completed_at: OffsetDateTime::now_utc(),
            })
            .await?;
        if callback_task.callback_kind == "data_model_side_effect_confirmation" {
            return self
                .complete_data_model_side_effect_callback(
                    command,
                    &actor,
                    &callback_task,
                    &waiting_node,
                    base_started_at,
                    &application,
                    &checkpoint,
                    &flow_run,
                    &compiled_plan,
                )
                .await;
        }
        let snapshot = checkpoint_snapshot_from_record(&checkpoint)?;
        let waiting_node_id = checkpoint_node_id(&checkpoint)?;
        let execution = self
            .resume_execution_segment(ResumeExecutionSegmentInput {
                actor: &actor,
                application: &application,
                flow_run: &flow_run,
                compiled_plan: &compiled_plan,
                snapshot: &snapshot,
                waiting_node_id: &waiting_node_id,
                waiting_node_run_id: Some(callback_task.node_run_id),
                resume_payload: &command.response_payload,
            })
            .await?;
        let waiting_node_output_payload = if callback_task.callback_kind == "llm_tool_calls" {
            waiting_node.output_payload.clone()
        } else {
            callback_task
                .response_payload
                .clone()
                .ok_or_else(|| anyhow!("completed callback task is missing response payload"))?
        };

        self.persist_flow_debug_outcome_record(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run: &flow_run,
            compiled_plan: Some(&compiled_plan),
            outcome: &execution.outcome,
            prepared_node_runs: Some(&execution.prepared_node_runs),
            answer_presentation: execution.answer_presentation.as_ref(),
            trigger_event_type: "flow_run_resumed",
            trigger_event_payload: json!({
                "callback_task_id": callback_task.id,
                "response_payload": command.response_payload,
            }),
            base_started_at,
            waiting_node_resume: Some(WaitingNodeResumeUpdate {
                node_run_id: callback_task.node_run_id,
                from_status: waiting_node.status,
                output_payload: waiting_node_output_payload,
                metrics_payload: json!({
                    "resumed": true,
                    "callback_kind": callback_task.callback_kind,
                }),
                debug_payload: json!({
                    "callback_task_id": callback_task.id,
                    "callback_kind": callback_task.callback_kind,
                }),
            }),
        })
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn complete_data_model_side_effect_callback(
        &self,
        command: CompleteCallbackTaskCommand,
        actor: &domain::ActorContext,
        callback_task: &domain::CallbackTaskRecord,
        waiting_node: &CallbackResumeWaitingNode,
        base_started_at: OffsetDateTime,
        application: &domain::ApplicationRecord,
        checkpoint: &domain::CheckpointRecord,
        flow_run: &domain::FlowRunRecord,
        compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    ) -> Result<domain::FlowRunRecord>
    where
        R: crate::ports::FileManagementRepository,
    {
        let waiting_node_id = checkpoint_node_id(checkpoint)?;
        let node = compiled_plan
            .nodes
            .get(&waiting_node_id)
            .ok_or_else(|| anyhow!("waiting data_model node not found in compiled plan"))?;
        let confirmation_payload = callback_task
            .external_ref_payload
            .as_ref()
            .unwrap_or(&callback_task.request_payload);
        let execution = data_model_runtime::execute_confirmed_data_model_side_effect(
            self.repository.clone(),
            self.runtime_engine.clone(),
            actor,
            node,
            &data_model_runtime::DataModelRunContext {
                workspace_id: application.workspace_id,
                application_id: command.application_id,
                draft_id: flow_run.draft_id,
                flow_run_id: flow_run.id,
                node_run_id: callback_task.node_run_id,
            },
            confirmation_payload,
        )
        .await;

        if let Some(error_payload) = execution.error_payload.clone() {
            ensure_node_run_transition(
                waiting_node.status,
                domain::NodeRunStatus::Failed,
                "complete_data_model_side_effect_callback",
            )?;
            self.repository
                .update_node_run(&UpdateNodeRunInput {
                    node_run_id: callback_task.node_run_id,
                    status: domain::NodeRunStatus::Failed,
                    output_payload: json!({}),
                    error_payload: Some(error_payload.clone()),
                    metrics_payload: execution.metrics_payload,
                    debug_payload: json!({
                        "callback_task_id": callback_task.id,
                        "callback_kind": callback_task.callback_kind,
                    }),
                    finished_at: Some(OffsetDateTime::now_utc()),
                })
                .await?;
            ensure_flow_run_transition(
                flow_run.status,
                domain::FlowRunStatus::Failed,
                "complete_data_model_side_effect_callback",
            )?;
            let terminal_event =
                debug_stream_events::flow_failed(flow_run.id, error_payload.clone());
            let receipt = self
                .repository
                .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                    flow_run_id: flow_run.id,
                    expected_status: flow_run.status,
                    result: CommitFlowRunTerminalResult::Failed {
                        output_payload: flow_run.output_payload.clone(),
                        error_payload: error_payload.clone(),
                    },
                    flow_run_event_payload: error_payload,
                    terminal_event_payload: terminal_event.payload,
                    finished_at: OffsetDateTime::now_utc(),
                })
                .await?;
            let failed_flow_run = match stream_terminal_recovery::resolve_terminal_commit(
                &self.repository,
                command.application_id,
                flow_run.id,
                receipt,
            )
            .await?
            {
                stream_terminal_recovery::TerminalCommitResolution::Winner(flow_run)
                | stream_terminal_recovery::TerminalCommitResolution::Loser(flow_run) => flow_run,
            };
            live_debug_run::project_committed_terminal(self, &failed_flow_run).await;
            return Ok(failed_flow_run);
        }

        let snapshot = checkpoint_snapshot_from_record(checkpoint)?;
        let resumed_execution = self
            .resume_execution_segment(ResumeExecutionSegmentInput {
                actor,
                application,
                flow_run,
                compiled_plan,
                snapshot: &snapshot,
                waiting_node_id: &waiting_node_id,
                waiting_node_run_id: Some(callback_task.node_run_id),
                resume_payload: &execution.output_payload,
            })
            .await?;
        let side_effect_receipt = execution
            .metrics_payload
            .get("side_effect_receipt")
            .cloned()
            .unwrap_or(Value::Null);

        self.persist_flow_debug_outcome_record(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run,
            compiled_plan: Some(compiled_plan),
            outcome: &resumed_execution.outcome,
            prepared_node_runs: Some(&resumed_execution.prepared_node_runs),
            answer_presentation: resumed_execution.answer_presentation.as_ref(),
            trigger_event_type: "data_model_side_effect_confirmed",
            trigger_event_payload: json!({
                "callback_task_id": callback_task.id,
                "response_payload": command.response_payload,
                "side_effect_receipt": side_effect_receipt,
            }),
            base_started_at,
            waiting_node_resume: Some(WaitingNodeResumeUpdate {
                node_run_id: callback_task.node_run_id,
                from_status: waiting_node.status,
                output_payload: persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    None,
                    &json!({
                        "callback_task_id": callback_task.id,
                        "callback_kind": callback_task.callback_kind,
                        "confirmed": true,
                    }),
                ),
                metrics_payload: execution.metrics_payload,
                debug_payload: json!({
                    "callback_task_id": callback_task.id,
                    "callback_kind": callback_task.callback_kind,
                    "confirmed": true,
                }),
            }),
        })
        .await
    }
}
