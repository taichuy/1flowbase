use std::collections::BTreeSet;

use serde::Deserialize;

use crate::error::{FrameworkResult, PluginFrameworkError};

pub const DEPLOYMENT_PLUGIN_SET_SCHEMA_V1: &str = "1flowbase.plugin-set/v1";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentPluginSet {
    schema_version: String,
    set_id: String,
    host_extensions: Vec<String>,
    runtime_extensions: Vec<String>,
    capability_plugins: Vec<String>,
}

impl DeploymentPluginSet {
    pub fn set_id(&self) -> &str {
        &self.set_id
    }

    pub fn host_extension_ids(&self) -> &[String] {
        &self.host_extensions
    }

    pub fn runtime_extension_ids(&self) -> &[String] {
        &self.runtime_extensions
    }

    pub fn capability_plugin_ids(&self) -> &[String] {
        &self.capability_plugins
    }
}

pub fn parse_deployment_plugin_set(raw: &str) -> FrameworkResult<DeploymentPluginSet> {
    let set: DeploymentPluginSet = serde_yaml::from_str(raw).map_err(|error| {
        PluginFrameworkError::invalid_provider_package(format!(
            "invalid deployment plugin set: {error}"
        ))
    })?;
    validate_deployment_plugin_set(&set)?;
    Ok(set)
}

fn validate_deployment_plugin_set(set: &DeploymentPluginSet) -> FrameworkResult<()> {
    if set.schema_version != DEPLOYMENT_PLUGIN_SET_SCHEMA_V1 {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "deployment plugin set schema_version must be {DEPLOYMENT_PLUGIN_SET_SCHEMA_V1}"
        )));
    }
    validate_identifier(&set.set_id, "set_id")?;

    let mut module_ids = BTreeSet::new();
    for (field, ids) in [
        ("host_extensions", &set.host_extensions),
        ("runtime_extensions", &set.runtime_extensions),
        ("capability_plugins", &set.capability_plugins),
    ] {
        for module_id in ids {
            validate_identifier(module_id, field)?;
            if !module_ids.insert(module_id.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "deployment plugin set module id {module_id} is declared more than once"
                )));
            }
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &str) -> FrameworkResult<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "deployment plugin set {field} contains invalid identifier {value:?}"
        )));
    }
    Ok(())
}
