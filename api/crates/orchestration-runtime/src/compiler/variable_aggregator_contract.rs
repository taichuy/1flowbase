use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::compiled_plan::{CompiledBinding, CompiledOutput};

pub(crate) const VARIABLE_GROUPS_BINDING_KIND: &str = "variable_groups";
pub(crate) const VARIABLE_GROUP_VALUE_TYPES: &[&str] =
    &["string", "number", "boolean", "object", "array"];

#[derive(Debug)]
pub(crate) struct VariableAggregatorGroup<'a> {
    pub(crate) key: &'a str,
    pub(crate) value_type: &'a str,
    pub(crate) candidates: Vec<Vec<String>>,
}

pub(crate) fn variable_aggregator_groups(
    binding: &CompiledBinding,
) -> Result<Vec<VariableAggregatorGroup<'_>>> {
    if binding.kind != VARIABLE_GROUPS_BINDING_KIND {
        bail!("variable_aggregator bindings.groups must be variable_groups");
    }
    let raw_groups = binding
        .raw_value
        .as_array()
        .ok_or_else(|| anyhow!("variable_aggregator bindings.groups value must be an array"))?;
    if raw_groups.is_empty() {
        bail!("variable_aggregator must declare at least one variable group");
    }

    let mut groups = Vec::with_capacity(raw_groups.len());
    let mut keys = std::collections::BTreeSet::new();
    for (group_index, raw_group) in raw_groups.iter().enumerate() {
        let group = raw_group
            .as_object()
            .ok_or_else(|| anyhow!("variable_aggregator group {group_index} must be an object"))?;
        if group.len() != 3
            || !group.contains_key("key")
            || !group.contains_key("valueType")
            || !group.contains_key("candidates")
        {
            bail!(
                "variable_aggregator group {group_index} must contain only key, valueType, and candidates"
            );
        }
        let key = group.get("key").and_then(Value::as_str).ok_or_else(|| {
            anyhow!("variable_aggregator group {group_index} key must be a string")
        })?;
        if key.trim().is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || !keys.insert(key)
        {
            bail!(
                "variable_aggregator group keys must be unique, nonempty, and contain only ASCII letters, digits, or underscores"
            );
        }
        let value_type = group
            .get("valueType")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("variable_aggregator group {key} valueType must be a string"))?;
        if !VARIABLE_GROUP_VALUE_TYPES.contains(&value_type) {
            bail!(
                "variable_aggregator group {key} valueType must be string, number, boolean, object, or array"
            );
        }
        let raw_candidates = group
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                anyhow!("variable_aggregator group {key} candidates must be an array")
            })?;
        if raw_candidates.is_empty() {
            bail!("variable_aggregator group {key} must declare at least one candidate");
        }
        let candidates = raw_candidates
            .iter()
            .enumerate()
            .map(|(candidate_index, candidate)| {
                let selector = candidate.as_array().ok_or_else(|| {
                    anyhow!(
                        "variable_aggregator group {key} candidate {candidate_index} must be a selector array"
                    )
                })?;
                if selector.len() < 2 {
                    bail!(
                        "variable_aggregator group {key} candidate {candidate_index} must contain at least two non-empty selector segments"
                    );
                }
                selector
                    .iter()
                    .map(|segment| {
                        segment
                            .as_str()
                            .filter(|segment| !segment.trim().is_empty())
                            .map(str::to_string)
                            .ok_or_else(|| {
                                anyhow!(
                                    "variable_aggregator group {key} candidate {candidate_index} must contain at least two non-empty selector segments"
                                )
                            })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;

        groups.push(VariableAggregatorGroup {
            key,
            value_type,
            candidates,
        });
    }

    Ok(groups)
}

pub(crate) fn validate_variable_aggregator_outputs(
    groups: &[VariableAggregatorGroup<'_>],
    outputs: &[CompiledOutput],
) -> Result<()> {
    if outputs.len() != groups.len() {
        bail!("variable_aggregator outputs must match groups in order and count");
    }
    for (group, output) in groups.iter().zip(outputs) {
        if output.key != group.key
            || output.title != group.key
            || output.value_type != group.value_type
            || output.selector.len() != 1
            || output.selector[0] != group.key
            || output.json_schema.is_some()
        {
            bail!(
                "variable_aggregator output {} must be {{key: {}, title: {}, valueType: {}}}",
                group.key,
                group.key,
                group.key,
                group.value_type
            );
        }
    }
    Ok(())
}

pub(crate) fn normalized_variable_group_value_type(value_type: &str) -> Option<&str> {
    if value_type == "array" || (value_type.starts_with("array[") && value_type.ends_with(']')) {
        return Some("array");
    }
    VARIABLE_GROUP_VALUE_TYPES
        .contains(&value_type)
        .then_some(value_type)
}
