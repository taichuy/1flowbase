use anyhow::Result;
use std::{
    collections::{hash_map::Entry, HashMap},
    path::PathBuf,
    sync::Arc,
};
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
    resolver: FrontendContributionResolver,
}

impl<R> FrontendModuleAssetService<R> {
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
}

impl<R> FrontendModuleAssetService<R>
where
    R: FrontendBlockCatalogRepository + PluginRepository,
{
    pub async fn get_module_asset(
        &self,
        query: GetFrontendModuleAssetQuery,
    ) -> Result<Option<FrontendModuleAssetResource>> {
        let entries = self
            .repository
            .list_workspace_frontend_blocks(&self.node_id, query.workspace_id)
            .await?;
        let active = resolve_frontend_contributions(
            &self.repository,
            &self.node_id,
            query.workspace_id,
            &self.resolver,
            entries,
        )
        .await?;
        // Digest membership is proved by the same current workspace grant/lifecycle resolver
        // as execution. Retained bytes alone never grant access to a removed/disabled module.
        let registered = active.into_iter().find_map(|binding| {
            let asset = binding
                .assets
                .into_iter()
                .find(|asset| asset.digest == query.sha256)?;
            let registered = binding
                .catalog_entry
                .code_modules
                .into_iter()
                .flat_map(|module| module.assets)
                .find(|item| item.sha256 == asset.digest)?;
            Some((binding.catalog_entry.installation_id, asset, registered))
        });
        let Some((installation_id, asset, registered)) = registered else {
            return Ok(None);
        };
        // Resolver has already verified the current on-disk asset. Prefer retained immutable
        // bytes only after authorization, and verify those bytes independently as well.
        if let Some(retained) = self
            .repository
            .get_retained_frontend_module_asset(query.workspace_id, &asset.digest)
            .await?
        {
            use sha2::{Digest, Sha256};
            if format!(
                "{:x}",
                base16ct::HexDisplay(&Sha256::digest(&retained.bytes))
            ) != asset.digest
                || retained.media_type != asset.media_type
            {
                return Err(ControlPlaneError::UpstreamUnavailable(
                    "frontend_component_module_asset",
                )
                .into());
            }
            return Ok(Some(FrontendModuleAssetResource {
                sha256: asset.digest,
                media_type: asset.media_type,
                bytes: retained.bytes,
            }));
        }
        // Re-read through the registered package asset loader rather than a public path.
        let Some(artifact) = self
            .repository
            .get_artifact_instance(&self.node_id, installation_id)
            .await?
        else {
            return Ok(None);
        };
        let Some(local_path) = artifact.local_path else {
            return Ok(None);
        };
        let manifest = plugin_framework::FrontendModuleAssetManifest {
            path: registered.path,
            role: match registered.role {
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
            media_type: registered.media_type,
            sha256: registered.sha256,
        };
        let bytes = tokio::task::spawn_blocking(move || {
            plugin_framework::load_frontend_module_asset(&PathBuf::from(local_path), &manifest)
        })
        .await
        .map_err(|_| ControlPlaneError::UpstreamUnavailable("frontend_component_module_asset"))?
        .map_err(|_| ControlPlaneError::UpstreamUnavailable("frontend_component_module_asset"))?;
        Ok(Some(FrontendModuleAssetResource {
            sha256: asset.digest,
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
        let mut default_by_contribution = HashMap::with_capacity(managed_defaults.len());
        for template in &managed_defaults {
            default_by_contribution
                .entry((
                    template.provider_code.as_str(),
                    template.contribution_code.as_str(),
                ))
                .or_insert(template);
        }
        for entry in &mut entries {
            let Some(template) = default_by_contribution.get(&(
                entry.provider_code.as_str(),
                entry.contribution_code.as_str(),
            )) else {
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

pub(crate) async fn resolve_frontend_contributions<R>(
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
    let mut assignment_by_installation = HashMap::with_capacity(assignments.len());
    for assignment in assignments {
        assignment_by_installation
            .entry(assignment.installation_id)
            .or_insert(assignment);
    }
    // Reuse the first observed identity facts only within this request. Asset and resolver
    // checks still run for each contribution after the original ordered read phase.
    let mut installation_facts = HashMap::new();
    let mut candidates = Vec::with_capacity(catalog_entries.len());
    for catalog_entry in catalog_entries {
        let facts = match installation_facts.entry(catalog_entry.installation_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let facts = match repository
                    .get_installation(catalog_entry.installation_id)
                    .await?
                {
                    Some(installation) => repository
                        .get_artifact_instance(node_id, installation.id)
                        .await?
                        .map(|artifact| (installation, artifact)),
                    None => None,
                };
                entry.insert(facts)
            }
        };
        let Some((installation, artifact)) = facts else {
            continue;
        };
        let assignment = assignment_by_installation.get(&installation.id).cloned();
        candidates.push(FrontendContributionCandidate {
            workspace_id,
            installation: installation.clone(),
            artifact: artifact.clone(),
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
