pub mod mcp_projection;
mod mcp_template;
pub use mcp_template::prepare_managed_mcp_template;
mod schema;
pub use schema::prepare_managed_schema_plan;

use super::{
    verified_installed_managed_manifest, HostContributionGrantPolicy,
    PluginContributionAuthorityService,
};
use crate::ports::{
    ManagedSchemaRepository, McpManagementRepository, ModelDefinitionRepository,
    PluginContributionAuthorityRepository, PluginRepository, RoleConsolePolicyReader,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

/// Business preparation shared by every enable and version-selection entry point.
#[async_trait]
pub trait ManagedPluginActivationPort: Send + Sync {
    async fn prepare(
        &self,
        actor: &domain::ActorContext,
        installation_id: uuid::Uuid,
    ) -> Result<()>;
}

pub struct ManagedPluginActivationService<R> {
    repository: R,
    node_id: String,
    install_root: PathBuf,
}
impl<R> ManagedPluginActivationService<R> {
    pub fn new(repository: R, node_id: String, install_root: PathBuf) -> Self {
        Self {
            repository,
            node_id,
            install_root,
        }
    }
}
#[async_trait]
impl<R> ManagedPluginActivationPort for ManagedPluginActivationService<R>
where
    R: PluginRepository
        + PluginContributionAuthorityRepository
        + RoleConsolePolicyReader
        + ManagedSchemaRepository
        + McpManagementRepository
        + ModelDefinitionRepository
        + Clone
        + Send
        + Sync,
{
    async fn prepare(
        &self,
        actor: &domain::ActorContext,
        installation_id: uuid::Uuid,
    ) -> Result<()> {
        let manifest = verified_installed_managed_manifest(
            &self.repository,
            &self.node_id,
            &self.install_root,
            installation_id,
        )
        .await?
        .ok_or(crate::errors::ControlPlaneError::InvalidInput(
            "managed_contribution_installation_required",
        ))?;
        PluginContributionAuthorityService::new(
            self.repository.clone(),
            HostContributionGrantPolicy::root_composition(),
        )
        .with_node_id(self.node_id.clone())
        .prepare_activation_authority(actor, installation_id, &manifest)
        .await?;
        // Approval is durable even if later schema application or activation fails. Retry reuses
        // exact approvals; failed activation must never erase a concurrent grant or revoke.
        if !manifest.data_models.is_empty() {
            let owner = plugin_framework::PluginSchemaOwner {
                publisher_namespace: manifest.publisher_namespace.clone(),
                plugin_code: manifest.plugin_code()?.to_string(),
                plugin_version: manifest.version.clone(),
            };
            let (plan, _) = prepare_managed_schema_plan(
                &self.repository,
                actor.current_workspace_id,
                owner,
                &manifest.data_models,
            )
            .await?;
            self.repository.apply_managed_schema(&plan).await?;
        }
        if let Some(template) = prepare_managed_mcp_template(actor, installation_id, &manifest)? {
            self.repository
                .seed_mcp_bundle_graph_once_atomically(&template)
                .await?;
        }
        Ok(())
    }
}
