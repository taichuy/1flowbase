use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};

const PLUGIN_SET_SCHEMA_V1: &str = "1flowbase.plugin-set/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPluginSetCatalog {
    sets: BTreeMap<String, DeploymentPluginSet>,
}

impl DeploymentPluginSetCatalog {
    pub fn new(sets: Vec<DeploymentPluginSet>) -> Result<Self> {
        let mut indexed = BTreeMap::new();
        for set in sets {
            let set_id = set.set_id.clone();
            if indexed.insert(set_id.clone(), set).is_some() {
                bail!("duplicate plugin set id {set_id}");
            }
        }
        Ok(Self { sets: indexed })
    }

    pub fn get(&self, set_id: &str) -> Option<&DeploymentPluginSet> {
        self.sets.get(set_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListField {
    HostExtensions,
    RuntimeExtensions,
    CapabilityPlugins,
}

pub fn parse_deployment_plugin_set(raw: &str) -> Result<DeploymentPluginSet> {
    let mut schema_version = None;
    let mut set_id = None;
    let mut host_extensions = None;
    let mut runtime_extensions = None;
    let mut capability_plugins = None;
    let mut current_list = None;
    let mut seen_fields = BTreeSet::new();

    for (index, raw_line) in raw.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if line.starts_with(' ') {
            let list = current_list.with_context(|| {
                format!("plugin set line {line_number} has an item outside a list")
            })?;
            let item = trimmed
                .strip_prefix("- ")
                .with_context(|| format!("plugin set line {line_number} must be a list item"))?;
            let item = scalar(item, line_number)?;
            list_values_mut(
                list,
                &mut host_extensions,
                &mut runtime_extensions,
                &mut capability_plugins,
            )
            .push(item);
            continue;
        }

        current_list = None;
        let (field, value) = trimmed.split_once(':').with_context(|| {
            format!("plugin set line {line_number} must contain a field and value")
        })?;
        if !seen_fields.insert(field.to_string()) {
            bail!("plugin set field {field} is declared more than once");
        }
        let value = value.trim();
        match field {
            "schema_version" => schema_version = Some(scalar(value, line_number)?),
            "set_id" => set_id = Some(scalar(value, line_number)?),
            "host_extensions" => {
                host_extensions = Some(parse_list_start(value, field)?);
                current_list = value.is_empty().then_some(ListField::HostExtensions);
            }
            "runtime_extensions" => {
                runtime_extensions = Some(parse_list_start(value, field)?);
                current_list = value.is_empty().then_some(ListField::RuntimeExtensions);
            }
            "capability_plugins" => {
                capability_plugins = Some(parse_list_start(value, field)?);
                current_list = value.is_empty().then_some(ListField::CapabilityPlugins);
            }
            _ => bail!("unknown plugin set field {field}"),
        }
    }

    let set = DeploymentPluginSet {
        schema_version: schema_version.context("plugin set schema_version is required")?,
        set_id: set_id.context("plugin set set_id is required")?,
        host_extensions: host_extensions.context("plugin set host_extensions is required")?,
        runtime_extensions: runtime_extensions
            .context("plugin set runtime_extensions is required")?,
        capability_plugins: capability_plugins
            .context("plugin set capability_plugins is required")?,
    };
    validate_plugin_set(&set)?;
    Ok(set)
}

fn parse_list_start(value: &str, field: &str) -> Result<Vec<String>> {
    if value.is_empty() || value == "[]" {
        return Ok(Vec::new());
    }
    bail!("plugin set {field} must be a block list or []")
}

fn list_values_mut<'a>(
    field: ListField,
    host_extensions: &'a mut Option<Vec<String>>,
    runtime_extensions: &'a mut Option<Vec<String>>,
    capability_plugins: &'a mut Option<Vec<String>>,
) -> &'a mut Vec<String> {
    match field {
        ListField::HostExtensions => host_extensions
            .as_mut()
            .expect("host_extensions list is initialized"),
        ListField::RuntimeExtensions => runtime_extensions
            .as_mut()
            .expect("runtime_extensions list is initialized"),
        ListField::CapabilityPlugins => capability_plugins
            .as_mut()
            .expect("capability_plugins list is initialized"),
    }
}

fn scalar(value: &str, line_number: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("plugin set line {line_number} has an empty scalar");
    }
    let unquoted = if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    };
    if unquoted.trim().is_empty() {
        bail!("plugin set line {line_number} has an empty scalar");
    }
    Ok(unquoted.to_string())
}

fn validate_plugin_set(set: &DeploymentPluginSet) -> Result<()> {
    if set.schema_version != PLUGIN_SET_SCHEMA_V1 {
        bail!("plugin set schema_version must be {PLUGIN_SET_SCHEMA_V1}");
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
            if !module_ids.insert(module_id.clone()) {
                bail!("plugin set module id {module_id} is declared more than once");
            }
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        bail!("plugin set {field} contains invalid identifier {value:?}");
    }
    Ok(())
}
