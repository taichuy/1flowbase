use anyhow::Result;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

mod frontend_contribution;

pub use frontend_contribution::{
    FrontendContributionAssetBinding, FrontendContributionAssetIntegrity,
    FrontendContributionBinding, FrontendContributionCandidate, FrontendContributionDisableReason,
    FrontendContributionDisabledReceipt, FrontendContributionExecutionKind,
    FrontendContributionIsolationRequirement, FrontendContributionResolution,
    FrontendContributionResolver, FrontendContributionRuntimeKind,
    FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID, FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
    FRONTEND_BLOCK_CONTRIBUTION_POINT_ID, FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
    FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
};

use crate::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, FrontendBlockCatalogRepository, PluginRepository, RoleConsolePolicyReader,
    },
};

const FRONTEND_BLOCKS_VIEW_OPERATION_ID: &str = "frontend_blocks.view";

pub struct ListFrontendBlockCatalogQuery {
    pub actor_user_id: Uuid,
}

pub struct GetFrontendModuleAssetQuery {
    pub workspace_id: Uuid,
    pub sha256: String,
}

pub struct FrontendModuleAssetResource {
    pub sha256: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

pub struct FrontendModuleAssetService<R> {
    repository: R,
    node_id: String,
}

impl<R> FrontendModuleAssetService<R> {
    pub fn new(repository: R, node_id: impl Into<String>) -> Self {
        Self {
            repository,
            node_id: node_id.into(),
        }
    }
}

impl<R> FrontendModuleAssetService<R>
where
    R: FrontendBlockCatalogRepository + PluginRepository,
{
    pub async fn get_module_asset(
        &self,
        query: GetFrontendModuleAssetQuery,
    ) -> Result<Option<FrontendModuleAssetResource>> {
        if let Some(asset) = self
            .repository
            .get_retained_frontend_module_asset(query.workspace_id, &query.sha256)
            .await?
        {
            return Ok(Some(FrontendModuleAssetResource {
                sha256: asset.sha256,
                media_type: asset.media_type,
                bytes: asset.bytes,
            }));
        }
        let blocks = self
            .repository
            .list_system_frontend_blocks(&self.node_id)
            .await?
            .into_iter();
        let registered = blocks
            .flat_map(|block| {
                let installation_id = block.installation_id;
                block.code_modules.into_iter().flat_map(move |module| {
                    module
                        .assets
                        .into_iter()
                        .map(move |asset| (installation_id, asset))
                })
            })
            .find(|(_, asset)| asset.sha256 == query.sha256);
        let Some((installation_id, asset)) = registered else {
            return Ok(None);
        };
        let Some(installation) = self.repository.get_installation(installation_id).await? else {
            return Ok(None);
        };
        let Some(artifact) = self
            .repository
            .get_artifact_instance(&self.node_id, installation.id)
            .await?
        else {
            return Ok(None);
        };
        let Some(local_path) = artifact.local_path else {
            return Ok(None);
        };
        let root = PathBuf::from(local_path);
        let registered = plugin_framework::FrontendModuleAssetManifest {
            path: asset.path,
            role: match asset.role {
                domain::FrontendModuleAssetRole::BrowserModule => {
                    plugin_framework::FrontendModuleAssetRoleManifest::BrowserModule
                }
                domain::FrontendModuleAssetRole::ShadowStyle => {
                    plugin_framework::FrontendModuleAssetRoleManifest::ShadowStyle
                }
                domain::FrontendModuleAssetRole::Support => {
                    plugin_framework::FrontendModuleAssetRoleManifest::Support
                }
            },
            media_type: asset.media_type.clone(),
            sha256: asset.sha256.clone(),
        };
        let bytes = tokio::task::spawn_blocking(move || {
            plugin_framework::load_frontend_module_asset(&root, &registered)
        })
        .await
        .map_err(|_| ControlPlaneError::UpstreamUnavailable("frontend_component_module_asset"))?
        .map_err(|_| ControlPlaneError::UpstreamUnavailable("frontend_component_module_asset"))?;
        Ok(Some(FrontendModuleAssetResource {
            sha256: asset.sha256,
            media_type: asset.media_type,
            bytes,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct FrontendBlockCatalogView {
    pub entries: Vec<FrontendContributionBinding>,
}

pub struct FrontendBlockCatalogService<R> {
    repository: R,
    node_id: String,
    resolver: FrontendContributionResolver,
}

impl<R> FrontendBlockCatalogService<R>
where
    R: AuthRepository + FrontendBlockCatalogRepository + PluginRepository + RoleConsolePolicyReader,
{
    pub fn new(
        repository: R,
        node_id: impl Into<String>,
        graph: Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>,
    ) -> Result<Self> {
        Ok(Self {
            repository,
            node_id: node_id.into(),
            resolver: FrontendContributionResolver::compile(graph)?,
        })
    }

    pub async fn list_frontend_blocks(
        &self,
        query: ListFrontendBlockCatalogQuery,
    ) -> Result<FrontendBlockCatalogView> {
        let actor = self
            .repository
            .load_actor_context_for_user(query.actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(&actor)
                .await?;
            let group = domain::ConsolePolicyGroup::other("other.frontend-blocks")
                .expect("compiled frontend block policy group must be valid");
            let operation_id =
                domain::ConsoleOperationId::try_from(FRONTEND_BLOCKS_VIEW_OPERATION_ID)
                    .expect("compiled frontend block operation id must be valid");
            if !domain::effective_console_simple_operation(&policies, &group, &operation_id) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        let mut entries = self
            .repository
            .list_workspace_frontend_blocks(&self.node_id, actor.current_workspace_id)
            .await?;
        let managed_defaults = self
            .repository
            .list_active_ui_code_templates_for_catalog()
            .await?;
        for entry in &mut entries {
            let Some(template) = managed_defaults.iter().find(|template| {
                template.is_default
                    && template.provider_code == entry.provider_code
                    && template.contribution_code == entry.contribution_code
            }) else {
                continue;
            };
            let Some(revision) = template.published_revision.as_ref() else {
                continue;
            };
            entry.code_template = Some(revision.source.clone());
            entry.code_template_language = Some(revision.language.as_str().to_string());
            entry.code_template_version = Some(format!("managed-r{}", revision.revision));
        }
        let entries = resolve_frontend_contributions(
            &self.repository,
            &self.node_id,
            actor.current_workspace_id,
            &self.resolver,
            entries,
        )
        .await?;
        Ok(FrontendBlockCatalogView { entries })
    }
}

async fn resolve_frontend_contributions<R>(
    repository: &R,
    node_id: &str,
    workspace_id: Uuid,
    resolver: &FrontendContributionResolver,
    catalog_entries: Vec<domain::FrontendBlockCatalogEntry>,
) -> Result<Vec<FrontendContributionBinding>>
where
    R: FrontendBlockCatalogRepository + PluginRepository,
{
    let assignments = repository.list_assignments(workspace_id).await?;
    let mut candidates = Vec::with_capacity(catalog_entries.len());
    for catalog_entry in catalog_entries {
        let Some(installation) = repository
            .get_installation(catalog_entry.installation_id)
            .await?
        else {
            continue;
        };
        let Some(artifact) = repository
            .get_artifact_instance(node_id, installation.id)
            .await?
        else {
            continue;
        };
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.installation_id == installation.id)
            .cloned();
        candidates.push(FrontendContributionCandidate {
            workspace_id,
            installation,
            artifact,
            assignment,
            catalog_entry,
        });
    }
    let resolver = resolver.clone();
    tokio::task::spawn_blocking(move || {
        candidates
            .into_iter()
            .filter_map(|candidate| match resolver.resolve(candidate) {
                FrontendContributionResolution::Active(binding) => Some(*binding),
                FrontendContributionResolution::Disabled(_) => None,
            })
            .collect()
    })
    .await
    .map_err(|_| {
        anyhow::Error::from(ControlPlaneError::UpstreamUnavailable(
            "frontend_block_catalog",
        ))
    })
}
