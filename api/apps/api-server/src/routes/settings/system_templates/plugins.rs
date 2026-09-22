use crate::{
    host_infrastructure::CacheStore,
    official_extension_catalog::OfficialExtensionCatalogSourcePort,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeServices},
};
use control_plane::{
    errors::ControlPlaneError,
    plugin_management::{
        InstallCurrentNodePluginArtifactCommand, InstallResolvedOfficialPluginCommand,
        PluginManagementService,
    },
    portable_template::PortablePluginDependency,
    ports::{OfficialPluginSourcePort, PluginRepository},
};
use std::{collections::BTreeSet, sync::Arc};
use storage_durable_postgres::MainDurableStore;

pub(crate) struct TemplateDependencies {
    pub store: MainDurableStore,
    pub provider_runtime: Arc<ApiRuntimeServices>,
    pub official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub official_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub cache_store: Arc<dyn CacheStore>,
    pub provider_install_root: String,
    pub api_node_id: String,
}
impl TemplateDependencies {
    pub async fn resolve_plugins(
        &self,
        actor: &domain::ActorContext,
        dependencies: &[PortablePluginDependency],
    ) -> anyhow::Result<()> {
        let repository = self.store.for_actor(actor.clone());
        let service = PluginManagementService::new(
            repository.clone(),
            ApiProviderRuntime::new(self.provider_runtime.clone()),
            self.official_plugin_source.clone(),
            self.provider_install_root.clone(),
        )
        .with_node_id(self.api_node_id.clone())
        .with_model_routing_cache_store(self.cache_store.clone());
        let mut resolved = BTreeSet::new();
        for dependency in dependencies {
            if !resolved.insert((&dependency.plugin_id, &dependency.plugin_version)) {
                continue;
            }
            let installed = repository
                .list_installations()
                .await?
                .into_iter()
                .find(|item| {
                    item.plugin_id == dependency.plugin_id
                        && item.plugin_version == dependency.plugin_version
                });
            if let Some(installed) = installed {
                if installed.verification_status != domain::PluginVerificationStatus::Valid
                    || dependency
                        .checksum
                        .as_ref()
                        .is_some_and(|value| installed.expected_checksum.as_ref() != Some(value))
                {
                    return Err(ControlPlaneError::Conflict("template_plugin_verification").into());
                }
                service
                    .install_current_node_artifact(InstallCurrentNodePluginArtifactCommand {
                        actor_user_id: actor.user_id,
                        installation_id: installed.id,
                    })
                    .await?;
                continue;
            }
            let mut matched = None;
            for category in ["runtime-extensions", "capability-plugins"] {
                let mut cursor = None;
                let mut seen = BTreeSet::new();
                loop {
                    let page = self
                        .official_catalog_source
                        .list_page_for_workspace(
                            actor.current_workspace_id,
                            category,
                            cursor.as_deref(),
                        )
                        .await?;
                    if let Some(entry) = page.entries.into_iter().find(|entry| {
                        entry
                            .source
                            .metadata
                            .get("plugin_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(dependency.plugin_id.as_str())
                            && entry.version == dependency.plugin_version
                    }) {
                        matched = Some((entry, page.source_kind));
                        break;
                    }
                    let Some(next) = page.metadata.next_cursor else {
                        break;
                    };
                    if !seen.insert(next.clone()) {
                        return Err(
                            ControlPlaneError::Conflict("template_plugin_catalog_cursor").into(),
                        );
                    }
                    cursor = Some(next);
                }
                if matched.is_some() {
                    break;
                }
            }
            let (entry, source_kind) =
                matched.ok_or(ControlPlaneError::NotFound("template_plugin_version"))?;
            let plugin_type = entry
                .source
                .metadata
                .get("plugin_type")
                .and_then(serde_json::Value::as_str)
                .ok_or(ControlPlaneError::InvalidInput("template_plugin_type"))?
                .to_owned();
            let downloaded = self
                .official_catalog_source
                .download_artifact_for_workspace(actor.current_workspace_id, &entry)
                .await?;
            let checksum = downloaded
                .descriptor
                .expected_checksum
                .clone()
                .ok_or(ControlPlaneError::InvalidInput("template_plugin_checksum"))?;
            if dependency
                .checksum
                .as_ref()
                .is_some_and(|expected| expected != &checksum)
            {
                return Err(ControlPlaneError::Conflict("template_plugin_checksum").into());
            }
            // Verify the package identity before any installer writes; the existing installer owns trust and lifecycle checks.
            let intake = plugin_framework::intake_package_bytes(
                &downloaded.artifact_bytes,
                &plugin_framework::PackageIntakePolicy {
                    source_kind: source_kind.clone(),
                    trust_mode: "allow_unsigned".to_owned(),
                    expected_artifact_sha256: Some(checksum.clone()),
                    trusted_public_keys: self.official_plugin_source.trusted_public_keys(),
                    original_filename: Some(downloaded.file_name.clone()),
                },
            )
            .await?;
            if intake.manifest.version != dependency.plugin_version
                || intake.manifest.versioned_plugin_id()? != dependency.plugin_id
            {
                return Err(ControlPlaneError::Conflict("template_plugin_identity").into());
            }
            service
                .install_resolved_official_plugin(InstallResolvedOfficialPluginCommand {
                    actor_user_id: actor.user_id,
                    plugin_id: dependency.plugin_id.clone(),
                    plugin_type,
                    minimum_host_version: entry.host_version_requirement,
                    source_kind,
                    file_name: downloaded.file_name,
                    package_bytes: downloaded.artifact_bytes,
                    expected_checksum: checksum,
                    compatibility_override: None,
                    risk_override: None,
                })
                .await?;
        }
        Ok(())
    }
}
