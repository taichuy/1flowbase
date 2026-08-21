use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::{parse_plugin_manifest, PluginManifestV1},
    provider_contract::PluginFormSchema,
    PluginConsumptionKind, PluginExecutionMode,
};

pub const NETWORK_EGRESS_PROVIDER_CONTRACT: &str = "1flowbase.network_egress_provider/v1";

/// A third-party runtime package that owns network egress behind the stable v1 contract.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkEgressProviderPackage {
    pub root: PathBuf,
    pub manifest: PluginManifestV1,
    pub provider: NetworkEgressProviderDefinition,
}

/// Provider-owned instance settings.  The host owns the instance name and description; this
/// schema declares only the plugin-specific configuration rendered by Network Center.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NetworkEgressProviderDefinition {
    pub provider_code: String,
    pub display_name: String,
    pub form_schema: PluginFormSchema,
}

impl NetworkEgressProviderPackage {
    pub fn load_from_dir(path: impl AsRef<Path>) -> FrameworkResult<Self> {
        let root = path.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "network egress provider package root must be a directory: {}",
                root.display()
            )));
        }

        let manifest_path = root.join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| PluginFrameworkError::io(Some(&manifest_path), error.to_string()))?;
        let manifest = parse_plugin_manifest(&manifest_raw)?;
        validate_manifest(&manifest)?;

        let runtime_entry = root.join(&manifest.runtime.entry);
        if !runtime_entry.is_file() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "runtime entry does not exist: {}",
                runtime_entry.display()
            )));
        }

        let provider_path = root.join("provider/egress-provider.yaml");
        let provider_raw = fs::read_to_string(&provider_path)
            .map_err(|error| PluginFrameworkError::io(Some(&provider_path), error.to_string()))?;
        let provider: NetworkEgressProviderDefinition = serde_yaml::from_str(&provider_raw)
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
        validate_provider_definition(&provider)?;

        Ok(Self {
            root,
            manifest,
            provider,
        })
    }

    pub fn identifier(&self) -> String {
        self.manifest
            .versioned_plugin_id()
            .expect("network egress provider package manifest identity is validated")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.yaml")
    }

    pub fn runtime_entry(&self) -> PathBuf {
        self.root.join(&self.manifest.runtime.entry)
    }
}

fn validate_provider_definition(provider: &NetworkEgressProviderDefinition) -> FrameworkResult<()> {
    if provider.provider_code.trim().is_empty() || provider.display_name.trim().is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "network egress provider definition requires provider_code and display_name",
        ));
    }
    if provider.form_schema.schema_version != "1flowbase.plugin.form/v1" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "network egress provider form schema must declare schema_version=1flowbase.plugin.form/v1",
        ));
    }
    if provider.form_schema.fields.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "network egress provider form schema must declare at least one field",
        ));
    }
    if provider.form_schema.fields.iter().any(|field| {
        field.key.trim().is_empty() || field.label.trim().is_empty() || field.field_type != "string"
    }) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "network egress provider form fields must be labeled string values",
        ));
    }
    Ok(())
}

fn validate_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    if manifest.consumption_kind != PluginConsumptionKind::RuntimeExtension
        || manifest.slot_codes.as_slice() != ["network_egress_provider"]
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "network egress provider package must declare only the network_egress_provider runtime slot",
        ));
    }
    if manifest.contract_version != NETWORK_EGRESS_PROVIDER_CONTRACT {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "network egress provider package must declare contract_version={NETWORK_EGRESS_PROVIDER_CONTRACT}",
        )));
    }
    if manifest.execution_mode != PluginExecutionMode::StatefulRuntimeWorker
        || manifest.runtime.protocol != "stdio_json_worker"
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "network egress provider package must declare execution_mode=stateful_runtime_worker with runtime.protocol=stdio_json_worker",
        ));
    }
    Ok(())
}
