use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use anyhow::{bail, Context, Result};
use plugin_framework::{
    parse_host_extension_contribution_manifest, parse_plugin_manifest,
    provider_contract::PluginFormFieldSchema, HostInfrastructureProviderManifest,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    host_extension::is_host_extension_manifest,
    ports::{
        HostInfrastructureConfigRepository, PluginRepository, UpdatePluginDesiredStateInput,
        UpsertHostInfrastructureProviderConfigInput,
    },
};

pub struct HostInfrastructureConfigService<R> {
    repository: R,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostInfrastructureProviderConfigView {
    pub installation_id: Uuid,
    pub extension_id: String,
    pub provider_code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub runtime_status: String,
    pub desired_state: String,
    pub config_ref: String,
    pub contracts: Vec<String>,
    pub enabled_contracts: Vec<String>,
    pub config_schema: Vec<PluginFormFieldSchema>,
    pub config_json: Value,
    pub restart_required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostInfrastructureProviderConfigList {
    pub providers: Vec<HostInfrastructureProviderConfigView>,
}

pub struct SaveHostInfrastructureProviderConfigCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub enabled_contracts: Vec<String>,
    pub config_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveHostInfrastructureProviderConfigResult {
    pub restart_required: bool,
    pub installation_desired_state: String,
    pub provider_config_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderGroupKey {
    installation_id: Uuid,
    provider_code: String,
    config_ref: String,
}

#[derive(Debug, Clone)]
struct ProviderGroup {
    installation: domain::PluginInstallationRecord,
    extension_id: String,
    provider_code: String,
    display_name: String,
    description: Option<String>,
    config_ref: String,
    contracts: Vec<String>,
    config_schema: Vec<PluginFormFieldSchema>,
}

impl<R> HostInfrastructureConfigService<R>
where
    R: PluginRepository + HostInfrastructureConfigRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_providers(&self) -> Result<HostInfrastructureProviderConfigList> {
        let saved_configs = self
            .repository
            .list_host_infrastructure_provider_configs()
            .await?
            .into_iter()
            .map(|record| {
                (
                    (record.installation_id, record.provider_code.clone()),
                    record,
                )
            })
            .collect::<HashMap<_, _>>();
        let mut groups = self.load_provider_groups().await?;

        let mut providers = Vec::with_capacity(groups.len());
        for group in groups.drain(..) {
            let saved = saved_configs.get(&(group.installation.id, group.provider_code.clone()));
            let status = saved.map(|record| record.status);
            providers.push(HostInfrastructureProviderConfigView {
                installation_id: group.installation.id,
                extension_id: group.extension_id,
                provider_code: group.provider_code,
                display_name: group.display_name,
                description: group.description,
                runtime_status: group.installation.runtime_status.as_str().to_string(),
                desired_state: group.installation.desired_state.as_str().to_string(),
                config_ref: group.config_ref,
                contracts: group.contracts,
                enabled_contracts: saved
                    .map(|record| record.enabled_contracts.clone())
                    .unwrap_or_default(),
                config_schema: group.config_schema,
                config_json: saved
                    .map(|record| record.config_json.clone())
                    .unwrap_or_else(|| serde_json::json!({})),
                restart_required: matches!(
                    group.installation.desired_state,
                    domain::PluginDesiredState::PendingRestart
                ) || matches!(
                    status,
                    Some(domain::HostInfrastructureConfigStatus::PendingRestart)
                ),
            });
        }

        Ok(HostInfrastructureProviderConfigList { providers })
    }

    pub async fn save_provider_config(
        &self,
        command: SaveHostInfrastructureProviderConfigCommand,
    ) -> Result<SaveHostInfrastructureProviderConfigResult> {
        let groups = self.load_provider_groups().await?;
        let group = groups
            .into_iter()
            .find(|candidate| {
                candidate.installation.id == command.installation_id
                    && candidate.provider_code == command.provider_code
            })
            .ok_or(ControlPlaneError::NotFound("host_infrastructure_provider"))?;

        for contract in &command.enabled_contracts {
            if !group.contracts.contains(contract) {
                bail!(ControlPlaneError::InvalidInput(
                    "enabled_contract_not_declared"
                ));
            }
        }
        validate_required_config_fields(&group.config_schema, &command.config_json)?;

        let updated_installation = self
            .repository
            .update_desired_state(&UpdatePluginDesiredStateInput {
                installation_id: group.installation.id,
                desired_state: domain::PluginDesiredState::PendingRestart,
                availability_status: domain::PluginAvailabilityStatus::PendingRestart,
            })
            .await?;
        let saved_config = self
            .repository
            .upsert_host_infrastructure_provider_config(
                &UpsertHostInfrastructureProviderConfigInput {
                    installation_id: group.installation.id,
                    extension_id: group.extension_id,
                    provider_code: group.provider_code,
                    config_ref: group.config_ref,
                    enabled_contracts: command.enabled_contracts,
                    config_json: command.config_json,
                    status: domain::HostInfrastructureConfigStatus::PendingRestart,
                    actor_user_id: command.actor_user_id,
                },
            )
            .await?;

        Ok(SaveHostInfrastructureProviderConfigResult {
            restart_required: true,
            installation_desired_state: updated_installation.desired_state.as_str().to_string(),
            provider_config_status: saved_config.status.as_str().to_string(),
        })
    }

    async fn load_provider_groups(&self) -> Result<Vec<ProviderGroup>> {
        let installations = self.repository.list_installations().await?;
        let mut groups = BTreeMap::<ProviderGroupKey, ProviderGroup>::new();

        for installation in installations
            .into_iter()
            .filter(crate::host_extension::is_host_extension_installation)
        {
            let contribution = load_installed_host_extension_contribution(&installation)?;
            for provider in contribution.infrastructure_providers {
                let key = ProviderGroupKey {
                    installation_id: installation.id,
                    provider_code: provider.provider_code.clone(),
                    config_ref: provider.config_ref.clone(),
                };
                let entry = groups.entry(key).or_insert_with(|| ProviderGroup {
                    installation: installation.clone(),
                    extension_id: contribution.extension_id.clone(),
                    provider_code: provider.provider_code.clone(),
                    display_name: provider.display_name.clone(),
                    description: provider.description.clone(),
                    config_ref: provider.config_ref.clone(),
                    contracts: Vec::new(),
                    config_schema: provider.config_schema.clone(),
                });
                ensure_consistent_provider_group(entry, &provider)?;
                if !entry.contracts.contains(&provider.contract) {
                    entry.contracts.push(provider.contract);
                }
            }
        }

        Ok(groups.into_values().collect())
    }
}

fn load_installed_host_extension_contribution(
    installation: &domain::PluginInstallationRecord,
) -> Result<plugin_framework::HostExtensionContributionManifest> {
    let install_root = Path::new(&installation.installed_path);
    let manifest_path = install_root.join("manifest.yaml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = parse_plugin_manifest(&manifest_raw)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    if !is_host_extension_manifest(&manifest) {
        bail!(ControlPlaneError::InvalidInput("host_extension_manifest"));
    }

    let contribution_path = install_root.join(&manifest.runtime.entry);
    let contribution_raw = fs::read_to_string(&contribution_path)
        .with_context(|| format!("failed to read {}", contribution_path.display()))?;
    parse_host_extension_contribution_manifest(&contribution_raw)
        .with_context(|| format!("failed to parse {}", contribution_path.display()))
}

fn ensure_consistent_provider_group(
    group: &ProviderGroup,
    provider: &HostInfrastructureProviderManifest,
) -> Result<()> {
    if group.display_name != provider.display_name
        || group.description != provider.description
        || group.config_schema != provider.config_schema
    {
        bail!(ControlPlaneError::InvalidInput(
            "inconsistent_provider_config_schema"
        ));
    }
    Ok(())
}

fn validate_required_config_fields(schema: &[PluginFormFieldSchema], config: &Value) -> Result<()> {
    for field in schema {
        if field.required.unwrap_or(false) {
            let missing = config.get(&field.key).is_none_or(|value| value.is_null());
            if missing {
                bail!(ControlPlaneError::InvalidInput("missing_required_config"));
            }
        }
    }
    Ok(())
}
