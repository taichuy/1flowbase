use super::*;
use crate::binding_runtime::lookup_selector_value;
use crate::node_error_policy::ERROR_BRANCH_SOURCE_HANDLE;

pub fn initial_active_node_ids(plan: &CompiledPlan) -> BTreeSet<String> {
    let mounted_llm_target_node_ids = visible_internal_llm_tool_target_node_ids(plan);

    if plan.edges.is_empty() {
        return plan
            .topological_order
            .iter()
            .filter(|node_id| !mounted_llm_target_node_ids.contains(*node_id))
            .cloned()
            .collect();
    }

    plan.nodes
        .values()
        .filter(|node| node.dependency_node_ids.is_empty())
        .filter(|node| !mounted_llm_target_node_ids.contains(&node.node_id))
        .map(|node| node.node_id.clone())
        .collect()
}

fn outgoing_edges<'a>(
    plan: &'a CompiledPlan,
    node_id: &'a str,
) -> impl Iterator<Item = &'a CompiledEdge> + 'a {
    plan.edges.iter().filter(move |edge| edge.source == node_id)
}

pub fn activate_downstream_nodes(
    plan: &CompiledPlan,
    active_node_ids: &mut BTreeSet<String>,
    node: &CompiledNode,
    selected_source_handle: Option<&str>,
) -> bool {
    if plan.edges.is_empty() {
        let mut activated = false;
        for target_node_id in &node.downstream_node_ids {
            active_node_ids.insert(target_node_id.clone());
            activated = true;
        }
        return activated;
    }

    let mut activated = false;

    for edge in outgoing_edges(plan, &node.node_id) {
        if node.node_type == "if_else" && edge.source_handle.as_deref() != selected_source_handle {
            continue;
        }

        if node.node_type != "if_else" {
            let edge_source_handle = edge.source_handle.as_deref();
            if let Some(selected_source_handle) = selected_source_handle {
                if edge_source_handle != Some(selected_source_handle) {
                    continue;
                }
            } else if edge_source_handle == Some(ERROR_BRANCH_SOURCE_HANDLE)
                || is_visible_internal_llm_tool_source_handle(edge_source_handle)
            {
                continue;
            }
        }

        active_node_ids.insert(edge.target.clone());
        activated = true;
    }

    activated
}

pub fn checkpoint_active_node_ids(active_node_ids: &BTreeSet<String>) -> Vec<String> {
    active_node_ids.iter().cloned().collect()
}

pub fn select_if_else_source_handle(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Result<Option<String>> {
    let branches = node
        .bindings
        .get("branches")
        .ok_or_else(|| anyhow!("if_else node {} missing branches binding", node.node_id))?
        .raw_value
        .get("branches")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("if_else node {} branches must be an array", node.node_id))?;
    let mut else_handle = None;

    for branch in branches {
        let kind = branch
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("else_if");
        let source_handle = branch_source_handle(branch)?;

        if kind == "else" {
            else_handle = Some(source_handle);
            continue;
        }

        if let Some(condition) = branch.get("condition") {
            if evaluate_condition_group(condition, variable_pool)? {
                return Ok(Some(source_handle));
            }
        }
    }

    Ok(else_handle)
}

fn branch_source_handle(branch: &Value) -> Result<String> {
    branch
        .get("sourceHandle")
        .or_else(|| branch.get("source_handle"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|handle| !handle.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("if_else branch missing sourceHandle"))
}

fn evaluate_condition_group(group: &Value, variable_pool: &Map<String, Value>) -> Result<bool> {
    let operator = group
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or("and");
    let conditions = group
        .get("conditions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("condition group must include conditions"))?;

    if conditions.is_empty() {
        return Ok(false);
    }

    if operator == "or" {
        for condition in conditions {
            if evaluate_condition_expression(condition, variable_pool)? {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    for condition in conditions {
        if !evaluate_condition_expression(condition, variable_pool)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn evaluate_condition_expression(
    condition: &Value,
    variable_pool: &Map<String, Value>,
) -> Result<bool> {
    if condition
        .get("conditions")
        .and_then(Value::as_array)
        .is_some()
    {
        return evaluate_condition_group(condition, variable_pool);
    }

    evaluate_condition_rule(condition, variable_pool)
}

fn evaluate_condition_rule(rule: &Value, variable_pool: &Map<String, Value>) -> Result<bool> {
    let left_selector = selector_value_path(rule.get("left").unwrap_or(&Value::Null))?;
    let comparator = rule
        .get("comparator")
        .and_then(Value::as_str)
        .unwrap_or("exists");
    let left_value = match lookup_selector_value(variable_pool, &left_selector) {
        Ok(value) => value,
        Err(_) if comparator == "empty" => return Ok(true),
        Err(_) => return Ok(false),
    };

    if comparator == "exists" {
        return Ok(!left_value.is_null());
    }
    if comparator == "empty" {
        return Ok(is_empty_condition_value(&left_value));
    }

    let Some(right_value) = rule
        .get("right")
        .map(|value| resolve_condition_value(value, variable_pool))
        .transpose()?
    else {
        return Ok(false);
    };

    Ok(match comparator {
        "equals" => left_value == right_value,
        "not_equals" => left_value != right_value,
        "greater_than" => numeric_value(&left_value) > numeric_value(&right_value),
        "greater_than_or_equals" => numeric_value(&left_value) >= numeric_value(&right_value),
        "less_than" => numeric_value(&left_value) < numeric_value(&right_value),
        "less_than_or_equals" => numeric_value(&left_value) <= numeric_value(&right_value),
        "contains" => string_value(&left_value).contains(&string_value(&right_value)),
        "starts_with" => string_value(&left_value).starts_with(&string_value(&right_value)),
        "ends_with" => string_value(&left_value).ends_with(&string_value(&right_value)),
        "matches_regex" => regex::Regex::new(&string_value(&right_value))
            .map(|regex| regex.is_match(&string_value(&left_value)))
            .unwrap_or(false),
        _ => false,
    })
}

fn resolve_condition_value(value: &Value, variable_pool: &Map<String, Value>) -> Result<Value> {
    if value.is_array() {
        let selector = selector_value_path(value)?;

        return lookup_selector_value(variable_pool, &selector);
    }

    match value.get("kind").and_then(Value::as_str) {
        Some("selector") => {
            let selector = selector_value_path(value.get("selector").unwrap_or(&Value::Null))?;

            lookup_selector_value(variable_pool, &selector)
        }
        Some("constant") => Ok(value.get("value").cloned().unwrap_or(Value::Null)),
        _ => Ok(value.clone()),
    }
}

fn selector_value_path(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or_else(|| anyhow!("condition selector must be an array"))?
        .iter()
        .map(|segment| {
            segment
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("condition selector segment must be a string"))
        })
        .collect()
}

fn numeric_value(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .unwrap_or(f64::NAN)
}

fn string_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn is_empty_condition_value(value: &Value) -> bool {
    value.is_null()
        || value.as_str().is_some_and(str::is_empty)
        || value.as_array().is_some_and(Vec::is_empty)
}
