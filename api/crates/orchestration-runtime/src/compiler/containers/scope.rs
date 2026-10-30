use std::collections::{BTreeMap, BTreeSet};

use crate::compiled_plan::{CompiledNode, CompiledOutput};
use crate::container_contract::is_container_type;

/// Visibility follows lexical container ownership and connected upstream edges.
/// Body result/update bindings may read immediate children; grandchildren remain private.
pub(in crate::compiler) fn selector_is_visible(
    nodes: &BTreeMap<String, CompiledNode>,
    target: &CompiledNode,
    source: &CompiledNode,
    selector: &[String],
    field_path: &str,
) -> bool {
    let body_binding = is_container_type(&target.node_type)
        && matches!(
            field_path,
            "/bindings/output" | "/bindings/variable_updates"
        );
    let local_binding = body_binding || field_path == "/bindings/break_condition";
    if source.node_id == target.node_id {
        return local_binding && container_local_output(source, selector).is_some();
    }
    if body_binding && source.container_id.as_deref() == Some(target.node_id.as_str()) {
        return true;
    }
    let mut cursor = target;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(cursor.node_id.as_str()) {
            return false;
        }
        if cursor.container_id == source.container_id
            && super::super::topology::node_depends_on(nodes, &cursor.node_id, &source.node_id)
        {
            return true;
        }
        let Some(parent) = cursor.container_id.as_ref().and_then(|id| nodes.get(id)) else {
            return false;
        };
        if parent.node_id == source.node_id {
            return container_local_output(parent, selector).is_some();
        }
        cursor = parent;
    }
}

pub(in crate::compiler) fn container_local_output(
    node: &CompiledNode,
    selector: &[String],
) -> Option<CompiledOutput> {
    if !is_container_type(&node.node_type) {
        return None;
    }
    let key = selector.get(1)?.as_str();
    let value_type = match key {
        "index" if selector.len() == 2 => "number",
        "item" if node.node_type == "iteration" => "json",
        "variables" if node.node_type == "loop" && selector.len() == 2 => "object",
        "variables" if node.node_type == "loop" => {
            let name = selector.get(2)?;
            node.config
                .get("loop_variables")?
                .as_array()?
                .iter()
                .find(|definition| {
                    definition.get("key").and_then(|key| key.as_str()) == Some(name)
                })?
                .get("value_type")?
                .as_str()?
        }
        _ => return None,
    };
    Some(CompiledOutput {
        key: key.to_string(),
        title: key.to_string(),
        value_type: value_type.to_string(),
        selector: selector[1..].to_vec(),
        json_schema: None,
    })
}

pub(in crate::compiler) fn is_enclosing_container(
    nodes: &BTreeMap<String, CompiledNode>,
    target: &CompiledNode,
    container_id: &str,
) -> bool {
    let mut cursor = target.container_id.as_deref();
    let mut seen = BTreeSet::new();
    while let Some(id) = cursor {
        if !seen.insert(id) {
            return false;
        }
        if id == container_id {
            return true;
        }
        cursor = nodes.get(id).and_then(|node| node.container_id.as_deref());
    }
    false
}
