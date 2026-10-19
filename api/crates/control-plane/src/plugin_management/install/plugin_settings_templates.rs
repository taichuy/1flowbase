use anyhow::{Context, Result};
use control_plane_contracts::ports::PluginSettingsTemplateInput;
use plugin_framework::PluginManifestV1;
use std::path::Path;

pub(super) fn prepare_settings_templates(
    package_root: &Path,
    manifest: &PluginManifestV1,
) -> Result<Vec<PluginSettingsTemplateInput>> {
    if manifest.settings_pages.is_empty() {
        return Ok(Vec::new());
    }
    if manifest.managed_service.is_some() {
        return super::projections::load_plugin_settings_templates(manifest, package_root);
    }
    let native_path = package_root.join(&manifest.runtime.entry);
    let native = plugin_framework::parse_host_extension_contribution_manifest(
        &std::fs::read_to_string(&native_path)
            .with_context(|| format!("read native declaration {}", native_path.display()))?,
    )?;
    native.validate_package_settings_pages(manifest)?;
    super::projections::load_plugin_settings_templates(manifest, package_root)
}

// Preparation only reads the staging path and builds owned commit input. Await
// validation and asset reads before publishing; detached reads cannot commit.
pub(super) async fn prepare_installation_commit(
    package_root: &Path,
    node_id: &str,
    installation: super::PreparedPluginInstallationInput,
    manifest: &PluginManifestV1,
    package_catalog: Option<super::UpsertPluginPackageCatalogProjectionInput>,
) -> Result<super::CommitPluginInstallationInput> {
    let package_root = package_root.to_path_buf();
    let node_id = node_id.to_owned();
    let manifest = manifest.clone();
    run_preparation(move || {
        super::prepare_installation_commit_blocking(
            &package_root,
            &node_id,
            installation,
            &manifest,
            package_catalog,
        )
    })
    .await
}

async fn run_preparation<T: Send + 'static>(
    prepare: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(prepare)
        .await
        .context("plugin installation preparation task failed")?
}

#[cfg(test)]
#[path = "_tests/settings_preparation.rs"]
mod tests;
