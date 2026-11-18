//! Ephemeral channel status uses the provider envelope, never the durable send pipeline.
use super::*;
use extension_contracts::{
    NotificationActivityState, NotificationAuthorizationStatus, NotificationOperation,
    NotificationRuntimeInput, NotificationRuntimeResult, NotificationSendTarget,
};

impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader,
{
    pub async fn set_communication_activity(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        target: NotificationSendTarget,
        activity: NotificationActivityState,
    ) -> Result<bool> {
        self.ensure_actor(actor).await?;
        let instance = self.record(actor, instance_id).await?;
        let entry = self
            .type_entry(actor, &instance.type_code, instance.installation_id)
            .await?;
        if !entry.available
            || instance.status != NotificationInstanceStatus::Enabled
            || !entry
                .descriptor
                .receive
                .as_ref()
                .is_some_and(|receive| receive.supports_activity)
        {
            return Ok(false);
        }
        let state = self
            .repository
            .resolve_notification_authorization(
                actor.current_workspace_id,
                instance_id,
                &self.master_key,
            )
            .await?;
        if state.installation_id != instance.installation_id
            || state
                .authorization
                .as_ref()
                .is_none_or(|view| view.status != NotificationAuthorizationStatus::Authorized)
        {
            return Ok(false);
        }
        let input = NotificationRuntimeInput {
            delivery_window: state
                .authorization
                .as_ref()
                .and_then(|view| view.delivery_window.clone()),
            operation: NotificationOperation::SetActivity {
                target,
                state: activity,
            },
            managed_secrets: state.managed_secrets.clone(),
        };
        input.validate(&entry.descriptor)?;
        let config = self
            .notification_config(actor, &instance, &entry.descriptor)
            .await?;
        let provider = self
            .provider
            .as_ref()
            .ok_or(Error::Conflict("plugin_unavailable"))?;
        let installation = instance
            .installation_id
            .ok_or(Error::Conflict("type_unavailable"))?;
        let raw = provider
            .send(
                actor,
                installation,
                &entry.descriptor,
                config,
                serde_json::to_value(&input)?,
            )
            .await?;
        let result: NotificationRuntimeResult = serde_json::from_value(raw)?;
        result.validate_for(&input, &entry.descriptor)?;
        let NotificationRuntimeResult::Activity {
            applied,
            managed_secret_patch,
        } = result
        else {
            return Err(Error::UpstreamUnavailable("notification_provider_failed").into());
        };
        if !managed_secret_patch.is_empty() {
            self.apply_runtime_authorization(actor, instance_id, state, None, managed_secret_patch)
                .await?;
        }
        Ok(applied)
    }
}
