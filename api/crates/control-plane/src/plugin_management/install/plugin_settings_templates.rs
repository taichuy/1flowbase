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
    let native_path = package_root.join(&manifest.runtime.entry);
    let native = plugin_framework::parse_host_extension_contribution_manifest(
        &std::fs::read_to_string(&native_path)
            .with_context(|| format!("read native declaration {}", native_path.display()))?,
    )?;
    native.validate_package_settings_pages(manifest)?;
    super::projections::load_plugin_settings_templates(manifest, package_root)
}
