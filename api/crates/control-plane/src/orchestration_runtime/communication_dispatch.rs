//! Durable initial-dispatch ownership. Queued work can restart; uncertain running work is
//! finalized through the existing terminal CAS, never replayed against external tools/models.
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
    pub async fn dispatch_communication_run(
        &self,
        command: StartPublishedFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        let Some(_lease) = self
            .repository
            .try_acquire_flow_run_dispatch_lease(command.application_id, command.flow_run_id)
            .await?
        else {
            return self
                .communication_dispatch_detail(command.application_id, command.flow_run_id)
                .await;
        };
        let run = self
            .communication_dispatch_run(command.application_id, command.flow_run_id)
            .await?;
        match run.status {
            domain::FlowRunStatus::Queued => {
                // This call is the only initial-execution owner while the PG session lease
                // lives. Its internal Queued->Running CAS remains the durable start fact.
                let result = self.start_published_flow_run(command).await;
                if result.is_err() {
                    self.finalize_communication_dispatch_failure(
                        run.application_id,
                        run.id,
                        "communication_dispatch_failed",
                    )
                    .await?;
                    return self
                        .communication_dispatch_detail(run.application_id, run.id)
                        .await;
                }
                result
            }
            domain::FlowRunStatus::Running => {
                // Acquiring the lock proves no initial executor remains. Existing resume
                // claims own their own recovery protocol, even after their lease expires.
                self.finalize_communication_dispatch_failure(
                    run.application_id,
                    run.id,
                    "communication_dispatch_interrupted",
                )
                .await?;
                self.communication_dispatch_detail(run.application_id, run.id)
                    .await
            }
            _ => {
                self.communication_dispatch_detail(run.application_id, run.id)
                    .await
            }
        }
    }

    /// Failed live configuration/authority checks cannot leave an undispatched queue entry.
    /// Never supersedes an active initial executor or an established resume lifecycle.
    pub async fn reject_communication_dispatch(
        &self,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<()> {
        let Some(_lease) = self
            .repository
            .try_acquire_flow_run_dispatch_lease(application_id, run_id)
            .await?
        else {
            return Ok(());
        };
        self.communication_dispatch_run(application_id, run_id)
            .await?;
        self.finalize_communication_dispatch_failure(
            application_id,
            run_id,
            "communication_dispatch_preflight_failed",
        )
        .await
    }

    async fn communication_dispatch_run(
        &self,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<domain::FlowRunRecord> {
        let run = self
            .repository
            .get_flow_run(application_id, run_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        if run.run_mode != domain::FlowRunMode::AssistantExecution
            || run.compatibility_mode.as_deref() != Some("communication")
        {
            return Err(ControlPlaneError::InvalidInput("communication_run_mode").into());
        }
        Ok(run)
    }
    async fn communication_dispatch_detail(
        &self,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<domain::ApplicationRunDetail> {
        self.repository
            .get_application_run_detail(application_id, run_id)
            .await?
            .ok_or_else(|| anyhow!("communication run detail missing"))
    }
    async fn finalize_communication_dispatch_failure(
        &self,
        application_id: Uuid,
        run_id: Uuid,
        code: &str,
    ) -> Result<()> {
        let run = self
            .communication_dispatch_run(application_id, run_id)
            .await?;
        if !matches!(
            run.status,
            domain::FlowRunStatus::Queued | domain::FlowRunStatus::Running
        ) || self.repository.flow_run_has_resume_claims(run_id).await?
        {
            return Ok(());
        }
        let error = json!({"code":code,"message":"Communication execution could not complete its initial dispatch"});
        let terminal = debug_stream_events::flow_failed(run.id, error.clone());
        let receipt = self
            .repository
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: run.id,
                expected_status: run.status,
                require_initial_execution: true,
                result: CommitFlowRunTerminalResult::Failed {
                    output_payload: run.output_payload,
                    error_payload: error.clone(),
                },
                flow_run_event_payload: error,
                terminal_event_payload: terminal.payload,
                finished_at: OffsetDateTime::now_utc(),
            })
            .await?;
        let winner = match stream_terminal_recovery::resolve_terminal_commit(
            &self.repository,
            application_id,
            run_id,
            receipt,
        )
        .await?
        {
            stream_terminal_recovery::TerminalCommitResolution::Winner(run)
            | stream_terminal_recovery::TerminalCommitResolution::Loser(run) => run,
        };
        self.ensure_durable_terminal_projection(&winner).await
    }
}
