use super::*;
use api_host_kernel::plugin_transport::{
    to_artifact_instance_response, to_installation_response_with_artifact,
};

pub(super) fn to_catalog_response(
    entry: PluginCatalogEntry,
) -> Result<PluginCatalogEntryResponse, ApiError> {
    let local_artifact = entry.local_artifact;
    Ok(PluginCatalogEntryResponse {
        installation: to_installation_response_with_artifact(
            entry.installation,
            Some(local_artifact.clone()),
        )?,
        local_artifact: to_artifact_instance_response(local_artifact)?,
        plugin_type: entry.plugin_type,
        namespace: entry.namespace,
        label_key: entry.label_key,
        description_key: entry.description_key,
        provider_label_key: entry.provider_label_key,
        help_url: entry.help_url,
        default_base_url: entry.default_base_url,
        model_discovery_mode: entry.model_discovery_mode,
        assigned_to_current_workspace: entry.assigned_to_current_workspace,
        catalog_refresh_status: entry.catalog_refresh_status,
        catalog_last_error_message: entry.catalog_last_error_message,
        catalog_refreshed_at: format_optional_time(entry.catalog_refreshed_at)?,
    })
}

pub(super) fn to_catalog_projection_response(
    projection: domain::PluginPackageCatalogProjectionRecord,
) -> Result<PluginCatalogProjectionResponse, ApiError> {
    Ok(PluginCatalogProjectionResponse {
        installation_id: projection.installation_id.to_string(),
        package_code: projection.package_code,
        package_version: projection.package_version,
        projection_status: projection.projection_status.as_str().to_string(),
        last_error_message: projection.last_error_message,
        refreshed_at: format_optional_time(projection.refreshed_at)?,
        updated_at: format_time(projection.updated_at)?,
    })
}

pub(super) fn to_official_catalog_entry_response(
    entry: OfficialPluginCatalogEntry,
) -> OfficialPluginCatalogEntryResponse {
    OfficialPluginCatalogEntryResponse {
        plugin_id: entry.plugin_id,
        plugin_type: entry.plugin_type,
        provider_code: entry.provider_code,
        display_name: entry.display_name,
        description: entry.description,
        icon: entry.icon,
        protocol: entry.protocol,
        latest_version: entry.latest_version,
        minimum_host_version: entry.minimum_host_version,
        current_host_version: entry.current_host_version,
        compatibility_status: entry.compatibility_status,
        compatibility_warning_reason: entry.compatibility_warning_reason,
        selected_artifact: OfficialPluginArtifactResponse {
            os: entry.selected_artifact.os,
            arch: entry.selected_artifact.arch,
            libc: entry.selected_artifact.libc,
            rust_target: entry.selected_artifact.rust_target,
            download_url: entry.selected_artifact.download_url,
            checksum: entry.selected_artifact.checksum,
            signature_algorithm: entry.selected_artifact.signature_algorithm,
            signing_key_id: entry.selected_artifact.signing_key_id,
        },
        help_url: entry.help_url,
        model_discovery_mode: entry.model_discovery_mode,
        install_status: entry.install_status.as_str().to_string(),
    }
}

pub(super) fn to_installed_version_response(
    version: PluginInstalledVersionView,
) -> Result<PluginInstalledVersionResponse, ApiError> {
    Ok(PluginInstalledVersionResponse {
        installation_id: version.installation_id.to_string(),
        plugin_version: version.plugin_version,
        source_kind: version.source_kind,
        trust_level: version.trust_level,
        desired_state: version.desired_state,
        availability_status: version.availability_status,
        local_artifact: to_artifact_instance_response(version.local_artifact)?,
        created_at: format_time(version.created_at)?,
        is_current: version.is_current,
    })
}

pub(super) fn to_family_response(
    entry: PluginFamilyView,
) -> Result<PluginFamilyResponse, ApiError> {
    Ok(PluginFamilyResponse {
        provider_code: entry.provider_code,
        plugin_type: entry.plugin_type,
        namespace: entry.namespace,
        label_key: entry.label_key,
        description_key: entry.description_key,
        provider_label_key: entry.provider_label_key,
        icon: entry.icon,
        protocol: entry.protocol,
        help_url: entry.help_url,
        default_base_url: entry.default_base_url,
        model_discovery_mode: entry.model_discovery_mode,
        current_installation_id: entry.current_installation_id.to_string(),
        current_version: entry.current_version,
        installation_status: entry.installation_status.as_str().to_string(),
        current_local_artifact: to_artifact_instance_response(entry.current_local_artifact)?,
        latest_version: entry.latest_version,
        has_update: entry.has_update,
        installed_versions: entry
            .installed_versions
            .into_iter()
            .map(to_installed_version_response)
            .collect::<Result<Vec<_>, _>>()?,
    })
}
