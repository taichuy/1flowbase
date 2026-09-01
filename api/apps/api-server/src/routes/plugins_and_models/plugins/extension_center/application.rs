use control_plane::{
    errors::ControlPlaneError,
    plugin_management::ExtensionCatalogCategory,
    ports::{ApplicationRepository, I18nCatalogRepository, McpManagementRepository},
};
use uuid::Uuid;

use crate::error_response::ApiError;

use super::ExtensionCenterDependencies;

pub(super) fn default_application_status(
    action: domain::ExtensionApplicationAction,
) -> &'static str {
    match action {
        domain::ExtensionApplicationAction::None => "not_required",
        domain::ExtensionApplicationAction::ConfigureModelProvider => "available",
        domain::ExtensionApplicationAction::ImportAgentFlow
        | domain::ExtensionApplicationAction::ImportMcp
        | domain::ExtensionApplicationAction::ActivateI18n => "not_applied",
    }
}

pub(super) fn catalog_application_action(
    category: ExtensionCatalogCategory,
) -> domain::ExtensionApplicationAction {
    match category {
        ExtensionCatalogCategory::AgentFlow => domain::ExtensionApplicationAction::ImportAgentFlow,
        ExtensionCatalogCategory::I18n => domain::ExtensionApplicationAction::ActivateI18n,
        ExtensionCatalogCategory::Mcp => domain::ExtensionApplicationAction::ImportMcp,
        ExtensionCatalogCategory::CapabilityPlugins
        | ExtensionCatalogCategory::HostExtensions
        | ExtensionCatalogCategory::RuntimeExtensions => domain::ExtensionApplicationAction::None,
    }
}

pub(super) async fn workspace_application_status(
    dependencies: &ExtensionCenterDependencies,
    workspace_id: Uuid,
    entry: &domain::ExtensionInstallationRecord,
) -> Result<&'static str, ApiError> {
    let applied = match entry.application_action {
        domain::ExtensionApplicationAction::None => return Ok("not_required"),
        domain::ExtensionApplicationAction::ConfigureModelProvider => return Ok("available"),
        domain::ExtensionApplicationAction::ImportAgentFlow => {
            ApplicationRepository::has_application_extension_source(
                &dependencies.store,
                workspace_id,
                entry.id,
            )
            .await?
        }
        domain::ExtensionApplicationAction::ImportMcp => {
            let local_path = entry
                .local_path
                .as_deref()
                .ok_or(ControlPlaneError::Conflict(
                    "extension_artifact_path_missing",
                ))?;
            let bytes = tokio::fs::read(local_path).await?;
            let package = tokio::task::spawn_blocking(move || {
                crate::routes::mcp_management::bundles::parse_bundle_archive(&bytes)
            })
            .await
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
            let Ok(package) = package else {
                return Ok("not_applied");
            };
            let mut all_instances_are_present = true;
            for template in package.instances {
                let current = McpManagementRepository::get_mcp_instance(
                    &dependencies.store,
                    workspace_id,
                    &template.instance_id,
                )
                .await?;
                let has_matching_source = current
                    .and_then(|instance| instance.managed_by)
                    .is_some_and(|source| {
                        source.organization == package.manifest.organization
                            && source.bundle_id == package.manifest.bundle_id
                            && source.bundle_version == package.manifest.bundle_version
                    });
                if !has_matching_source {
                    all_instances_are_present = false;
                    break;
                }
            }
            all_instances_are_present
        }
        domain::ExtensionApplicationAction::ActivateI18n => {
            let catalog_state = I18nCatalogRepository::get_workspace_catalog_state(
                &dependencies.store,
                workspace_id,
            )
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "workspace_i18n_catalog_state",
            ))?;
            let Some(release_id) = catalog_state.active_release_id() else {
                return Ok("not_applied");
            };
            let Some(active) = I18nCatalogRepository::get_i18n_catalog_release_descriptor(
                &dependencies.store,
                workspace_id,
                release_id,
            )
            .await?
            else {
                return Ok("not_applied");
            };
            let local_path = entry
                .local_path
                .as_deref()
                .ok_or(ControlPlaneError::Conflict(
                    "extension_artifact_path_missing",
                ))?;
            let bytes = tokio::fs::read(local_path).await?;
            let inspection = tokio::task::spawn_blocking(move || {
                crate::official_i18n_catalog_seed::inspect_catalog_seed(&bytes)
            })
            .await
            .map_err(|_| {
                control_plane::errors::ControlPlaneError::InvalidInput("i18n_catalog_seed")
            })?;
            let Ok(inspection) = inspection else {
                return Ok("not_applied");
            };
            active.catalog_version == inspection.catalog_version
                && active.semantic_sha256 == inspection.semantic_sha256
        }
    };
    Ok(if applied { "applied" } else { "not_applied" })
}
