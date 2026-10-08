use std::collections::BTreeMap;

use super::topology::variable_aggregator_output_for_selector;
use super::{FlowCompileContext, FlowValidationError};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use crate::compiled_plan::{CompiledBinding, CompiledNode, CompiledOutput};

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
    node_id: &str,
    groups: &[VariableAggregatorGroup<'_>],
    outputs: &[CompiledOutput],
) -> Result<()> {
    if outputs.len() != groups.len() {
        return Err(FlowValidationError::node(
            node_id,
            "variable_aggregator_output_mismatch",
            "/outputs",
            "variable_aggregator outputs must match groups in order and count",
            Some(serde_json::json!({ "count": groups.len() })),
        )
        .into());
    }
    for (index, (group, output)) in groups.iter().zip(outputs).enumerate() {
        let checks = [
            ("key", output.key != group.key, serde_json::json!(group.key)),
            (
                "title",
                output.title != group.key,
                serde_json::json!(group.key),
            ),
            (
                "valueType",
                output.value_type != group.value_type,
                serde_json::json!(group.value_type),
            ),
            (
                "selector",
                output.selector.len() != 1 || output.selector[0] != group.key,
                serde_json::json!([group.key]),
            ),
            ("jsonSchema", output.json_schema.is_some(), Value::Null),
        ];
        for (field, invalid, expected) in checks {
            if invalid {
                return Err(FlowValidationError::node(
                    node_id,
                    "variable_aggregator_output_mismatch",
                    &format!("/outputs/{index}/{field}"),
                    format!(
                        "variable_aggregator output {field} must match group {}",
                        group.key
                    ),
                    Some(expected),
                )
                .into());
            }
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

pub(super) fn validate_variable_aggregator_topology(
    document: &Value,
    context: &FlowCompileContext,
    nodes: &BTreeMap<String, CompiledNode>,
) -> Result<()> {
    for node in nodes
        .values()
        .filter(|node| node.node_type == "variable_aggregator")
    {
        let binding = node
            .bindings
            .get("groups")
            .ok_or_else(|| anyhow!("node {} is missing bindings.groups", node.node_id))?;
        let groups =
            crate::compiler::variable_aggregator_contract::variable_aggregator_groups(binding)?;
        for (group_index, group) in groups.into_iter().enumerate() {
            for (candidate_index, selector) in group.candidates.into_iter().enumerate() {
                let path =
                    format!("/bindings/groups/value/{group_index}/candidates/{candidate_index}");
                let output = variable_aggregator_output_for_selector(document, context, nodes, &selector)
                    .with_context(|| format!(
                        "node {} variable_aggregator group {} selector {} has no valid declared type",
                        node.node_id, group.key, selector.join("."),
                    ))
                    .map_err(|error| FlowValidationError::at_node(&node.node_id, &path, error))?;
                let actual = crate::compiler::variable_aggregator_contract::normalized_variable_group_value_type(&output.value_type)
                    .ok_or_else(|| FlowValidationError::node(
                        &node.node_id, "variable_aggregator_candidate_type_mismatch", &path,
                        format!("node {} variable_aggregator group {} selector {} uses forbidden upstream valueType {}",
                            node.node_id, group.key, selector.join("."), output.value_type),
                        Some(serde_json::json!(group.value_type)),
                    ))?;
                if actual != group.value_type {
                    return Err(FlowValidationError::node(
                        &node.node_id, "variable_aggregator_candidate_type_mismatch", &path,
                        format!("node {} variable_aggregator group {} expects valueType {} but selector {} declares {}",
                            node.node_id, group.key, group.value_type, selector.join("."), output.value_type),
                        Some(serde_json::json!(group.value_type)),
                    ).into());
                }
            }
        }
    }
    Ok(())
}
