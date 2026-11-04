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

    if node.node_type == "start" && output_key == "history" && selector_tail.len() == 1 {
        return Some(CompiledOutput {
            key: "history".to_string(),
            title: "userinput.history".to_string(),
            value_type: "array".to_string(),
            selector: Vec::new(),
            json_schema: Some(history_messages_schema()),
        });
    }

    if node.node_type == "start" && output_key == "protocol_context" && selector_tail.len() == 1 {
        return Some(CompiledOutput {
            key: "protocol_context".to_string(),
            title: "userinput.protocol_context".to_string(),
            value_type: extension_contracts::provider_contract::PROTOCOL_CONTEXT_VALUE_TYPE
                .to_string(),
            selector: Vec::new(),
            json_schema: None,
        });
    }

    let contract = super::super::output_contract::selector_contract(node, selector_tail);
    let declared = super::super::output_contract::source_outputs(node)
        .into_iter()
        .filter(|output| selector_tail.starts_with(&output.selector))
        .max_by_key(|output| output.selector.len())?;
    if declared.selector == selector_tail {
        return Some(declared);
    }
    match contract {
        super::super::output_contract::PathContract::Missing => None,
        super::super::output_contract::PathContract::Dynamic => Some(CompiledOutput {
            key: selector_tail.last()?.clone(),
            title: selector_tail.join("."),
            value_type: "json".into(),
            selector: selector_tail.to_vec(),
            json_schema: None,
        }),
        super::super::output_contract::PathContract::Known(schema) => Some(CompiledOutput {
            key: selector_tail.last()?.clone(),
            title: selector_tail.join("."),
            value_type: super::super::output_contract::schema_value_type(&schema),
            selector: selector_tail.to_vec(),
            json_schema: Some(schema),
        }),
    }
}
