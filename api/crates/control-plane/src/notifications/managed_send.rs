//! Send outcomes and Host-owned authorization write sets on the canonical Execute envelope.
use super::authorization::send_result_contains_credentials;
use super::*;
use extension_contracts::{
    NotificationAuthorizationStatus as Status, NotificationFailureCode as Code,
    NotificationOperation, NotificationRuntimeInput, NotificationRuntimeResult,
    NotificationSendOutcome,
};

impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader + 'static,
{
    pub(super) async fn send_managed(
        &self,
        actor: &ActorContext,
        instance: &NotificationInstanceRecord,
        descriptor: &NotificationTypeDescriptor,
        input_payload: Value,
        log_id: Uuid,
        target: Option<extension_contracts::NotificationSendTarget>,
        attempt: &mut Option<super::lifecycle::SendAttemptObservation>,
        context: Option<control_plane_contracts::ports::CommunicationSendContext>,
    ) -> Result<NotificationSendResult> {
        let state = self
            .repository
            .resolve_notification_authorization(
                actor.current_workspace_id,
                instance.id,
                &self.master_key,
            )
            .await?;
        if state.installation_id != instance.installation_id {
            return Err(Error::Conflict("notification_authorization_stale").into());
        }
        // Account authorization alone permits a provider attempt; context_token is optional.
        let config = self
            .notification_config(actor, instance, descriptor)
            .await?;
        self.snapshot_send_message(
            actor,
            log_id,
            descriptor,
            &input_payload,
            &config,
            &state.managed_secrets,
        )
        .await?;
        if descriptor.authorization.is_some() {
            match state
                .authorization
                .as_ref()
                .map_or(Status::Unauthorized, |view| view.status)
            {
                Status::Unauthorized | Status::Pending => {
                    return Err(Error::Conflict("authorization_required").into())
                }
                Status::Invalid => return Err(Error::Conflict("authorization_invalid").into()),
                Status::Authorized => {}
            }
        }
        let archive = descriptor
            .receive
            .as_ref()
            .is_some_and(|receive| receive.supports_send_receipts);
        anyhow::ensure!(
            context.is_none() || archive,
            "communication_send_receipt_required"
        );
        let requested_target = target.clone();
        let input = NotificationRuntimeInput {
            delivery_window: state
                .authorization
                .clone()
                .map(|view| super::authorization::project_for_descriptor(view, descriptor))
                .and_then(|view| view.delivery_window),
            operation: NotificationOperation::Send {
                request_id: archive.then(|| log_id.to_string()),
                input_payload,
                target,
            },
            managed_secrets: state.managed_secrets.clone(),
        };
        input
            .validate(descriptor)
            .map_err(|_| Error::Conflict("notification_authorization_state"))?;
        let provider = self
            .provider
            .as_ref()
            .ok_or(Error::Conflict("plugin_unavailable"))?;
        let installation_id = instance
            .installation_id
            .ok_or(Error::Conflict("type_unavailable"))?;
        let payload = serde_json::to_value(&input)?;
        if archive {
            self.repository
                .prepare_communication_send(
                    control_plane_contracts::ports::PrepareCommunicationSend {
                        workspace_id: actor.current_workspace_id,
                        instance_id: instance.id,
                        send_log_id: log_id,
                        target: requested_target,
                        context,
                    },
                )
                .await?;
        }
        let mut unknown = super::lifecycle::SendAttemptObservation::unknown();
        unknown.archive = archive;
        *attempt = Some(unknown);
        let raw = provider
            .send(actor, installation_id, descriptor, config.clone(), payload)
            .await
            .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
        let response: NotificationRuntimeResult = serde_json::from_value(raw)
            .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
        response
            .validate_for(&input, descriptor)
            .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
        let NotificationRuntimeResult::Send {
            outcome,
            receipt,
            authorization,
            managed_secret_patch,
        } = response
        else {
            return Err(Error::UpstreamUnavailable("notification_provider_failed").into());
        };
        // Examine only the public output/view; the validated Host-only patch contains secrets
        // by design and must never be copied into either response or notification log.
        let public = json!({"authorization": authorization, "receipt":receipt, "result": match &outcome { NotificationSendOutcome::Succeeded { result } => result.clone(), _ => Value::Null }});
        if send_result_contains_credentials(
            &public,
            descriptor,
            &config,
            &state.managed_secrets,
            &managed_secret_patch,
            &input,
        ) {
            return Err(Error::UpstreamUnavailable("notification_provider_failed").into());
        }
        let log_extra = match &outcome {
            NotificationSendOutcome::Succeeded { result } => super::logging::result_extra(
                descriptor,
                result,
                &config,
                &state.managed_secrets,
                &managed_secret_patch,
            )?,
            _ => None,
        };
        *attempt = Some(super::lifecycle::SendAttemptObservation {
            archive,
            receipt,
            result_kind: match &outcome {
                NotificationSendOutcome::Succeeded { .. } => CommunicationSendResultKind::Accepted,
                NotificationSendOutcome::Failed { .. } => CommunicationSendResultKind::Failed,
                NotificationSendOutcome::Unknown { .. } => CommunicationSendResultKind::Unknown,
            },
            failure_code: match &outcome {
                NotificationSendOutcome::Succeeded { .. } => None,
                NotificationSendOutcome::Failed { failure_code }
                | NotificationSendOutcome::Unknown { failure_code } => {
                    serde_json::to_value(failure_code)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                }
            },
        });
        if archive {
            // Start the owned finalization before another await. Dropping the caller cannot
            // discard an accepted receipt; this task only retries local writes, never sends.
            let observed = attempt.as_ref().expect("validated provider outcome");
            let receipt = control_plane_contracts::ports::RecordCommunicationSendReceipt {
                workspace_id: actor.current_workspace_id,
                send_log_id: log_id,
                send_status: match observed.result_kind {
                    CommunicationSendResultKind::Accepted => {
                        domain::CommunicationSendStatus::Succeeded
                    }
                    CommunicationSendResultKind::Failed => domain::CommunicationSendStatus::Failed,
                    CommunicationSendResultKind::Unknown => {
                        domain::CommunicationSendStatus::Unknown
                    }
                },
                failure_code: observed.failure_code.clone(),
                receipt: observed.receipt.clone(),
                log_extra: log_extra.clone(),
            };
            // The outer failure finalizer must not replace this known provider result.
            attempt
                .as_mut()
                .expect("validated provider outcome")
                .archive = false;
            let repository = Arc::clone(&self.repository);
            tokio::spawn(async move {
                let mut delay = std::time::Duration::from_millis(100);
                loop {
                    match repository.record_communication_send_receipt(receipt.clone()).await {
                        Ok(()) => return Ok(()),
                        Err(error) if error.is::<control_plane_contracts::ports::CommunicationSendArchiveUnavailable>() => {
                            // Keep the verified receipt alive across a storage outage. A process
                            // crash before commit remains unknown, never an executable retry.
                            tracing::warn!(send_log_id=%receipt.send_log_id, "channel receipt persistence pending; provider replay suppressed");
                            tokio::time::sleep(delay).await;
                            delay = (delay * 2).min(std::time::Duration::from_secs(30));
                        }
                        Err(error) => return Err(error),
                    }
                }
            }).await.map_err(|_| Error::Conflict("communication_send_finalization_interrupted"))??;
        }
        let authorization_result = self
            .apply_runtime_authorization(
                actor,
                instance.id,
                state,
                authorization,
                managed_secret_patch,
            )
            .await;
        if let Err(error) = authorization_result {
            if archive && matches!(outcome, NotificationSendOutcome::Succeeded { .. }) {
                tracing::warn!(%log_id, "accepted channel send authorization persistence failed");
            } else {
                return Err(error);
            }
        }
        match outcome {
            NotificationSendOutcome::Succeeded { result } => {
                if !archive {
                    self.repository
                        .complete_notification_send_log(
                            actor.current_workspace_id,
                            log_id,
                            "success",
                            None,
                            log_extra,
                        )
                        .await?;
                }
                Ok(NotificationSendResult {
                    instance_id: instance.id,
                    type_code: instance.type_code.clone(),
                    result,
                })
            }
            NotificationSendOutcome::Failed { failure_code } => {
                Err(failure_error(failure_code).into())
            }
            NotificationSendOutcome::Unknown { .. } => {
                Err(Error::UpstreamUnavailable("notification_provider_failed").into())
            }
        }
    }
}

fn failure_error(code: Code) -> Error {
    match code {
        Code::AuthorizationRequired => Error::Conflict("authorization_required"),
        Code::AuthorizationInvalid => Error::Conflict("authorization_invalid"),
        Code::RecipientNotReady => Error::Conflict("recipient_not_ready"),
        Code::SubscriptionRequired => Error::Conflict("subscription_required"),
        Code::SubscriptionExpired => Error::Conflict("subscription_expired"),
        Code::SubscriptionUnavailable => Error::UpstreamUnavailable("subscription_unavailable"),
        Code::ProviderRejected => Error::UpstreamUnavailable("provider_rejected"),
        Code::ProviderResultUnknown => Error::UpstreamUnavailable("notification_provider_failed"),
    }
}
