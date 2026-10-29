use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::compiled_plan::{CompiledEdge, CompiledNode};
use crate::container_contract::{is_container_type, ContainerConfig, CONTAINER_START_NODE_TYPE};

use super::diagnostics::FlowValidationError;

mod scope;
pub(super) use scope::{container_local_output, is_enclosing_container, selector_is_visible};

fn invalid(node: &CompiledNode, path: &str, message: impl Into<String>) -> anyhow::Error {
    FlowValidationError::node(
        &node.node_id,
        "invalid_container_contract",
        path,
        message,
        None,
    )
    .into()
}

/// Container ownership is a forest; every owned graph has one explicit entry.
/// The ordinary graph compiler independently rejects directed edge cycles.
pub(super) fn validate_containers(
    nodes: &BTreeMap<String, CompiledNode>,
    edges: &[CompiledEdge],
) -> Result<()> {
    let mut children = BTreeMap::<&str, Vec<&CompiledNode>>::new();
    for node in nodes.values() {
        if let Some(parent_id) = &node.container_id {
            let parent = nodes.get(parent_id).ok_or_else(|| {
                invalid(
                    node,
                    "/containerId",
                    format!("unknown container {parent_id}"),
                )
            })?;
            if !is_container_type(&parent.node_type) {
                return Err(invalid(
                    node,
                    "/containerId",
                    "parent must be a loop or iteration",
                ));
            }
            let mut ancestors = BTreeSet::from([node.node_id.as_str()]);
            let mut cursor = Some(parent_id.as_str());
            while let Some(id) = cursor {
                if !ancestors.insert(id) {
                    return Err(invalid(
                        node,
                        "/containerId",
                        "container ownership contains a cycle",
                    ));
                }
                cursor = nodes
                    .get(id)
                    .and_then(|ancestor| ancestor.container_id.as_deref());
            }
            if matches!(
                node.node_type.as_str(),
                "start" | "workflow_start" | "workflow_end" | "answer"
            ) {
                return Err(invalid(
                    node,
                    "/containerId",
                    "workflow entry and terminal nodes cannot be container body nodes",
                ));
            }
            children.entry(parent_id).or_default().push(node);
        } else if node.node_type == CONTAINER_START_NODE_TYPE {
            return Err(invalid(
                node,
                "/containerId",
                "container_start requires a parent container",
            ));
        }
        if node.node_type == CONTAINER_START_NODE_TYPE {
            if !node.bindings.is_empty()
                || !node.outputs.is_empty()
                || !node.dependency_node_ids.is_empty()
            {
                return Err(invalid(
                    node,
                    "/bindings",
                    "container_start cannot have inputs, outputs, or incoming edges",
                ));
            }
        }
        if is_container_type(&node.node_type) {
            validate_settings(node)?;
            validate_selector_types(node, nodes)?;
        }
    }
    for edge in edges {
        let source = nodes
            .get(&edge.source)
            .ok_or_else(|| anyhow!("missing edge source"))?;
        let target = nodes
            .get(&edge.target)
            .ok_or_else(|| anyhow!("missing edge target"))?;
        if source.container_id != target.container_id {
            return Err(invalid(
                target,
                "/containerId",
                format!("edge {} cannot cross a container boundary", edge.edge_id),
            ));
        }
    }
    for container in nodes
        .values()
        .filter(|node| is_container_type(&node.node_type))
    {
        let body = children
            .get(container.node_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let starts: Vec<_> = body
            .iter()
            .filter(|node| node.node_type == CONTAINER_START_NODE_TYPE)
            .collect();
        if starts.len() != 1 {
            return Err(invalid(
                container,
                "/containerId",
                "each container must have exactly one container_start",
            ));
        }
        let mut reached = BTreeSet::new();
        let mut queue = vec![starts[0].node_id.as_str()];
        while let Some(id) = queue.pop() {
            if !reached.insert(id) {
                continue;
            }
            if let Some(node) = nodes.get(id) {
                queue.extend(node.downstream_node_ids.iter().map(String::as_str));
            }
        }
        if let Some(unreachable) = body
            .iter()
            .find(|node| !reached.contains(node.node_id.as_str()))
        {
            return Err(invalid(
                unreachable,
                "/containerId",
                "container body node must be reachable from its container_start",
            ));
        }
    }
    Ok(())
}

fn validate_settings(node: &CompiledNode) -> Result<()> {
    let config = ContainerConfig::from_node(node)
        .map_err(|error| invalid(node, "/config", error.to_string()))?;
    for key in node.bindings.keys() {
        let allowed = if node.node_type == "iteration" {
            matches!(key.as_str(), "items" | "output")
        } else {
            matches!(
                key.as_str(),
                "variables" | "variable_updates" | "output" | "break_condition"
            )
        };
        if !allowed {
            return Err(invalid(
                node,
                &format!("/bindings/{key}"),
                "binding is not part of the container contract",
            ));
        }
    }
    if node.node_type == "iteration" {
        require_binding(node, "items", "selector")?;
        require_binding(node, "output", "selector")?;
        for key in ["variables", "variable_updates", "break_condition"] {
            if node.bindings.contains_key(key) {
                return Err(invalid(
                    node,
                    &format!("/bindings/{key}"),
                    "iteration cannot carry mutable loop state or a loop break condition",
                ));
            }
        }
    } else {
        if let Some(output) = node.bindings.get("output") {
            if output.kind != "selector" {
                return Err(invalid(
                    node,
                    "/bindings/output",
                    "loop output must be a selector",
                ));
            }
        }
        if let Some(condition) = node.bindings.get("break_condition") {
            if condition.kind != "condition_group" || !valid_condition(&condition.raw_value) {
                return Err(invalid(
                    node,
                    "/bindings/break_condition",
                    "break_condition must be a complete condition group",
                ));
            }
        }
        let keys: BTreeSet<_> = config
            .loop_variables
            .iter()
            .map(|var| var.key.as_str())
            .collect();
        let mut initial = BTreeSet::new();
        for key in ["variables", "variable_updates"] {
            let Some(binding) = node.bindings.get(key) else {
                continue;
            };
            if binding.kind != "named_bindings" {
                return Err(invalid(
                    node,
                    &format!("/bindings/{key}"),
                    "loop values must use named_bindings",
                ));
            }
            let entries = binding.raw_value.as_array().ok_or_else(|| {
                invalid(
                    node,
                    &format!("/bindings/{key}"),
                    "named bindings must be an array",
                )
            })?;
            let mut names = BTreeSet::new();
            for entry in entries {
                let name = entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !keys.contains(name) || !names.insert(name) {
                    return Err(invalid(
                        node,
                        &format!("/bindings/{key}"),
                        "loop values must name unique declared variables",
                    ));
                }
                if entry.pointer("/value/kind").and_then(Value::as_str) == Some("constant") {
                    let value = entry.pointer("/value/value").unwrap_or(&Value::Null);
                    let definition = config
                        .loop_variables
                        .iter()
                        .find(|var| var.key == name)
                        .expect("declared above");
                    let matches = match definition.value_type.as_str() {
                        "string" => value.is_string(),
                        "number" => value.is_number(),
                        "boolean" => value.is_boolean(),
                        "object" => value.is_object(),
                        "array" => value.is_array(),
                        _ => false,
                    };
                    if !matches {
                        return Err(invalid(
                            node,
                            &format!("/bindings/{key}"),
                            format!("loop variable {name} must be {}", definition.value_type),
                        ));
                    }
                }
            }
            if key == "variables" {
                initial = names;
            }
        }
        if initial != keys {
            return Err(invalid(
                node,
                "/bindings/variables",
                "every loop variable requires an initial value",
            ));
        }
    }
    for output in &node.outputs {
        let expected = match output.key.as_str() {
            "result" => "array",
            "variables" if node.node_type == "loop" => "object",
            _ => {
                return Err(invalid(
                    node,
                    "/outputs",
                    "containers expose result and loop variables only",
                ))
            }
        };
        if output.value_type != expected {
            return Err(invalid(
                node,
                "/outputs",
                format!("container output {} must be {expected}", output.key),
            ));
        }
    }
    if !node.outputs.iter().any(|output| output.key == "result")
        || (node.node_type == "loop"
            && !node.outputs.iter().any(|output| output.key == "variables"))
    {
        return Err(invalid(
            node,
            "/outputs",
            "containers must declare result; loops must also declare variables",
        ));
    }
    Ok(())
}

fn validate_selector_types(
    node: &CompiledNode,
    nodes: &BTreeMap<String, CompiledNode>,
) -> Result<()> {
    let config = ContainerConfig::from_node(node)?;
    let declared_type = |key: &str| -> Option<String> {
        let selector = node.bindings.get(key)?.selector_paths.first()?;
        let source = nodes.get(selector.first()?)?;
        if let Some(output) = container_local_output(source, selector) {
            return Some(output.value_type);
        }
        source
            .outputs
            .iter()
            .find(|output| {
                (selector.len() == 2 && output.key == selector[1])
                    || output.selector.as_slice() == &selector[1..]
            })
            .map(|output| output.value_type.clone())
    };
    for (key, required) in [
        ("items", node.node_type == "iteration"),
        ("output", config.flatten_output),
    ] {
        if !required {
            continue;
        }
        if let Some(value_type) = declared_type(key) {
            if !matches!(value_type.as_str(), "array" | "json" | "any")
                && !value_type.starts_with("array[")
            {
                return Err(invalid(
                    node,
                    &format!("/bindings/{key}"),
                    format!("{key} requires an array, but the selected output is {value_type}"),
                ));
            }
        }
    }
    Ok(())
}

fn require_binding(node: &CompiledNode, key: &str, kind: &str) -> Result<()> {
    if node.bindings.get(key).is_some_and(|binding| {
        binding.kind == kind
            && binding
                .selector_paths
                .first()
                .is_some_and(|path| path.len() >= 2)
    }) {
        return Ok(());
    }
    Err(invalid(
        node,
        &format!("/bindings/{key}"),
        format!("{key} requires a {kind} binding"),
    ))
}

fn valid_condition(value: &Value) -> bool {
    if let Some(conditions) = value.get("conditions").and_then(Value::as_array) {
        return matches!(
            value.get("operator").and_then(Value::as_str),
            Some("and" | "or")
        ) && !conditions.is_empty()
            && conditions.iter().all(valid_condition);
    }
    let path_valid = |value: &Value| {
        value.as_array().is_some_and(|path| {
            path.len() >= 2
                && path
                    .iter()
                    .all(|part| part.as_str().is_some_and(|part| !part.trim().is_empty()))
        })
    };
    if !path_valid(value.get("left").unwrap_or(&Value::Null)) {
        return false;
    }
    let comparator = value
        .get("comparator")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(comparator, "exists" | "empty") {
        return true;
    }
    if !matches!(
        comparator,
        "equals"
            | "not_equals"
            | "greater_than"
            | "greater_than_or_equals"
            | "less_than"
            | "less_than_or_equals"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "matches_regex"
    ) {
        return false;
    }
    let Some(right) = value.get("right") else {
        return false;
    };
    match right.get("kind").and_then(Value::as_str) {
        Some("selector") => path_valid(right.get("selector").unwrap_or(&Value::Null)),
        Some("constant") if right.get("value").is_some() => {
            comparator != "matches_regex"
                || right
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|pattern| regex::Regex::new(pattern).is_ok())
        }
        _ => false,
    }
}
