use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use crate::{
    error::{FrameworkResult, PluginFrameworkError},
    FrontendModuleBrowserAssetManifest, PluginManifestV1,
};

pub fn validate_frontend_module_assets(
    package_root: &Path,
    manifest: &PluginManifestV1,
) -> FrameworkResult<()> {
    for contribution in &manifest.block_contributions {
        for module in &contribution.code_modules {
            load_frontend_module_asset(package_root, &module.browser_asset)?;
        }
    }
    Ok(())
}

pub fn load_frontend_module_asset(
    package_root: &Path,
    asset: &FrontendModuleBrowserAssetManifest,
) -> FrameworkResult<Vec<u8>> {
    let root = package_root
        .canonicalize()
        .map_err(|error| PluginFrameworkError::io(Some(package_root), error.to_string()))?;
    let candidate = root.join(&asset.path);
    let resolved = candidate.canonicalize().map_err(|error| {
        PluginFrameworkError::invalid_provider_package(format!(
            "registered frontend module asset {} is unavailable: {error}",
            asset.path
        ))
    })?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "registered frontend module asset must be a file within the plugin package",
        ));
    }
    let bytes = fs::read(&resolved)
        .map_err(|error| PluginFrameworkError::io(Some(&resolved), error.to_string()))?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != asset.sha256 {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "registered frontend module asset {} SHA-256 mismatch",
            asset.path
        )));
    }
    Ok(bytes)
}
