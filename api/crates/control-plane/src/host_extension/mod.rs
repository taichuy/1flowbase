use crate::errors::ControlPlaneError;

pub const HOST_EXTENSION_CONTRACT_VERSION: &str = "1flowbase.host_extension/v1";
pub const MODEL_PROVIDER_CONTRACT_V1: &str = "1flowbase.provider/v1";
pub const MODEL_PROVIDER_CONTRACT_V2: &str = "1flowbase.provider/v2";

pub fn ensure_root_actor(actor: &domain::ActorContext) -> Result<(), ControlPlaneError> {
    if actor.is_root {
        return Ok(());
    }

    Err(ControlPlaneError::PermissionDenied(
        "host_extension_root_required",
    ))
}

pub fn ensure_uploaded_host_extensions_enabled(enabled: bool) -> Result<(), ControlPlaneError> {
    if enabled {
        return Ok(());
    }

    Err(ControlPlaneError::Conflict(
        "uploaded_host_extensions_disabled",
    ))
}

pub fn plugin_code_from_plugin_id(plugin_id: &str) -> Result<String, ControlPlaneError> {
    let plugin_code = plugin_id
        .split_once('@')
        .map(|(plugin_code, _version)| plugin_code)
        .unwrap_or(plugin_id);
    if plugin_code.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("plugin_id"));
    }
    Ok(plugin_code.to_string())
}

pub fn is_host_extension_manifest(manifest: &plugin_framework::PluginManifestV1) -> bool {
    manifest.consumption_kind == plugin_framework::PluginConsumptionKind::HostExtension
}

pub fn is_host_extension_installation(installation: &domain::PluginInstallationRecord) -> bool {
    installation.contract_version == HOST_EXTENSION_CONTRACT_VERSION
}

pub fn is_model_provider_installation(installation: &domain::PluginInstallationRecord) -> bool {
    matches!(
        installation.contract_version.as_str(),
        MODEL_PROVIDER_CONTRACT_V1 | MODEL_PROVIDER_CONTRACT_V2
    )
}
