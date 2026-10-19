use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    capability_kind::PluginConsumptionKind,
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::PluginManifestV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostExtensionDropinPolicy {
    pub allow_unverified_filesystem_dropins: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedHostExtensionInstallation {
    pub package_root: PathBuf,
    pub manifest_path: PathBuf,
    pub plugin_id: String,
    pub source_kind: String,
    pub trust_level: String,
    pub manifest: PluginManifestV1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostExtensionDropinScan {
    pub installations: Vec<DetectedHostExtensionInstallation>,
    pub warnings: Vec<String>,
}

pub fn scan_host_extension_dropins(
    dropin_root: impl AsRef<Path>,
) -> FrameworkResult<HostExtensionDropinScan> {
    scan_host_extension_dropins_with_policy(dropin_root, HostExtensionDropinPolicy::default())
}

pub fn scan_host_extension_dropins_with_policy(
    dropin_root: impl AsRef<Path>,
    policy: HostExtensionDropinPolicy,
) -> FrameworkResult<HostExtensionDropinScan> {
    let dropin_root = dropin_root.as_ref();
    if !dropin_root.is_dir() {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "host extension dropin root must be a directory: {}",
            dropin_root.display()
        )));
    }

    let mut package_roots = Vec::new();
    collect_dropin_package_roots(dropin_root, &mut package_roots)?;
    package_roots.sort();

    let mut installations = Vec::with_capacity(package_roots.len());
    let mut warnings = Vec::new();

    for package_root in package_roots {
        let manifest_path = package_root.join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| PluginFrameworkError::io(Some(&manifest_path), error.to_string()))?;
        let manifest = crate::parse_plugin_manifest(&manifest_raw)?;
        validate_host_extension_dropin_manifest(&manifest)?;

        let package_id = manifest.versioned_plugin_id()?;

        if !policy.allow_unverified_filesystem_dropins && manifest.trust_level == "unverified" {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "filesystem_dropin package {} is unverified and policy disallows it",
                manifest.plugin_id
            )));
        }

        if manifest.trust_level == "unverified" {
            warnings.push(format!(
                "filesystem_dropin package {} is unverified; allowed by policy",
                manifest.plugin_id
            ));
        }

        installations.push(DetectedHostExtensionInstallation {
            package_root: package_root.clone(),
            manifest_path,
            plugin_id: package_id,
            source_kind: manifest.source_kind.clone(),
            trust_level: manifest.trust_level.clone(),
            manifest,
        });
    }

    Ok(HostExtensionDropinScan {
        installations,
        warnings,
    })
}

fn collect_dropin_package_roots(
    current: &Path,
    package_roots: &mut Vec<PathBuf>,
) -> FrameworkResult<()> {
    if current.join("manifest.yaml").is_file() {
        package_roots.push(current.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(current)
        .map_err(|error| PluginFrameworkError::io(Some(current), error.to_string()))?
    {
        let path = entry
            .map_err(|error| PluginFrameworkError::io(Some(current), error.to_string()))?
            .path();
        if path.is_dir() {
            collect_dropin_package_roots(&path, package_roots)?;
        }
    }

    Ok(())
}

fn validate_host_extension_dropin_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    if manifest.consumption_kind != PluginConsumptionKind::HostExtension {
        return Err(PluginFrameworkError::invalid_provider_package(
            "filesystem drop-in package must declare consumption_kind=host_extension",
        ));
    }
    if manifest.source_kind != "filesystem_dropin" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "drop-in package must resolve to source_kind=filesystem_dropin",
        ));
    }

    Ok(())
}
