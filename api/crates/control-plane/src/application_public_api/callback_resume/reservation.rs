use super::*;

/// Only an acquisition's returned row version can release admission before execution.
/// This host-only receipt is never deserialized from a client payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCallbackReservationReceipt {
    attempt_id: Uuid,
    acquired_at: OffsetDateTime,
}

impl PublishedCallbackReservationReceipt {
    pub fn attempt_id(&self) -> Uuid {
        self.attempt_id
    }

    pub(super) fn admission_fence(&self) -> crate::ports::CallbackResumeAdmissionFence {
        crate::ports::CallbackResumeAdmissionFence {
            attempt_id: self.attempt_id,
            expected_updated_at: self.acquired_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedCallbackReservation {
    Acquired(PublishedCallbackReservationReceipt),
    SettledReplay { attempt_id: Uuid },
}

impl PublishedCallbackReservation {
    fn acquired(attempt: domain::FlowRunCallbackResumeAttemptRecord) -> Self {
        Self::Acquired(PublishedCallbackReservationReceipt {
            attempt_id: attempt.id,
            acquired_at: attempt.updated_at,
        })
    }

    pub fn attempt_id(&self) -> Uuid {
        match self {
            Self::Acquired(receipt) => receipt.attempt_id,
            Self::SettledReplay { attempt_id } => *attempt_id,
        }
    }
}

impl<R, C> ApplicationPublishedCallbackResumeService<R, C>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublishedFlowRunRepository
        + ApplicationPublishedRunControlRepository
        + ApplicationPublishedCallbackAttemptRepository
        + Clone,
    C: ApplicationPublishedCallbackConsumer,
{
    /// Compatibility entry point for owners that do not yet handle pre-execution failure.
    pub async fn reserve_native_callback_for_actor(
        &self,
        actor: super::super::api_keys::ApplicationApiKeyActor,
        command: &ResumePublishedCallbackCommand,
    ) -> Result<Uuid> {
        Ok(self
            .reserve_native_callback_reservation_for_actor(actor, command)
            .await?
            .attempt_id())
    }

    /// Call only before handing execution to the consumer. A false result means
    /// admission changed or an execution owner is active; never park it by ID.
    pub async fn release_unstarted_native_callback(
        &self,
        receipt: &PublishedCallbackReservationReceipt,
    ) -> Result<bool> {
        self.repository
            .release_unstarted_published_callback_resume_attempt(
                receipt.attempt_id,
                receipt.acquired_at,
            )
            .await
    }

    /// Reserve before opening the shared runtime stream, so a losing delivery cannot close it.
    pub async fn reserve_native_callback_reservation_for_actor(
        &self,
        actor: super::super::api_keys::ApplicationApiKeyActor,
        command: &ResumePublishedCallbackCommand,
    ) -> Result<PublishedCallbackReservation> {
        let context = self
            .resolve_resume_context_for_actor(actor, command)
            .await?;
        if let Some(existing) = self
            .repository
            .get_published_callback_resume_attempt(context.callback_task.id)
            .await?
        {
            if existing.source != command.source.as_str() {
                return Err(ControlPlaneError::Conflict("callback_resume_source_conflict").into());
            }
            if (command.responses_continuation.is_some()
                || command.anthropic_continuation.is_some())
                && matches!(
                    existing.status,
                    domain::FlowRunCallbackResumeAttemptStatus::Processing
                        | domain::FlowRunCallbackResumeAttemptStatus::Received
                )
            {
                if !domain::orchestration::callback_resume_payloads_match(
                    &existing.response_payload,
                    &command.response_payload,
                ) {
                    return Err(
                        ControlPlaneError::Conflict("callback_resume_payload_conflict").into(),
                    );
                }
                return self
                    .repository
                    .reclaim_expired_callback_resume_attempt(
                        existing.id,
                        command.response_payload.clone(),
                    )
                    .await?
                    .map(PublishedCallbackReservation::acquired)
                    .ok_or_else(|| {
                        ControlPlaneError::Conflict("callback_resume_already_admitted").into()
                    });
            }
            let is_tool_round = context.callback_task.callback_kind == "llm_tool_calls";
            if !is_tool_round
                && !domain::orchestration::callback_resume_payloads_match(
                    &existing.response_payload,
                    &command.response_payload,
                )
            {
                return Err(ControlPlaneError::Conflict("callback_resume_payload_conflict").into());
            }
            if is_tool_round
                && context.callback_task.status == domain::CallbackTaskStatus::Completed
                && command.source == PublishedCallbackResumeSource::OpenAiResponses
                && command.responses_continuation.is_none()
                && command.native_transport.is_some()
                && matches!(
                    existing.status,
                    domain::FlowRunCallbackResumeAttemptStatus::Processing
                        | domain::FlowRunCallbackResumeAttemptStatus::Received
                )
            {
                if !domain::orchestration::callback_resume_payloads_match(
                    &existing.response_payload,
                    &command.response_payload,
                ) {
                    return Err(
                        ControlPlaneError::Conflict("callback_resume_payload_conflict").into(),
                    );
                }
                return self
                    .repository
                    .reclaim_expired_callback_resume_attempt(
                        existing.id,
                        command.response_payload.clone(),
                    )
                    .await?
                    .map(PublishedCallbackReservation::acquired)
                    .ok_or_else(|| {
                        ControlPlaneError::Conflict("callback_resume_already_admitted").into()
                    });
            }
            if is_tool_round
                && context.callback_task.status != domain::CallbackTaskStatus::Completed
            {
                // A tool round keeps one durable attempt across partial
                // deliveries. Only a parked attempt can be re-acquired, and
                // exactly one concurrent delivery wins that transition.
                return self
                    .repository
                    .claim_published_callback_resume_attempt(
                        existing.id,
                        command.response_payload.clone(),
                    )
                    .await?
                    .map(PublishedCallbackReservation::acquired)
                    .ok_or_else(|| {
                        ControlPlaneError::Conflict("callback_resume_already_admitted").into()
                    });
            }
            if matches!(
                existing.status,
                domain::FlowRunCallbackResumeAttemptStatus::Succeeded
                    | domain::FlowRunCallbackResumeAttemptStatus::Failed
                    | domain::FlowRunCallbackResumeAttemptStatus::Cancelled
            ) && context.callback_task.status == domain::CallbackTaskStatus::Completed
            {
                ensure_existing_callback_resume_matches(
                    &context.callback_task,
                    &existing,
                    command,
                )?;
                return Ok(PublishedCallbackReservation::SettledReplay {
                    attempt_id: existing.id,
                });
            }
            return Err(ControlPlaneError::Conflict("callback_resume_already_admitted").into());
        }
        ensure_callback_is_consumable(
            &context.flow_run,
            &context.callback_task,
            &command.response_payload,
        )?;
        let reserved = self
            .repository
            .record_published_callback_resume_attempt(&RecordFlowRunCallbackResumeAttemptInput {
                flow_run_id: context.flow_run.id,
                callback_task_id: context.callback_task.id,
                source: command.source.as_str().to_string(),
                response_payload: command.response_payload.clone(),
                idempotency_key: format!("callback_task:{}", context.callback_task.id),
            })
            .await?;
        if !reserved.inserted {
            return Err(ControlPlaneError::Conflict("callback_resume_already_admitted").into());
        }
        Ok(PublishedCallbackReservation::acquired(reserved.attempt))
    }
}
