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
    pub async fn start_published_flow_run(
        &self,
        command: StartPublishedFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        Box::pin(self.start_published_flow_run_inner(
            command.application_id,
            command.flow_run_id,
            command.provider_transport_slot,
            command.transport_connection_scope,
            command.observation_context,
        ))
        .await
    }

    pub(super) async fn start_published_flow_run_inner(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        provider_transport_slot: Option<crate::ports::ProviderTransportSlotId>,
        transport_connection_scope: Option<String>,
        observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        let flow_run = self
            .repository
            .get_flow_run(application_id, flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found"))?;
        if !matches!(
            flow_run.run_mode,
            domain::FlowRunMode::PublishedApiRun
                | domain::FlowRunMode::AssistantExecution
                | domain::FlowRunMode::WorkflowHttpRun
                | domain::FlowRunMode::WorkflowScheduleRun
        ) {
            return Err(ControlPlaneError::InvalidInput("run_mode").into());
        }
        let actor = ApplicationRepository::load_actor_context_for_user(
            &self.repository,
            flow_run.created_by,
        )
        .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;

        let running = match flow_run.status {
            domain::FlowRunStatus::Queued => {
                ensure_flow_run_transition(
                    domain::FlowRunStatus::Queued,
                    domain::FlowRunStatus::Running,
                    "start_published_flow_run",
                )?;
                self.repository
                    .update_flow_run_if_status(
                        &UpdateFlowRunInput {
                            flow_run_id: flow_run.id,
                            status: domain::FlowRunStatus::Running,
                            output_payload: flow_run.output_payload.clone(),
                            error_payload: flow_run.error_payload.clone(),
                            finished_at: None,
                        },
                        domain::FlowRunStatus::Queued,
                    )
                    .await?
            }
            domain::FlowRunStatus::Running => Some(flow_run.clone()),
            _ => None,
        };
        let Some(running) = running else {
            return self
                .repository
                .get_application_run_detail(application_id, flow_run.id)
                .await?
                .ok_or_else(|| anyhow!("flow run detail not found"));
        };

        let execution_started_event = if running.run_mode == domain::FlowRunMode::AssistantExecution
        {
            "assistant_execution_started"
        } else {
            "public_run_execution_started"
        };
        self.repository
            .append_run_event(&AppendRunEventInput {
                flow_run_id: running.id,
                node_run_id: None,
                event_type: execution_started_event.to_string(),
                payload: json!({
                    "api_key_id": running.api_key_id,
                    "application_id": running.application_id,
                    "publication_version_id": running.publication_version_id,
                    "creator_user_id": running.created_by,
                    "external_user": running.external_user,
                    "external_conversation_id": running.external_conversation_id,
                    "external_trace_id": running.external_trace_id,
                    "compatibility_mode": running.compatibility_mode,
                }),
            })
            .await?;
        let flow_started_event = debug_stream_events::flow_started(running.id);
        if let Err(error) = runtime_event_persister::persist_runtime_event_payload(
            &self.repository,
            running.id,
            &flow_started_event,
        )
        .await
        {
            tracing::warn!(
                flow_run_id = %running.id,
                event_type = %flow_started_event.event_type,
                error = %error,
                "failed to persist published flow runtime start event"
            );
        }
        if let Some(stream) = &self.runtime_event_stream {
            let _ = stream.append(running.id, flow_started_event).await;
        }

        let continuation = ContinueFlowDebugRunCommand {
            application_id,
            flow_run_id: running.id,
            workspace_id: application.workspace_id,
        };
        // Keep the slot available until the complete execution segment has exhausted Provider
        // retries. A disconnected SSE receiver does not own this background execution lifetime.
        let provider_transport_payload = self
            .resolve_provider_transport_payload(&running, provider_transport_slot)
            .await?;
        let result = live_debug_run::continue_flow_debug_run_with_provider_transport(
            self,
            continuation,
            provider_transport_payload,
            transport_connection_scope,
            observation_context,
        )
        .await;
        if let Some(slot_id) = provider_transport_slot {
            self.delete_provider_transport_slot(slot_id).await;
        }
        let detail = match result {
            Ok(detail) => detail,
            Err(error) => {
                if let Ok(Some(failed)) = self
                    .repository
                    .get_flow_run(application_id, running.id)
                    .await
                {
                    self.append_published_terminal_audit(&application, &failed)
                        .await;
                }
                return Err(error);
            }
        };
        self.append_published_terminal_audit(&application, &detail.flow_run)
            .await;
        Ok(detail)
    }

    pub async fn continue_flow_debug_run_with_live_provider_events(
        &self,
        command: ContinueFlowDebugRunCommand,
        live_provider_events: LiveProviderStreamEventSender,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        live_debug_run::continue_flow_debug_run_with_live_provider_events(
            self,
            command,
            live_provider_events,
        )
        .await
    }

    pub async fn cancel_flow_run(
        &self,
        command: CancelFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail> {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        live_debug_run::cancel_flow_run(self, command, &context).await
    }

    pub async fn resume_flow_run(
        &self,
        command: ResumeFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        let flow_run = self
            .repository
            .get_flow_run(command.application_id, command.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found"))?;
        let checkpoint = self
            .repository
            .get_checkpoint(command.flow_run_id, command.checkpoint_id)
            .await?
            .ok_or_else(|| anyhow!("checkpoint not found"))?;
        let current_detail = self
            .repository
            .get_application_run_detail(command.application_id, command.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run detail not found"))?;
        let ApplicationRunContext { actor, application } = context;
        let compiled_plan_id = flow_run
            .compiled_plan_id
            .ok_or_else(|| anyhow!("flow run compiled plan is not attached"))?;
        let compiled_record = self
            .load_frozen_compiled_plan(compiled_plan_id)
            .await?
            .ok_or_else(|| anyhow!("compiled plan not found"))?;
        let compiled_plan: orchestration_runtime::compiled_plan::CompiledPlan =
            serde_json::from_value(compiled_record.plan.clone())?;
        ensure_compiled_plan_runnable(&compiled_plan)?;
        let snapshot = checkpoint_snapshot_from_record_with_context(
            &self.repository,
            &checkpoint,
            flow_run.id,
        )
        .await?;
        let waiting_node_id = checkpoint_node_id(&checkpoint)?;
        let resume_patch = command
            .input_payload
            .as_object()
            .and_then(|payload| payload.get(&waiting_node_id))
            .cloned()
            .ok_or_else(|| anyhow!("resume payload is missing node input for {waiting_node_id}"))?;
        let waiting_node_resume = if let Some(node_run_id) = checkpoint.node_run_id {
            let waiting_node = current_detail
                .node_runs
                .iter()
                .find(|record| record.id == node_run_id)
                .ok_or_else(|| anyhow!("waiting node run not found for checkpoint"))?;
            Some(WaitingNodeResumeUpdate {
                node_run_id,
                from_status: waiting_node.status,
                output_payload: resume_patch.clone(),
                metrics_payload: json!({ "resumed": true }),
                debug_payload: json!({}),
            })
        } else {
            None
        };
        ensure_flow_run_transition(
            flow_run.status,
            domain::FlowRunStatus::WaitingHuman,
            "resume_flow_run",
        )?;
        if let Some(waiting_node_resume) = &waiting_node_resume {
            ensure_node_run_transition(
                waiting_node_resume.from_status,
                domain::NodeRunStatus::WaitingHuman,
                "resume_flow_run",
            )?;
        }
        let claim = self
            .repository
            .acquire_resume_claim(&AcquireResumeClaimInput {
                scope_id: application.workspace_id,
                application_id: command.application_id,
                flow_run_id: flow_run.id,
                checkpoint_id: checkpoint.id,
                callback_task_id: None,
                kind: ResumeClaimKind::Human,
                request_payload: command.input_payload.clone(),
            })
            .await?;
        if claim.disposition != ResumeClaimDisposition::Acquired {
            return Ok(current_detail);
        }
        let mut communication_events = None;
        let result = async {
            communication_events = self.begin_communication_events(&flow_run).await?;
            let execution = self
                .resume_execution_segment(ResumeExecutionSegmentInput {
                    transport_connection_scope: None,
                    observation_context: None,
                    resumed_node_run: None,
                    native_transport: None,
                    response_round_id: None,
                    resume_claim_id: claim.claim.id,
                    resume_claim_owner: Some(RenewResumeClaimInput {
                        claim_id: claim.claim.id,
                        claim_token: claim.claim.claim_token,
                        expected_generation: claim.claim.generation,
                    }),
                    actor: &actor,
                    application: &application,
                    flow_run: &flow_run,
                    compiled_plan: &compiled_plan,
                    snapshot: &snapshot,
                    waiting_node_id: &waiting_node_id,
                    waiting_node_run_id: checkpoint.node_run_id,
                    resume_payload: &resume_patch,
                })
                .await?;

            self.persist_flow_debug_outcome(PersistFlowDebugOutcomeInput {
                responses_round: None,
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
                    "checkpoint_id": checkpoint.id,
                    "input_payload": command.input_payload,
                }),
                base_started_at: next_node_started_at(&current_detail),
                waiting_node_resume,
                resume_claim_id: Some(claim.claim.id),
                resume_claim_token: Some(claim.claim.claim_token),
                tool_delivery_events: execution.tool_delivery_events.clone(),
            })
            .await
        }
        .await;
        let finish = FinishResumeClaimInput {
            claim_id: claim.claim.id,
            claim_token: claim.claim.claim_token,
            expected_generation: claim.claim.generation,
            status: if result.is_ok() {
                ResumeClaimStatus::Succeeded
            } else {
                ResumeClaimStatus::Failed
            },
            error_payload: result
                .as_ref()
                .err()
                .map(|error| json!({ "message": error.to_string() })),
            completed_at: OffsetDateTime::now_utc(),
        };
        self.repository.finish_resume_claim(&finish).await?;
        if result.is_ok() {
            if let Err(error) = self
                .provider_transport_store
                .delete_continuation(crate::ports::ProviderContinuationSlotId::for_resume_claim(
                    flow_run.id,
                    claim.claim.id,
                ))
                .await
            {
                tracing::warn!(flow_run_id = %flow_run.id, error = %error, "resume claim continuation cleanup failed");
            }
        }
        if let Some(events) = communication_events {
            events.finish().await;
        }
        result
    }
}
