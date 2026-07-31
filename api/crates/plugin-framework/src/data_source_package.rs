use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    capability_kind::PluginConsumptionKind,
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::{parse_plugin_manifest, PluginManifestV1},
    provider_contract::PluginFormFieldSchema,
    PluginExecutionMode, DATA_SOURCE_NATIVE_SQL_CAPABILITY,
};

#[derive(Debug, Clone, PartialEq)]
pub struct DataSourceDefinition {
    pub source_code: String,
    pub display_name: String,
    pub auth_modes: Vec<String>,
    pub capabilities: Vec<String>,
    pub supports_sync: bool,
    pub supports_webhook: bool,
    pub resource_kinds: Vec<String>,
    pub config_schema: Vec<PluginFormFieldSchema>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataSourcePackage {
    pub root: PathBuf,
    pub manifest: PluginManifestV1,
    pub definition: DataSourceDefinition,
}

impl DataSourcePackage {
    pub fn load_from_dir(path: impl AsRef<Path>) -> FrameworkResult<Self> {
        let root = path.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "data source package root must be a directory: {}",
                root.display()
            )));
        }

        let manifest_path = root.join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| PluginFrameworkError::io(Some(&manifest_path), error.to_string()))?;
        let manifest = parse_plugin_manifest(&manifest_raw)?;
        validate_manifest(&manifest)?;

        let source_code = source_code_from_plugin_id(&manifest)?;
        let definition_path = root.join("datasource").join(format!("{source_code}.yaml"));
        let raw_definition: RawDataSourceDefinition = load_yaml(&definition_path)?;
        if raw_definition.source_code != source_code {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "source_code {} does not match plugin_id prefix {}",
                raw_definition.source_code, source_code
            )));
        }

        Ok(Self {
            root,
            manifest,
            definition: DataSourceDefinition {
                source_code: raw_definition.source_code,
                display_name: raw_definition
                    .display_name
                    .unwrap_or_else(|| "Unnamed Data Source".to_string()),
                auth_modes: raw_definition.auth_modes,
                capabilities: raw_definition.capabilities,
                supports_sync: raw_definition.supports_sync,
                supports_webhook: raw_definition.supports_webhook,
                resource_kinds: raw_definition.resource_kinds,
                config_schema: raw_definition.config_schema,
            },
        })
    }

    pub fn identifier(&self) -> String {
        self.manifest
            .versioned_plugin_id()
            .expect("data source package manifest identity is validated")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.yaml")
    }

    pub fn runtime_entry(&self) -> PathBuf {
        self.root.join(&self.manifest.runtime.entry)
    }

    pub fn supports_native_sql(&self) -> bool {
        self.definition
            .capabilities
            .iter()
            .any(|capability| capability == DATA_SOURCE_NATIVE_SQL_CAPABILITY)
    }
}

#[derive(Debug, Deserialize)]
struct RawDataSourceDefinition {
    source_code: String,
    display_name: Option<String>,
    #[serde(default)]
    auth_modes: Vec<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    supports_sync: bool,
    #[serde(default)]
    supports_webhook: bool,
    #[serde(default)]
    resource_kinds: Vec<String>,
    #[serde(default)]
    config_schema: Vec<PluginFormFieldSchema>,
}

fn validate_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    if manifest.consumption_kind != PluginConsumptionKind::RuntimeExtension {
        return Err(PluginFrameworkError::invalid_provider_package(
            "data source package must declare consumption_kind=runtime_extension",
        ));
    }
    if !manifest.slot_codes.iter().any(|slot| slot == "data_source") {
        return Err(PluginFrameworkError::invalid_provider_package(
            "data source package must declare slot_codes including data_source",
        ));
    }
    if manifest.contract_version != "1flowbase.data_source/v1" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "data source package must declare contract_version=1flowbase.data_source/v1",
        ));
    }
    if manifest.execution_mode != PluginExecutionMode::ProcessPerCall {
        return Err(PluginFrameworkError::invalid_provider_package(
            "data source package must declare execution_mode=process_per_call",
        ));
    }
    if manifest.runtime.protocol != "stdio_json" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "data source package must declare runtime.protocol=stdio_json",
        ));
    }
    Ok(())
}

fn source_code_from_plugin_id(manifest: &PluginManifestV1) -> FrameworkResult<&str> {
    manifest.plugin_code()
}

fn load_yaml<T>(path: &Path) -> FrameworkResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path)
        .map_err(|error| PluginFrameworkError::io(Some(path), error.to_string()))?;
    serde_yaml::from_str::<T>(&content)
        .map_err(|error| PluginFrameworkError::serialization(Some(path), error.to_string()))
}
