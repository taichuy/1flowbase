use std::path::Path;

use anyhow::Result;
use plugin_framework::{HostExtensionContributionManifest, PluginManifestV1};

#[path = "../extension_bus/mod.rs"]
pub mod extension_bus;

pub fn load_builtin_host_extension_manifests(
    api_workspace_root: impl AsRef<Path>,
) -> Result<Vec<(PluginManifestV1, HostExtensionContributionManifest)>> {
    Ok(extension_bus::assemble_extension_graph_input(
        api_workspace_root,
        extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )?
    .into_host_extension_manifests())
}
