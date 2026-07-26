use anyhow::Result;
use std::{collections::BTreeSet, path::PathBuf};
use uuid::Uuid;

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

#[derive(Debug, Clone)]
pub struct ListFrontendComponentCapabilitiesQuery {
    pub workspace_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub contribution_code: Option<String>,
    pub query: Option<String>,
    pub module_source: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct GetFrontendComponentCapabilityQuery {
    pub workspace_id: Uuid,
    pub component_id: String,
}

#[derive(Debug, Clone)]
pub struct FrontendComponentCapability {
    pub component_id: String,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub module_source: String,
    pub module_version: String,
    pub browser_asset: domain::FrontendModuleBrowserAsset,
    pub contract: domain::FrontendComponentContract,
}

#[derive(Debug, Clone)]
pub struct FrontendComponentCapabilityPage {
    pub items: Vec<FrontendComponentCapability>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub module_sources: Vec<String>,
}

pub struct GetFrontendModuleAssetQuery {
    pub workspace_id: Uuid,
    pub sha256: String,
}

pub struct FrontendComponentModuleAsset {
    pub sha256: String,
    pub bytes: Vec<u8>,
}

pub struct FrontendComponentCatalogService<R> {
    repository: R,
}

impl<R> FrontendComponentCatalogService<R>
where
    R: FrontendBlockCatalogRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_component_capabilities(
        &self,
        query: ListFrontendComponentCapabilitiesQuery,
    ) -> Result<FrontendComponentCapabilityPage> {
        let mut entries = self.load_entries(query.workspace_id).await?;
        let module_sources = entries
            .iter()
            .map(|entry| entry.module_source.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if let Some(installation_id) = query.installation_id {
            entries.retain(|entry| entry.installation_id == installation_id);
        }
        if let Some(contribution_code) = non_empty(query.contribution_code.as_deref()) {
            entries.retain(|entry| entry.contribution_code == contribution_code);
        }
        if let Some(module_source) = non_empty(query.module_source.as_deref()) {
            entries.retain(|entry| entry.module_source == module_source);
        }
        if let Some(search) = non_empty(query.query.as_deref()) {
            let search = search.to_lowercase();
            entries.retain(|entry| component_matches(entry, &search));
        }

        entries.sort_by(|left, right| {
            left.contract
                .export_name
                .cmp(&right.contract.export_name)
                .then_with(|| left.module_source.cmp(&right.module_source))
                .then_with(|| left.component_id.cmp(&right.component_id))
        });
        let total = entries.len();
        let offset = query.offset.min(total);
        let limit = query.limit.max(1);
        let end = offset.saturating_add(limit).min(total);
        let has_more = end < total;
        Ok(FrontendComponentCapabilityPage {
            items: entries[offset..end].to_vec(),
            total,
            offset,
            limit,
            has_more,
            next_offset: has_more.then_some(end),
            module_sources,
        })
    }

    pub async fn get_component_capability(
        &self,
        query: GetFrontendComponentCapabilityQuery,
    ) -> Result<Option<FrontendComponentCapability>> {
        Ok(self
            .load_entries(query.workspace_id)
            .await?
            .into_iter()
            .find(|entry| entry.component_id == query.component_id))
    }

    async fn load_entries(&self, workspace_id: Uuid) -> Result<Vec<FrontendComponentCapability>> {
        let blocks = self
            .repository
            .list_workspace_frontend_blocks(workspace_id)
            .await?;
        let mut entries = Vec::new();
        for block in blocks {
            for module in block.code_modules {
                for contract in module.components {
                    entries.push(FrontendComponentCapability {
                        component_id: format!(
                            "{}:{}:{}",
                            block.installation_id, block.contribution_code, contract.component_code
                        ),
                        installation_id: block.installation_id,
                        provider_code: block.provider_code.clone(),
                        plugin_id: block.plugin_id.clone(),
                        plugin_version: block.plugin_version.clone(),
                        contribution_code: block.contribution_code.clone(),
                        module_source: module.source.clone(),
                        module_version: module.version.clone(),
                        browser_asset: module.browser_asset.clone(),
                        contract,
                    });
                }
            }
        }
        Ok(entries)
    }
}

impl<R> FrontendComponentCatalogService<R>
where
    R: FrontendBlockCatalogRepository + PluginRepository,
{
    pub async fn get_module_asset(
        &self,
        query: GetFrontendModuleAssetQuery,
    ) -> Result<Option<FrontendComponentModuleAsset>> {
        let blocks = self
            .repository
            .list_workspace_frontend_blocks(query.workspace_id)
            .await?
            .into_iter();
        let registered = blocks
            .flat_map(|block| {
                let installation_id = block.installation_id;
                block
                    .code_modules
                    .into_iter()
                    .map(move |module| (installation_id, module.browser_asset))
            })
            .find(|(_, asset)| asset.sha256 == query.sha256);
        let Some((installation_id, browser_asset)) = registered else {
            return Ok(None);
        };
        let Some(installation) = self.repository.get_installation(installation_id).await? else {
            return Ok(None);
        };
        let root = PathBuf::from(installation.installed_path);
        let registered = plugin_framework::FrontendModuleBrowserAssetManifest {
            path: browser_asset.path,
            sha256: browser_asset.sha256.clone(),
        };
        let bytes = tokio::task::spawn_blocking(move || {
            plugin_framework::load_frontend_module_asset(&root, &registered)
        })
        .await
        .map_err(|_| ControlPlaneError::UpstreamUnavailable("frontend_component_module_asset"))?
        .map_err(|_| ControlPlaneError::UpstreamUnavailable("frontend_component_module_asset"))?;
        Ok(Some(FrontendComponentModuleAsset {
            sha256: browser_asset.sha256,
            bytes,
        }))
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn component_matches(entry: &FrontendComponentCapability, search: &str) -> bool {
    entry.contract.export_name.to_lowercase().contains(search)
        || entry.contract.description.to_lowercase().contains(search)
        || entry
            .contract
            .props
            .iter()
            .any(|prop| prop.name.to_lowercase().contains(search))
        || entry
            .contract
            .limitations
            .iter()
            .any(|limitation| limitation.to_lowercase().contains(search))
}

#[derive(Debug, Clone)]
pub struct FrontendBlockCatalogView {
    pub entries: Vec<domain::FrontendBlockCatalogEntry>,
}

pub struct FrontendBlockCatalogService<R> {
    repository: R,
}

impl<R> FrontendBlockCatalogService<R>
where
    R: AuthRepository + FrontendBlockCatalogRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
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
                .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
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

        Ok(FrontendBlockCatalogView {
            entries: self
                .repository
                .list_workspace_frontend_blocks(actor.current_workspace_id)
                .await?,
        })
    }
}
