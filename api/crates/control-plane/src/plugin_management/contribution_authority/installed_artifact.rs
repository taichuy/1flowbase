//! Installed package identity and hash-bound asset reads shared by activation consumers.
use super::*;
use sha2::Sha256;
use std::path::{Path, PathBuf};

/// Read the same verified artifact for schema restoration and activation authority.
pub async fn verified_installed_managed_manifest<R: PluginRepository>(
    repository: &R,
    node_id: &str,
    install_root: &Path,
    installation_id: Uuid,
) -> Result<Option<plugin_framework::PluginManifestV1>> {
    Ok(
        verified_installed_managed_artifact(repository, node_id, install_root, installation_id)
            .await?
            .map(|(manifest, _)| manifest),
    )
}

pub(crate) async fn verified_installed_managed_artifact<R: PluginRepository>(
    repository: &R,
    node_id: &str,
    install_root: &Path,
    installation_id: Uuid,
) -> Result<Option<(plugin_framework::PluginManifestV1, PathBuf)>> {
    let installation = repository
        .get_installation(installation_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    if installation.contract_version != "1flowbase.extension-bus/v1" {
        return Ok(None);
    }
    let local = crate::plugin_management::ready_current_node_plugin_installation(
        repository,
        node_id,
        install_root,
        installation_id,
    )
    .await?;
    let package_root = PathBuf::from(
        local
            .local_path()
            .ok_or(ControlPlaneError::PluginUnavailable)?,
    );
    if tokio::fs::symlink_metadata(&package_root)
        .await?
        .file_type()
        .is_symlink()
    {
        return Err(ControlPlaneError::Conflict("managed_schema_artifact_identity").into());
    }
    let raw = read_verified_package_asset(&package_root, "manifest.yaml", None).await?;
    let fingerprint = digest(&raw);
    if local.installation.verification_status != domain::PluginVerificationStatus::Valid
        || local.artifact.manifest_fingerprint.as_deref() != Some(fingerprint.as_str())
    {
        return Err(ControlPlaneError::Conflict("managed_schema_artifact_identity").into());
    }
    let manifest = plugin_framework::parse_plugin_manifest(std::str::from_utf8(&raw)?)?;
    if manifest.publisher_namespace != local.installation.organization
        || manifest.plugin_code()? != local.installation.provider_code
        || manifest.version != local.installation.plugin_version
        || manifest.managed.is_none()
    {
        return Err(ControlPlaneError::Conflict("managed_schema_artifact_identity").into());
    }
    Ok(Some((manifest, package_root)))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)))
}

/// Reject links at every component, including links whose target remains in the package.
/// The digest is checked on the exact bytes later decoded, before any graph mutation.
pub(crate) async fn read_verified_package_asset(
    package_root: &Path,
    relative: &str,
    expected: Option<&str>,
) -> Result<Vec<u8>> {
    if !plugin_framework::valid_managed_mcp_asset_path(relative) {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_path").into());
    }
    if tokio::fs::symlink_metadata(package_root)
        .await?
        .file_type()
        .is_symlink()
    {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_symlink").into());
    }
    let root = tokio::fs::canonicalize(package_root).await?;
    let mut path = root.clone();
    for component in relative.split('/') {
        path.push(component);
        if tokio::fs::symlink_metadata(&path)
            .await?
            .file_type()
            .is_symlink()
        {
            return Err(ControlPlaneError::InvalidInput("managed_mcp_template_symlink").into());
        }
    }
    if !tokio::fs::metadata(&path).await?.is_file()
        || !tokio::fs::canonicalize(&path).await?.starts_with(&root)
    {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_path").into());
    }
    let bytes = tokio::fs::read(path).await?;
    if expected.is_some_and(|expected| expected != digest(&bytes)) {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_digest").into());
    }
    Ok(bytes)
}
