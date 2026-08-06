use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

use crate::{binding_runtime::lookup_selector_value, compiled_plan::CompiledNode};

pub(super) const VARIABLE_AGGREGATOR_NO_CANDIDATE_VALUE: &str =
    "variable_aggregator_no_candidate_value";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VariableAggregatorExecution {
    pub(crate) input_payload: Map<String, Value>,
    pub(crate) output_payload: Value,
    pub(crate) error_payload: Option<Value>,
    pub(crate) debug_payload: Value,
}

pub(crate) fn variable_aggregator_input_payload(node: &CompiledNode) -> Result<Map<String, Value>> {
    let candidates = variable_aggregator_candidates(node)?;
    Ok(Map::from_iter([(
        "candidates".to_string(),
        candidates.raw_value.clone(),
    )]))
}

pub(crate) fn execute_variable_aggregator_node(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Result<VariableAggregatorExecution> {
    let candidates = variable_aggregator_candidates(node)?;
    let input_payload = Map::from_iter([("candidates".to_string(), candidates.raw_value.clone())]);
    for (index, selector) in candidates.selector_paths.iter().enumerate() {
        let Ok(value) = lookup_selector_value(variable_pool, selector) else {
            continue;
        };

        return Ok(VariableAggregatorExecution {
            input_payload,
            output_payload: json!({ "value": value }),
            error_payload: None,
            debug_payload: json!({
                "matched_candidate": {
                    "index": index,
                    "selector": selector,
                }
            }),
        });
    }

    Ok(VariableAggregatorExecution {
        input_payload,
        output_payload: json!({}),
        error_payload: Some(json!({
            "error_code": VARIABLE_AGGREGATOR_NO_CANDIDATE_VALUE,
            "message": "variable aggregator found no candidate value",
        })),
        debug_payload: json!({}),
    })
}

fn variable_aggregator_candidates(
    node: &CompiledNode,
) -> Result<&crate::compiled_plan::CompiledBinding> {
    let candidates = node
        .bindings
        .get("candidates")
        .ok_or_else(|| anyhow!("variable_aggregator node is missing bindings.candidates"))?;
    if candidates.kind != "selector_list" {
        return Err(anyhow!(
            "variable_aggregator bindings.candidates must be selector_list"
        ));
    }
    Ok(candidates)
}
