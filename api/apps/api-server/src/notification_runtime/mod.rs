//! Composition-root adapter for notification sends through the existing managed dispatcher.
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use control_plane::notifications::NotificationProviderPort;
use domain::ActorContext;
use extension_contracts::{ContributionId, NotificationTypeDescriptor, NOTIFICATION_POINT};
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use serde_json::Value;
use uuid::Uuid;

use crate::extension_bus::{ManagedExtensionComposition, ManagedWorkspaceSnapshot};

pub(crate) struct ManagedNotificationProvider {
    composition: Arc<ManagedExtensionComposition>,
}

impl ManagedNotificationProvider {
    pub(crate) fn new(composition: Arc<ManagedExtensionComposition>) -> Self {
        Self { composition }
    }

    /// Receive and attachment download have their own independently granted contribution.
    pub(crate) async fn execute_receive(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        descriptor: &NotificationTypeDescriptor,
        config: Value,
        input: extension_contracts::NotificationRuntimeInput,
    ) -> Result<extension_contracts::NotificationRuntimeResult> {
        input.validate(descriptor)?;
        let contribution = &descriptor
            .receive
            .as_ref()
            .context("receive unavailable")?
            .contribution_id;
        let snapshot = self
            .composition
            .snapshot(actor.current_workspace_id)
            .await?;
        let binding = snapshot
            .bindings
            .get(contribution)
            .context("receive unavailable")?;
        ensure!(
            binding.handle.identity().installation_id().as_str() == installation_id.to_string()
                && binding.descriptor.point_id.as_str()
                    == extension_contracts::NOTIFICATION_RECEIVE_POINT,
            "receive contribution does not belong to selected installation"
        );
        let raw = self
            .composition
            .execute_snapshot(
                snapshot,
                actor.current_workspace_id,
                contribution,
                RuntimeExecutionPrincipal {
                    caller: None,
                    workspace_id: actor.current_workspace_id.to_string(),
                    actor_id: Some(actor.user_id.to_string()),
                    deadline_unix_ms: i64::MAX,
                },
                config,
                serde_json::to_value(&input)?,
            )
            .await
            .map_err(|_| anyhow::anyhow!("communication_provider_failed"))?;
        let result: extension_contracts::NotificationRuntimeResult = serde_json::from_value(raw)?;
        result.validate_for(&input, descriptor)?;
        Ok(result)
    }

    async fn snapshot(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        contribution_id: &ContributionId,
    ) -> Result<Arc<ManagedWorkspaceSnapshot>> {
        let snapshot = self
            .composition
            .snapshot(actor.current_workspace_id)
            .await
            .context("notification plugin is unavailable")?;
        let binding = snapshot
            .bindings
            .get(contribution_id)
            .context("notification contribution is unavailable")?;
        ensure!(
            binding.handle.identity().installation_id().as_str() == installation_id.to_string()
                && binding.descriptor.point_id.as_str() == NOTIFICATION_POINT,
            "notification contribution does not belong to the selected installation"
        );
        Ok(snapshot)
    }
}

#[async_trait]
impl NotificationProviderPort for ManagedNotificationProvider {
    async fn available(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        contribution_id: &ContributionId,
    ) -> Result<bool> {
        Ok(self
            .snapshot(actor, installation_id, contribution_id)
            .await
            .is_ok())
    }

    async fn send(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        descriptor: &NotificationTypeDescriptor,
        config: Value,
        input: Value,
    ) -> Result<Value> {
        let snapshot = self
            .snapshot(actor, installation_id, &descriptor.contribution_id)
            .await?;
        self.composition
            .execute_snapshot(
                snapshot,
                actor.current_workspace_id,
                &descriptor.contribution_id,
                RuntimeExecutionPrincipal {
                    // Notification sends do not delegate an MCP resource-import scope.
                    caller: None,
                    workspace_id: actor.current_workspace_id.to_string(),
                    actor_id: Some(actor.user_id.to_string()),
                    // The managed binding supplies its configured execution timeout. No new
                    // notification-specific process budget or user capacity is introduced.
                    deadline_unix_ms: i64::MAX,
                },
                config,
                input,
            )
            .await
            .map_err(|_| anyhow::anyhow!("notification provider send failed"))
    }
}
