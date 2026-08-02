use control_plane::{
    errors::ControlPlaneError,
    plugin_management::ExtensionCatalogCategory,
    ports::{ApplicationRepository, I18nCatalogRepository, McpManagementRepository},
};
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError};

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
    state: &ApiState,
    workspace_id: Uuid,
    entry: &domain::ExtensionInstallationRecord,
) -> Result<&'static str, ApiError> {
    let applied = match entry.application_action {
        domain::ExtensionApplicationAction::None => return Ok("not_required"),
        domain::ExtensionApplicationAction::ConfigureModelProvider => return Ok("available"),
        domain::ExtensionApplicationAction::ImportAgentFlow => {
            ApplicationRepository::has_application_extension_source(
                &state.store,
                workspace_id,
                entry.id,
            )
            .await?
        }
        domain::ExtensionApplicationAction::ImportMcp => {
            McpManagementRepository::has_mcp_extension_bundle_import(
                &state.store,
                workspace_id,
                entry.id,
            )
            .await?
        }
        domain::ExtensionApplicationAction::ActivateI18n => {
            let catalog_state =
                I18nCatalogRepository::get_workspace_catalog_state(&state.store, workspace_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "workspace_i18n_catalog_state",
                    ))?;
            let Some(release_id) = catalog_state.active_release_id() else {
                return Ok("not_applied");
            };
            let Some(active) = I18nCatalogRepository::get_i18n_catalog_release_descriptor(
                &state.store,
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
