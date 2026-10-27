//! Send outcomes and Host-owned authorization write sets on the canonical Execute envelope.
use super::authorization::public_contains_credentials;
use super::*;
use extension_contracts::{
    NotificationAuthorizationStatus as Status, NotificationFailureCode as Code,
    NotificationOperation, NotificationRuntimeInput, NotificationRuntimeResult,
    NotificationSendOutcome,
};

impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader,
{
    pub(super) async fn send_managed(
        &self,
        actor: &ActorContext,
        instance: &NotificationInstanceRecord,
        descriptor: &NotificationTypeDescriptor,
        input_payload: Value,
        log_id: Uuid,
        target: Option<extension_contracts::NotificationSendTarget>,
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
        // Account authorization alone permits a provider attempt; context_token is optional.
        let config = self
            .notification_config(actor, instance, descriptor)
            .await?;
        let input = NotificationRuntimeInput {
            delivery_window: state
                .authorization
                .clone()
                .map(|view| super::authorization::project_for_descriptor(view, descriptor))
                .and_then(|view| view.delivery_window),
            operation: NotificationOperation::Send {
                input_payload,
                target,
            },
            managed_secrets: state.managed_secrets.clone(),
        };
        input
            .validate(descriptor)
            .map_err(|_| Error::Conflict("notification_authorization_state"))?;
        let raw = self
            .provider
            .as_ref()
            .ok_or(Error::Conflict("plugin_unavailable"))?
            .send(
                actor,
                instance
                    .installation_id
                    .ok_or(Error::Conflict("type_unavailable"))?,
                descriptor,
                config.clone(),
                serde_json::to_value(&input)?,
            )
            .await
            .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
        let response: NotificationRuntimeResult = serde_json::from_value(raw)
            .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
        response
            .validate_for(&input, descriptor)
            .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
        let NotificationRuntimeResult::Send {
            outcome,
            authorization,
            managed_secret_patch,
        } = response
        else {
            return Err(Error::UpstreamUnavailable("notification_provider_failed").into());
        };
        // Examine only the public output/view; the validated Host-only patch contains secrets
        // by design and must never be copied into either response or notification log.
        let public = json!({"authorization": authorization, "result": match &outcome { NotificationSendOutcome::Succeeded { result } => result.clone(), _ => Value::Null }});
        if public_contains_credentials(
            &public,
            descriptor,
            &config,
            &state.managed_secrets,
            &managed_secret_patch,
        ) {
            return Err(Error::UpstreamUnavailable("notification_provider_failed").into());
        }
        self.apply_runtime_authorization(
            actor,
            instance.id,
            state,
            authorization,
            managed_secret_patch,
        )
        .await?;
        match outcome {
            NotificationSendOutcome::Succeeded { result } => {
                self.repository
                    .complete_notification_send_log(
                        actor.current_workspace_id,
                        log_id,
                        "success",
                        None,
                    )
                    .await?;
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
