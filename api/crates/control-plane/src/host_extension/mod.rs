use crate::errors::ControlPlaneError;
use plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;

pub const HOST_EXTENSION_CONTRACT_VERSION: &str = "1flowbase.host_extension/v1";

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
    installation.contract_version == CURRENT_PROVIDER_CONTRACT
}

/// Reparse the raw manifest under the durable startup fence; only the verified entry is readable.
pub fn verify_native_startup_manifest(
    raw: &str,
    plugin_id: &str,
    verified: &control_plane_contracts::ports::VerifiedNativePluginArtifact,
) -> Option<String> {
    let manifest = plugin_framework::parse_plugin_manifest(raw).ok()?;
    (manifest.versioned_plugin_id().ok().as_deref() == Some(plugin_id)
        && manifest.version == verified.local_version
        && is_host_extension_manifest(&manifest)
        && plugin_framework::canonical_manifest_fingerprint(raw.as_bytes())
            == verified.manifest_fingerprint)
        .then_some(manifest.runtime.entry)
}

pub fn native_startup_contribution_is_valid(raw: &str) -> bool {
    plugin_framework::parse_host_extension_contribution_manifest(raw).is_ok()
}
