use control_plane::plugin_management::InstallResolvedOfficialPluginCommand;

use super::{to_compatibility_override, to_risk_override, InstallOfficialPluginBody};
use crate::{
    error_response::ApiError, official_extension_catalog::OfficialExtensionCatalogSourcePort,
};

pub(crate) async fn resolve_official_install_command(
    source: &dyn OfficialExtensionCatalogSourcePort,
    actor_user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    body: InstallOfficialPluginBody,
) -> Result<InstallResolvedOfficialPluginCommand, ApiError> {
    let mut cursor = None;
    let (entry, source_kind) = loop {
        let page = source
            .list_page_for_workspace(workspace_id, "runtime-extensions", cursor.as_deref())
            .await?;
        if let Some(entry) = page.entries.into_iter().find(|entry| {
            entry
                .source
                .metadata
                .get("plugin_id")
                .and_then(serde_json::Value::as_str)
                == Some(body.plugin_id.as_str())
        }) {
            break (entry, page.source_kind);
        }
        let Some(next) = page.metadata.next_cursor else {
            return Err(
                control_plane::errors::ControlPlaneError::NotFound("official_plugin").into(),
            );
        };
        cursor = Some(next);
    };
    let plugin_type = entry
        .source
        .metadata
        .get("plugin_type")
        .and_then(serde_json::Value::as_str)
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "official_plugin_type",
        ))?
        .to_string();
    let downloaded = source
        .download_artifact_for_workspace(workspace_id, &entry)
        .await?;
    let expected_checksum = downloaded.descriptor.expected_checksum.clone().ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("official_plugin_checksum"),
    )?;
    Ok(InstallResolvedOfficialPluginCommand {
        actor_user_id,
        plugin_id: body.plugin_id,
        plugin_type,
        minimum_host_version: entry.host_version_requirement,
        source_kind,
        file_name: downloaded.file_name,
        package_bytes: downloaded.artifact_bytes,
        expected_checksum,
        compatibility_override: to_compatibility_override(body.compatibility_override),
        risk_override: to_risk_override(body.risk_override),
    })
}
