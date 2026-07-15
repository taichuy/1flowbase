use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use plugin_framework::{
    parse_host_extension_contribution_manifest, parse_plugin_manifest,
    HostExtensionContributionManifest, PluginManifestV1,
};

pub fn builtin_host_extension_manifest_paths() -> Vec<&'static str> {
    vec![
        "plugins/host-extensions/official.identity-host/manifest.yaml",
        "plugins/host-extensions/official.workspace-host/manifest.yaml",
        "plugins/host-extensions/official.plugin-host/manifest.yaml",
        "plugins/host-extensions/official.local-infra-host/manifest.yaml",
        "plugins/host-extensions/official.file-management-host/manifest.yaml",
        "plugins/host-extensions/official.runtime-orchestration-host/manifest.yaml",
    ]
}

pub fn load_builtin_host_extension_manifests(
    api_workspace_root: impl AsRef<Path>,
) -> Result<Vec<(PluginManifestV1, HostExtensionContributionManifest)>> {
    let api_workspace_root = api_workspace_root.as_ref();
    builtin_host_extension_manifest_paths()
        .into_iter()
        .map(|relative_path| {
            load_builtin_host_extension_manifest(api_workspace_root, relative_path)
        })
        .collect()
}

fn load_builtin_host_extension_manifest(
    api_workspace_root: &Path,
    relative_path: &str,
) -> Result<(PluginManifestV1, HostExtensionContributionManifest)> {
    let manifest_path = api_workspace_root.join(relative_path);
    let manifest_raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "failed to read builtin host extension manifest {}",
            display_path(relative_path, &manifest_path)
        )
    })?;
    let manifest = parse_plugin_manifest(&manifest_raw).with_context(|| {
        format!(
            "failed to parse builtin host extension manifest {}",
            display_path(relative_path, &manifest_path)
        )
    })?;

    let contribution_path = manifest_path
        .parent()
        .unwrap_or(api_workspace_root)
        .join(&manifest.runtime.entry);
    let contribution_raw = fs::read_to_string(&contribution_path).with_context(|| {
        format!(
            "failed to read builtin host extension contribution {}",
            contribution_path.display()
        )
    })?;
    let contribution =
        parse_host_extension_contribution_manifest(&contribution_raw).with_context(|| {
            format!(
                "failed to parse builtin host extension contribution {}",
                contribution_path.display()
            )
        })?;

    let plugin_code = manifest
        .plugin_code()
        .with_context(|| format!("invalid builtin host extension id {}", manifest.plugin_id))?;
    if plugin_code != contribution.extension_id {
        bail!(
            "builtin host extension identity mismatch: package {} contribution {}",
            plugin_code,
            contribution.extension_id
        );
    }
    if manifest.version != contribution.version {
        bail!(
            "builtin host extension version mismatch: package {} contribution {}",
            manifest.version,
            contribution.version
        );
    }
    if !contribution.native.library.starts_with("builtin://") {
        bail!(
            "builtin host extension native library must use builtin://: {}",
            contribution.native.library
        );
    }

    Ok((manifest, contribution))
}

fn display_path(relative_path: &str, absolute_path: &Path) -> String {
    format!("{} ({})", relative_path, absolute_path.display())
}
