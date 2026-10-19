use serde::Deserialize;

use crate::error::{FrameworkResult, PluginFrameworkError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionSourceKind {
    Builtin,
    FilesystemDropin,
    Uploaded,
}

impl HostExtensionSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::FilesystemDropin => "filesystem_dropin",
            Self::Uploaded => "uploaded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionActivationPhase {
    Boot,
}

impl HostExtensionActivationPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boot => "boot",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionInterfaceManifest {
    pub code: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionStorageManifest {
    pub kind: String,
    pub implementation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionDependencyManifest {
    pub extension_id: String,
    pub version_range: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionLoadOrderManifest {
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default)]
    pub before: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionManifestV1 {
    pub manifest_version: u32,
    pub extension_id: String,
    pub version: String,
    pub display_name: String,
    pub source_kind: HostExtensionSourceKind,
    pub trust_level: String,
    pub activation_phase: HostExtensionActivationPhase,
    #[serde(default)]
    pub provides_contracts: Vec<String>,
    #[serde(default)]
    pub overrides_contracts: Vec<String>,
    #[serde(default)]
    pub registers_slots: Vec<String>,
    #[serde(default)]
    pub registers_interfaces: Vec<HostExtensionInterfaceManifest>,
    #[serde(default)]
    pub registers_storage: Vec<HostExtensionStorageManifest>,
    #[serde(default)]
    pub dependencies: Vec<HostExtensionDependencyManifest>,
    #[serde(default)]
    pub load_order: HostExtensionLoadOrderManifest,
}

pub fn parse_host_extension_manifest(raw: &str) -> FrameworkResult<HostExtensionManifestV1> {
    let manifest: HostExtensionManifestV1 = serde_yaml::from_str(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
    validate_host_extension_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_host_extension_manifest(manifest: &HostExtensionManifestV1) -> FrameworkResult<()> {
    if manifest.manifest_version != 1 {
        return Err(PluginFrameworkError::invalid_provider_package(
            "manifest_version must be 1",
        ));
    }
    validate_required(&manifest.extension_id, "extension_id")?;
    validate_required(&manifest.version, "version")?;
    validate_required(&manifest.display_name, "display_name")?;
    validate_required(&manifest.trust_level, "trust_level")?;

    for contract in manifest
        .provides_contracts
        .iter()
        .chain(manifest.overrides_contracts.iter())
    {
        validate_required(contract, "contracts")?;
    }
    for slot in &manifest.registers_slots {
        validate_required(slot, "registers_slots")?;
    }
    for interface in &manifest.registers_interfaces {
        validate_required(&interface.code, "registers_interfaces.code")?;
        validate_required(&interface.kind, "registers_interfaces.kind")?;
    }
    for storage in &manifest.registers_storage {
        validate_required(&storage.kind, "registers_storage.kind")?;
        validate_required(&storage.implementation, "registers_storage.implementation")?;
    }
    for dependency in &manifest.dependencies {
        validate_required(&dependency.extension_id, "dependencies.extension_id")?;
        validate_required(&dependency.version_range, "dependencies.version_range")?;
    }
    for extension_id in manifest
        .load_order
        .after
        .iter()
        .chain(manifest.load_order.before.iter())
    {
        validate_required(extension_id, "load_order")?;
    }

    Ok(())
}

fn validate_required(value: &str, field: &'static str) -> FrameworkResult<()> {
    if value.trim().is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}
