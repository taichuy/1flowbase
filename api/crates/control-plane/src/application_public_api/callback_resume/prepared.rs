use super::*;
use crate::{
    capability_plugin_runtime::CapabilityPluginRuntimePort,
    orchestration_runtime::callback_completion::{PreparedCallbackExecution, PreparedCallbackTask},
    ports::{FinishResumeClaimInput, RenewResumeClaimInput, ResumeClaimStatus},
};
use std::{future::Future, pin::Pin};
use tokio::{sync::watch, task::JoinHandle, time::Instant};

type Cleanup = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>;

pub enum PreparedPublishedCallback {
    Execution(Box<PreparedPublishedCallbackExecution>),
    NoExecution(Box<ResumePublishedCallbackResult>),
}

/// A single execution claim owner. Move it from host setup into execution; do not clone.
/// Dropping it cancels renewal and fences cleanup by the acquired token/generation.
pub struct PreparedPublishedCallbackExecution {
    execution: Option<PreparedCallbackExecution>,
    command: ResumePublishedCallbackCommand,
    flow_run: domain::FlowRunRecord,
    callback_task: domain::CallbackTaskRecord,
    attempt: domain::FlowRunCallbackResumeAttemptRecord,
    claim_id: Uuid,
    claim_generation: i64,
    lease: CallbackExecutionLease,
}

struct CallbackExecutionLease {
    lost: watch::Receiver<bool>,
    heartbeat: JoinHandle<()>,
    deadline: Arc<std::sync::Mutex<Instant>>,
    cleanup: Option<Cleanup>,
}

impl CallbackExecutionLease {
    async fn while_owned<F: Future>(&self, future: F) -> Result<F::Output> {
        let mut lost = self.lost.clone();
        tokio::select! {
            biased;
            _ = async {
                loop {
                    let deadline = *self.deadline.lock().expect("callback lease deadline poisoned");
                    if *lost.borrow() || Instant::now() >= deadline { return; }
                    tokio::select! {
                        _ = tokio::time::sleep_until(deadline) => {},
                        changed = lost.changed() => {
                            if changed.is_err() || *lost.borrow_and_update() { return; }
                        }
                    }
                }
            } => Err(ControlPlaneError::Conflict("resume_claim_not_owned").into()),
            result = future => Ok(result),
        }
    }

    fn disarm(&mut self) {
        self.heartbeat.abort();
        self.cleanup = None;
    }

    async fn abort(mut self) -> Result<()> {
        self.heartbeat.abort();
        match self.cleanup.take() {
            Some(cleanup) => cleanup().await,
            None => Ok(()),
        }
    }
}

impl Drop for CallbackExecutionLease {
    fn drop(&mut self) {
        self.heartbeat.abort();
        if let Some(cleanup) = self.cleanup.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    if let Err(error) = cleanup().await {
                        tracing::warn!(%error, "prepared callback claim cleanup did not retain ownership");
                    }
                });
            }
        }
    }
}

impl PreparedPublishedCallbackExecution {
    pub fn flow_run_id(&self) -> Uuid {
        self.flow_run.id
    }
    pub fn callback_task_id(&self) -> Uuid {
        self.callback_task.id
    }
    pub fn claim_id(&self) -> Uuid {
        self.claim_id
    }
    pub fn claim_generation(&self) -> i64 {
        self.claim_generation
    }

    /// Race every awaited host setup operation against lease loss. Output is the
    /// future's unchanged output, so a fallible setup produces nested Results.
    pub async fn while_owned<F: Future>(&self, future: F) -> Result<F::Output> {
        self.lease.while_owned(future).await
    }

    pub async fn abort(self) -> Result<()> {
        self.lease.abort().await
    }
}

impl<R, H> ApplicationPublishedCallbackResumeService<R, OrchestrationRuntimeService<R, H>>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublishedFlowRunRepository
        + ApplicationPublishedRunControlRepository
        + ApplicationPublishedCallbackAttemptRepository
        + crate::ports::BillingRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone,
{
    pub async fn prepare_reserved_native_callback_for_actor(
        &self,
        actor: super::super::api_keys::ApplicationApiKeyActor,
        command: ResumePublishedCallbackCommand,
        receipt: PublishedCallbackReservationReceipt,
    ) -> Result<PreparedPublishedCallback> {
        let context = self
            .resolve_resume_context_for_actor(actor, &command)
            .await?;
        let attempt = self
            .repository
            .get_published_callback_resume_attempt(context.callback_task.id)
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "callback_resume_reservation_missing",
            ))?;
        if attempt.id != receipt.attempt_id()
            || attempt.response_payload != command.response_payload
            || attempt.source != command.source.as_str()
        {
            return Err(ControlPlaneError::Conflict("callback_resume_reservation_mismatch").into());
        }
        let admission = receipt.admission_fence();
        let prepared = self
            .consumer
            .prepare_callback_task_execution(
                CompleteCallbackTaskCommand {
                    responses_continuation: command.responses_continuation.clone(),
                    transport_connection_scope: command.transport_connection_scope.clone(),
                    observation_context: command.observation_context.clone(),
                    native_transport: command.native_transport.clone(),
                    actor_user_id: context.actor.creator_user_id,
                    application_id: context.actor.application_id,
                    callback_task_id: context.callback_task.id,
                    response_payload: command.response_payload.clone(),
                },
                Some(&admission),
            )
            .await?;
        let execution = match prepared {
            PreparedCallbackTask::NoExecution(run) => {
                let callback = self
                    .repository
                    .get_published_callback_task(context.callback_task.id)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("callback_task"))?;
                if callback.status == domain::CallbackTaskStatus::Pending {
                    self.release_unstarted_native_callback(&receipt).await?;
                    let attempt = self
                        .repository
                        .get_published_callback_resume_attempt(callback.id)
                        .await?
                        .ok_or(ControlPlaneError::Conflict(
                            "callback_resume_reservation_missing",
                        ))?;
                    return Ok(PreparedPublishedCallback::NoExecution(Box::new(
                        ResumePublishedCallbackResult {
                            run: self.native_result_for_flow_run(&run).await?,
                            attempt,
                        },
                    )));
                }
                let result = self
                    .finish_callback_resume_result(
                        context.flow_run,
                        callback,
                        attempt,
                        &command,
                        Ok(*run),
                    )
                    .await?;
                return Ok(PreparedPublishedCallback::NoExecution(Box::new(result)));
            }
            PreparedCallbackTask::Execution(execution) => execution,
        };
        // No await separates actual acquisition from starting its renewable owner.
        let renew = execution.renew_input();
        let claim = execution.claim();
        let deadline = Instant::now()
            + (claim.lease_expires_at - OffsetDateTime::now_utc())
                .try_into()
                .unwrap_or(std::time::Duration::ZERO);
        let claim_id = claim.id;
        let claim_generation = claim.generation;
        let lease = self.callback_execution_lease(renew, deadline, receipt);
        let owner = PreparedPublishedCallbackExecution {
            execution: Some(*execution),
            command,
            flow_run: context.flow_run,
            callback_task: context.callback_task,
            attempt,
            claim_id,
            claim_generation,
            lease,
        };
        owner.while_owned(self.append_resume_event(owner.flow_run.id, Some(owner.callback_task.node_run_id),
            "public_run_resume_requested", json!({
                "callback_task_id":owner.callback_task.id,"resume_attempt_id":owner.attempt.id,
                "source":owner.command.source.as_str(),"response_mode":owner.command.response_mode,
                "response_payload":owner.command.response_payload,
            }))).await??;
        Ok(PreparedPublishedCallback::Execution(Box::new(owner)))
    }

    pub async fn execute_prepared_callback(
        &self,
        mut owner: PreparedPublishedCallbackExecution,
    ) -> Result<ResumePublishedCallbackResult> {
        let execution = owner
            .execution
            .take()
            .expect("prepared callback executes exactly once");
        let result = owner
            // Match the common callback entry: keep execution state out of both
            // lease wrappers and their callers' ordinary Tokio worker stacks.
            .while_owned(Box::pin(
                self.consumer.execute_prepared_callback_task(execution),
            ))
            .await?;
        if result
            .as_ref()
            .err()
            .and_then(|error| error.downcast_ref::<ControlPlaneError>())
            .is_some_and(|error| {
                matches!(error, ControlPlaneError::Conflict("resume_claim_not_owned"))
            })
        {
            // A replacement execution must never receive this owner's unfenced
            // legacy result finalization. Drop performs only token/version CAS cleanup.
            return Err(result.expect_err("ownership failure was checked"));
        }
        // The execution method has fenced and settled its claim. A later projection
        // failure must not re-admit already executed work through Drop cleanup.
        owner.lease.disarm();
        self.finish_callback_resume_result(
            owner.flow_run,
            owner.callback_task,
            owner.attempt,
            &owner.command,
            result,
        )
        .await
    }

    fn callback_execution_lease(
        &self,
        renew: RenewResumeClaimInput,
        initial_deadline: Instant,
        receipt: PublishedCallbackReservationReceipt,
    ) -> CallbackExecutionLease {
        let (lost_sender, lost) = watch::channel(false);
        let repository = self.repository.clone();
        let heartbeat_renew = renew.clone();
        let shared_deadline = Arc::new(std::sync::Mutex::new(initial_deadline));
        let heartbeat_deadline = shared_deadline.clone();
        let heartbeat = tokio::spawn(async move {
            let mut deadline = initial_deadline;
            let mut ticks = tokio::time::interval(std::time::Duration::from_secs(60));
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                let renewal = async {
                    ticks.tick().await;
                    let started = Instant::now();
                    repository
                        .renew_resume_claim(&heartbeat_renew)
                        .await
                        .map(|owned| (owned, started))
                };
                match tokio::time::timeout_at(deadline, renewal).await {
                    Ok(Ok((true, started))) => {
                        deadline = started + std::time::Duration::from_secs(300);
                        *heartbeat_deadline
                            .lock()
                            .expect("callback lease deadline poisoned") = deadline;
                    }
                    _ => {
                        lost_sender.send_replace(true);
                        break;
                    }
                }
            }
        });
        let repository = self.repository.clone();
        let cleanup: Cleanup = Box::new(move || {
            Box::pin(async move {
                // A stale token/generation must not release the current admission.
                repository
                    .finish_resume_claim(&FinishResumeClaimInput {
                        claim_id: renew.claim_id,
                        claim_token: renew.claim_token,
                        expected_generation: renew.expected_generation,
                        status: ResumeClaimStatus::Failed,
                        error_payload: Some(
                            json!({"code":"callback_execution_not_started_or_cancelled"}),
                        ),
                        completed_at: OffsetDateTime::now_utc(),
                    })
                    .await?;
                let admission = receipt.admission_fence();
                repository
                    .release_unstarted_published_callback_resume_attempt(
                        admission.attempt_id,
                        admission.expected_updated_at,
                    )
                    .await?;
                Ok(())
            })
        });
        CallbackExecutionLease {
            lost,
            heartbeat,
            deadline: shared_deadline,
            cleanup: Some(cleanup),
        }
    }
}

#[cfg(test)]
#[path = "../../_tests/application_public_api/prepared_callback_lease.rs"]
mod tests;
