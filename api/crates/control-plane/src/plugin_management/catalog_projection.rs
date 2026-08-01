use anyhow::Result;
use plugin_framework::provider_package::ProviderPackage;
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::{
    AuthRepository, PluginRepository, ProviderRuntimePort,
    UpsertPluginPackageCatalogProjectionInput,
};

use super::{
    install::{load_actor_context_for_user, load_provider_package},
    PluginManagementService,
};

#[derive(Debug, Clone)]
pub struct RefreshPluginPackageCatalogProjectionCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
}

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository + PluginRepository,
    H: ProviderRuntimePort,
{
    pub async fn refresh_catalog_projection(
        &self,
        command: RefreshPluginPackageCatalogProjectionCommand,
    ) -> Result<domain::PluginPackageCatalogProjectionRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .ready_current_node_installation(command.installation_id)
            .await?;
        match load_provider_package(&installation.installed_path) {
            Ok(package) => {
                refresh_provider_package_catalog_projection(
                    &self.repository,
                    &installation,
                    &package,
                )
                .await
            }
            Err(error) => {
                record_failed_catalog_projection(&self.repository, &installation, &error).await
            }
        }
    }
}

pub(super) async fn refresh_provider_package_catalog_projection<R>(
    repository: &R,
    installation: &domain::PluginInstallationRecord,
    package: &ProviderPackage,
) -> Result<domain::PluginPackageCatalogProjectionRecord>
where
    R: PluginRepository,
{
    repository
        .upsert_plugin_package_catalog_projection(&UpsertPluginPackageCatalogProjectionInput {
            installation_id: installation.id,
            package_code: installation.provider_code.clone(),
            package_version: installation.plugin_version.clone(),
            catalog_snapshot_json: provider_catalog_snapshot(package)?,
            projection_status: domain::PluginPackageCatalogProjectionStatus::Ok,
            last_error_message: None,
            refreshed_at: Some(OffsetDateTime::now_utc()),
        })
        .await
}

pub(super) async fn record_failed_catalog_projection<R>(
    repository: &R,
    installation: &domain::PluginInstallationRecord,
    error: &anyhow::Error,
) -> Result<domain::PluginPackageCatalogProjectionRecord>
where
    R: PluginRepository,
{
    let previous = repository
        .get_plugin_package_catalog_projection(installation.id)
        .await?;
    repository
        .upsert_plugin_package_catalog_projection(&UpsertPluginPackageCatalogProjectionInput {
            installation_id: installation.id,
            package_code: installation.provider_code.clone(),
            package_version: installation.plugin_version.clone(),
            catalog_snapshot_json: previous
                .map(|projection| projection.catalog_snapshot_json)
                .unwrap_or_else(|| json!({})),
            projection_status: domain::PluginPackageCatalogProjectionStatus::Failed,
            last_error_message: Some(error.to_string()),
            refreshed_at: Some(OffsetDateTime::now_utc()),
        })
        .await
}

pub(super) fn provider_catalog_snapshot(package: &ProviderPackage) -> Result<Value> {
    Ok(json!({
        "manifest": {
            "icon": &package.manifest.icon,
        },
        "provider": {
            "display_name": &package.provider.display_name,
            "help_url": &package.provider.help_url,
            "default_base_url": &package.provider.default_base_url,
            "model_discovery_mode": format!("{:?}", package.provider.model_discovery_mode).to_ascii_lowercase(),
            "supports_model_fetch_without_credentials": package.provider.supports_model_fetch_without_credentials,
            "form_schema": serde_json::to_value(&package.provider.form_schema)?,
            "parameter_form": serde_json::to_value(&package.provider.parameter_form)?,
            "predefined_models": serde_json::to_value(&package.predefined_models)?,
        },
        "i18n": {
            "default_locale": &package.i18n.default_locale,
            "bundles": &package.i18n.bundles,
        }
    }))
}
