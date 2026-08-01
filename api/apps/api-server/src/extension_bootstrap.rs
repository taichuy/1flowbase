use std::{fs, path::Path};

use anyhow::{Context, Result};
use control_plane::plugin_management::LockedExtensionBootstrapEntry;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LockedExtensionBootstrapManifest {
    schema_version: String,
    defaults: Vec<LockedExtensionBootstrapEntry>,
}

pub fn load_locked_extension_bootstrap(
    api_workspace_root: &Path,
) -> Result<Vec<LockedExtensionBootstrapEntry>> {
    let path = api_workspace_root.join("plugins/default-extensions.lock.json");
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "failed to read extension bootstrap lock at {}",
            path.display()
        )
    })?;
    let manifest: LockedExtensionBootstrapManifest =
        serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "failed to decode extension bootstrap lock at {}",
                path.display()
            )
        })?;
    if manifest.schema_version != "1flowbase.extension-bootstrap-lock/v1" {
        anyhow::bail!("unsupported extension bootstrap lock schema");
    }
    let target_suffix = format!(
        "{}-{}",
        std::env::consts::OS,
        normalized_arch(std::env::consts::ARCH)
    );
    Ok(manifest
        .defaults
        .into_iter()
        .filter(|entry| entry.bundled_path.contains(&target_suffix))
        .collect())
}

fn normalized_arch(arch: &str) -> &str {
    match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_boot_1_lock_manifest_selects_exact_current_target() {
        let root = crate::api_workspace_root().unwrap();
        let entries = load_locked_extension_bootstrap(&root).unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.category.as_str(), "runtime-extensions");
        assert_eq!(entry.artifact_kind, "model_provider");
        assert_eq!(entry.id, "1flowbase.anthropic");
        assert_eq!(entry.version, "0.1.33");
        assert!(entry.checksum.starts_with("sha256:"));
        assert_eq!(entry.source, "official_registry");
        assert!(entry.bootstrap);
    }
}
