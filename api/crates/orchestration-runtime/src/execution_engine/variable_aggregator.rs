use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

use crate::{
    binding_runtime::lookup_selector_value, compiled_plan::CompiledNode,
    output_schema::validate_output_value, variable_aggregator_contract::variable_aggregator_groups,
};

pub(super) const VARIABLE_AGGREGATOR_NO_CANDIDATE_VALUE: &str =
    "variable_aggregator_no_candidate_value";
pub(super) const VARIABLE_AGGREGATOR_OUTPUT_TYPE_MISMATCH: &str =
    "variable_aggregator_output_type_mismatch";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VariableAggregatorExecution {
    pub(crate) input_payload: Map<String, Value>,
    pub(crate) output_payload: Value,
    pub(crate) error_payload: Option<Value>,
    pub(crate) debug_payload: Value,
}

pub(crate) fn variable_aggregator_input_payload(node: &CompiledNode) -> Result<Map<String, Value>> {
    let groups = variable_aggregator_binding(node)?;
    Ok(Map::from_iter([(
        "groups".to_string(),
        groups.raw_value.clone(),
    )]))
}

pub(crate) fn execute_variable_aggregator_node(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Result<VariableAggregatorExecution> {
    let binding = variable_aggregator_binding(node)?;
    let groups = variable_aggregator_groups(binding)?;
    let input_payload = Map::from_iter([("groups".to_string(), binding.raw_value.clone())]);
    let mut output_payload = Map::new();
    let mut matched_candidates = Map::new();

    for (group_index, group) in groups.iter().enumerate() {
        let output = node.outputs.get(group_index).ok_or_else(|| {
            anyhow!(
                "variable_aggregator group {} is missing its compiled output",
                group.key
            )
        })?;
        let mut matched = false;
        for (candidate_index, selector) in group.candidates.iter().enumerate() {
            let Ok(value) = lookup_selector_value(variable_pool, selector) else {
                continue;
            };
            if validate_output_value(output, &value).is_err() {
                return Ok(VariableAggregatorExecution {
                    input_payload,
                    output_payload: json!({}),
                    error_payload: Some(json!({
                        "error_code": VARIABLE_AGGREGATOR_OUTPUT_TYPE_MISMATCH,
                        "message": "variable aggregator candidate does not match the group output type",
                        "group_key": group.key,
                        "expected_value_type": group.value_type,
                        "actual_value_type": json_value_type(&value),
                        "candidate_index": candidate_index,
                        "selector": selector,
                    })),
                    debug_payload: json!({}),
                });
            }

            output_payload.insert(group.key.to_string(), value.clone());
            matched_candidates.insert(
                group.key.to_string(),
                json!({ "index": candidate_index, "selector": selector }),
            );
            matched = true;
            break;
        }

        if !matched {
            return Ok(VariableAggregatorExecution {
                input_payload,
                output_payload: json!({}),
                error_payload: Some(json!({
                    "error_code": VARIABLE_AGGREGATOR_NO_CANDIDATE_VALUE,
                    "message": "variable aggregator group found no candidate value",
                    "group_key": group.key,
                })),
                debug_payload: json!({}),
            });
        }
    }

    Ok(VariableAggregatorExecution {
        input_payload,
        output_payload: Value::Object(output_payload),
        error_payload: None,
        debug_payload: json!({ "matched_candidates": matched_candidates }),
    })
}

fn variable_aggregator_binding(
    node: &CompiledNode,
) -> Result<&crate::compiled_plan::CompiledBinding> {
    node.bindings
        .get("groups")
        .ok_or_else(|| anyhow!("variable_aggregator node is missing bindings.groups"))
}

fn json_value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
