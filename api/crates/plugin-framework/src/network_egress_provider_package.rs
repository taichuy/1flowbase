use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::{parse_plugin_manifest, PluginManifestV1},
    PluginConsumptionKind, PluginExecutionMode,
};

pub const NETWORK_EGRESS_PROVIDER_CONTRACT: &str = "1flowbase.network_egress_provider/v1";

/// A third-party runtime package that owns network egress behind the stable v1 contract.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkEgressProviderPackage {
    pub root: PathBuf,
    pub manifest: PluginManifestV1,
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

        Ok(Self { root, manifest })
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
