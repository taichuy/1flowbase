use super::*;

pub(in crate::compiler) fn output_for_selector(
    nodes: &BTreeMap<String, CompiledNode>,
    selector: &[String],
) -> Option<CompiledOutput> {
    if selector.len() == crate::compiled_plan::SYSTEM_PROTOCOL_CONTEXT_SELECTOR.len()
        && selector
            .iter()
            .zip(crate::compiled_plan::SYSTEM_PROTOCOL_CONTEXT_SELECTOR)
            .all(|(actual, expected)| actual.as_str() == expected)
    {
        return Some(CompiledOutput {
            key: "protocol_context".to_string(),
            title: "sys.protocol_context".to_string(),
            value_type: "json".to_string(),
            selector: Vec::new(),
            json_schema: None,
        });
    }

    let node_id = selector.first()?;
    let selector_tail = selector.get(1..)?;
    let output_key = selector_tail.first()?;
    let node = nodes.get(node_id)?;

    if node.node_type == "start" && output_key == "history" {
        return Some(CompiledOutput {
            key: "history".to_string(),
            title: "userinput.history".to_string(),
            value_type: "array".to_string(),
            selector: Vec::new(),
            json_schema: Some(history_messages_schema()),
        });
    }

    if node.node_type == "start" && output_key == "protocol_context" {
        return Some(CompiledOutput {
            key: "protocol_context".to_string(),
            title: "userinput.protocol_context".to_string(),
            value_type: extension_contracts::provider_contract::PROTOCOL_CONTEXT_VALUE_TYPE
                .to_string(),
            selector: Vec::new(),
            json_schema: None,
        });
    }

    if matches!(node.node_type.as_str(), "start" | "workflow_start") {
        if let Some(output) = start_input_field_output(node, output_key) {
            return Some(output);
        }
    }

    node.outputs
        .iter()
        .find(|output| {
            output.selector == selector_tail
                || (selector_tail.len() == 1 && output.key == *output_key)
        })
        .cloned()
}

fn start_input_field_output(node: &CompiledNode, output_key: &str) -> Option<CompiledOutput> {
    let input_fields = node.config.get("input_fields")?.as_array()?;

    input_fields.iter().find_map(|field| {
        let key = field.get("key")?.as_str()?.trim();
        if key.is_empty() || key != output_key {
            return None;
        }

        let value_type = field
            .get("valueType")
            .or_else(|| field.get("value_type"))
            .and_then(Value::as_str)
            .filter(|value_type| !value_type.trim().is_empty())
            .unwrap_or("string");

        Some(CompiledOutput {
            key: key.to_string(),
            title: format!("userinput.{key}"),
            value_type: value_type.to_string(),
            selector: Vec::new(),
            json_schema: None,
        })
    })
}
