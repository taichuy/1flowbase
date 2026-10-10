//! Independent console operations authorize governance; current/history scope remains durable.
use super::*;
use control_plane_contracts::ports::{
    ManagedExecutionState, ManagedFrozenExecutionTarget, PluginContributionAuthorityRepository,
    ResumeManagedLifecycleDelivery,
};

#[async_trait::async_trait]
pub trait ManagedExecutionGovernancePort: Send + Sync {
    async fn managed_execution_state(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        cursor: Option<String>,
    ) -> Result<ManagedExecutionState>;
    async fn resume_managed_delivery(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        input: ResumeManagedLifecycleDelivery,
    ) -> Result<ManagedExecutionState>;
    async fn retire_managed_execution(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        target: ManagedFrozenExecutionTarget,
    ) -> Result<ManagedExecutionState>;
}

pub struct ManagedExecutionService<R> {
    repository: R,
    runtime: Arc<dyn ManagedExecutionGovernancePort>,
}
impl<R: RoleConsolePolicyReader + PluginContributionAuthorityRepository + PluginRepository>
    ManagedExecutionService<R>
{
    pub fn new(repository: R, runtime: Arc<dyn ManagedExecutionGovernancePort>) -> Self {
        Self {
            repository,
            runtime,
        }
    }
    async fn authorize(
        &self,
        actor: &domain::ActorContext,
        installation_id: Uuid,
        operation: &str,
    ) -> Result<Uuid> {
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor)
                .await?;
            if !domain::effective_console_simple_operation(
                &policies,
                &domain::ConsolePolicyGroup::settings_feature("system.extension-center")?,
                &domain::ConsoleOperationId::try_from(operation)?,
            ) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }
        let installation = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        if installation.contract_version != "1flowbase.extension-bus/v1" {
            return Err(ControlPlaneError::InvalidInput("managed_installation_required").into());
        }
        let scope_id =
            domain::managed_installation_scope(&installation, actor.current_workspace_id);
        self.repository
            .query_contribution_authority(
                installation_id,
                scope_id,
                &audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor.user_id),
                    "plugin_installation",
                    Some(installation_id),
                    operation,
                    json!({}),
                ),
            )
            .await?;
        Ok(scope_id)
    }
    pub async fn query(
        &self,
        actor: &domain::ActorContext,
        installation_id: Uuid,
        cursor: Option<String>,
    ) -> Result<ManagedExecutionState> {
        let scope_id = self
            .authorize(
                actor,
                installation_id,
                "extension_center.managed_execution.view",
            )
            .await?;
        if let Some(value) = cursor.as_deref() {
            control_plane_contracts::ports::ManagedDeliveryCursor::decode(
                value,
                installation_id,
                scope_id,
            )
            .map_err(|_| ControlPlaneError::InvalidInput("invalid_managed_delivery_cursor"))?;
        }
        self.runtime
            .managed_execution_state(scope_id, installation_id, cursor)
            .await
    }
    pub async fn resume(
        &self,
        actor: &domain::ActorContext,
        installation_id: Uuid,
        input: ResumeManagedLifecycleDelivery,
    ) -> Result<ManagedExecutionState> {
        let scope_id = self
            .authorize(
                actor,
                installation_id,
                "extension_center.lifecycle_deliveries.resume",
            )
            .await?;
        self.runtime
            .resume_managed_delivery(scope_id, installation_id, input)
            .await
    }
    pub async fn retire(
        &self,
        actor: &domain::ActorContext,
        installation_id: Uuid,
        target: ManagedFrozenExecutionTarget,
    ) -> Result<ManagedExecutionState> {
        let scope_id = self
            .authorize(
                actor,
                installation_id,
                "extension_center.managed_executions.retire",
            )
            .await?;
        self.runtime
            .retire_managed_execution(scope_id, installation_id, target)
            .await
    }
}
