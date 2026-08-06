use super::*;

pub(super) fn compile_variable_reference(
    field: &str,
    value: Option<&Value>,
) -> Result<Option<crate::compiled_plan::VariableReference>> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("{field} must be a selector binding or null"))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "kind" | "value"))
    {
        bail!("{field} must contain only kind and value");
    }
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{field}.kind must be selector"))?;
    if kind != "selector" {
        bail!("{field}.kind must be selector");
    }
    let selector = extract_selector_paths(kind, object.get("value").unwrap_or(&Value::Null))?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("{field}.value must contain one selector path"))?;
    if selector.len() < 2 || selector.iter().any(|segment| segment.trim().is_empty()) {
        bail!("{field}.value must contain at least two non-empty selector segments");
    }

    Ok(Some(crate::compiled_plan::VariableReference::selector(
        selector,
    )))
}

pub(super) fn extract_selector_paths(kind: &str, raw_value: &Value) -> Result<Vec<Vec<String>>> {
    match kind {
        "i18n_text" => Ok(Vec::new()),
        "templated_text" => {
            let template = raw_value
                .as_str()
                .ok_or_else(|| anyhow!("templated_text binding value must be a string"))?;
            Ok(parse_template_selector_tokens(template))
        }
        "selector" => Ok(vec![selector_path(raw_value)?]),
        "selector_list" => selector_path_list(raw_value),
        "variable_groups" => {
            let binding = crate::compiled_plan::CompiledBinding {
                i18n_text_ref: None,
                kind: kind.to_string(),
                raw_value: raw_value.clone(),
                selector_paths: Vec::new(),
            };
            Ok(
                crate::compiler::variable_aggregator_contract::variable_aggregator_groups(
                    &binding,
                )?
                .into_iter()
                .flat_map(|group| group.candidates)
                .collect(),
            )
        }
        "named_bindings" => {
            let entries = raw_value
                .as_array()
                .ok_or_else(|| anyhow!("named_bindings value must be an array"))?;
            let mut selectors = Vec::new();

            for entry in entries {
                if let Some(value) = entry.get("value").and_then(Value::as_object) {
                    match value.get("kind").and_then(Value::as_str) {
                        Some("constant") => continue,
                        Some("selector") => {
                            selectors.push(selector_path(
                                value.get("selector").unwrap_or(&Value::Null),
                            )?);
                            continue;
                        }
                        Some("templated_text") => {
                            let template =
                                value.get("value").and_then(Value::as_str).ok_or_else(|| {
                                    anyhow!("named_bindings templated_text value must be a string")
                                })?;
                            selectors.extend(parse_template_selector_tokens(template));
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(content) = entry
                    .get("content")
                    .and_then(|content| content.get("value"))
                    .and_then(Value::as_str)
                {
                    selectors.extend(parse_template_selector_tokens(content));
                    continue;
                }

                selectors.push(selector_path(
                    entry.get("selector").unwrap_or(&Value::Null),
                )?);
            }

            Ok(selectors)
        }
        "prompt_messages" => {
            let entries = raw_value
                .as_array()
                .ok_or_else(|| anyhow!("prompt_messages value must be an array"))?;
            let mut selectors = Vec::new();

            for entry in entries {
                let content = entry
                    .get("content")
                    .and_then(|content| content.get("value"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow!("prompt_messages entry content.value must be a string")
                    })?;
                selectors.extend(parse_template_selector_tokens(content));
            }

            Ok(selectors)
        }
        "data_model_query" => extract_data_model_query_selector_paths(raw_value),
        "condition_group" => extract_condition_group_selector_paths(raw_value),
        "if_else_branches" => extract_if_else_branch_selector_paths(raw_value),
        "state_write" => {
            let entries = raw_value
                .as_array()
                .ok_or_else(|| anyhow!("state_write value must be an array"))?;
            let mut selectors = Vec::new();

            for entry in entries {
                if let Some(source) = entry.get("source").filter(|value| value.is_array()) {
                    selectors.push(selector_path(source)?);
                }

                let Some(value) = entry.get("value").and_then(Value::as_object) else {
                    continue;
                };

                match value.get("kind").and_then(Value::as_str) {
                    Some("constant") => {}
                    Some("selector") => {
                        selectors.push(selector_path(
                            value.get("selector").unwrap_or(&Value::Null),
                        )?);
                    }
                    Some("templated_text") => {
                        let template =
                            value.get("value").and_then(Value::as_str).ok_or_else(|| {
                                anyhow!("state_write templated_text value must be a string")
                            })?;
                        selectors.extend(parse_template_selector_tokens(template));
                    }
                    Some(kind) => bail!("state_write value kind is unsupported: {kind}"),
                    None => bail!("state_write value kind is required"),
                }
            }

            Ok(selectors)
        }
        other => bail!("unsupported binding kind: {other}"),
    }
}

fn extract_if_else_branch_selector_paths(raw_value: &Value) -> Result<Vec<Vec<String>>> {
    let branches = raw_value
        .get("branches")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("if_else_branches value must include branches"))?;
    let mut selectors = Vec::new();

    for branch in branches {
        if let Some(condition) = branch.get("condition") {
            selectors.extend(extract_condition_group_selector_paths(condition)?);
        }
    }

    Ok(selectors)
}

fn extract_condition_group_selector_paths(raw_value: &Value) -> Result<Vec<Vec<String>>> {
    let conditions = raw_value
        .get("conditions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("condition_group value must include conditions"))?;
    let mut selectors = Vec::new();

    for condition in conditions {
        if condition
            .get("conditions")
            .and_then(Value::as_array)
            .is_some()
        {
            selectors.extend(extract_condition_group_selector_paths(condition)?);
            continue;
        }

        selectors.push(selector_path(
            condition.get("left").unwrap_or(&Value::Null),
        )?);

        if let Some(right) = condition.get("right") {
            push_condition_value_selector(right, &mut selectors)?;
        }
    }

    Ok(selectors)
}

fn push_condition_value_selector(value: &Value, selectors: &mut Vec<Vec<String>>) -> Result<()> {
    if value.is_array() {
        selectors.push(selector_path(value)?);
        return Ok(());
    }

    if value.get("kind").and_then(Value::as_str) == Some("selector") {
        selectors.push(selector_path(
            value.get("selector").unwrap_or(&Value::Null),
        )?);
    }

    Ok(())
}

fn extract_data_model_query_selector_paths(raw_value: &Value) -> Result<Vec<Vec<String>>> {
    let object = raw_value
        .as_object()
        .ok_or_else(|| anyhow!("data_model_query value must be an object"))?;
    let mut selectors = Vec::new();

    if let Some(filters) = object.get("filters") {
        for filter in filters
            .as_array()
            .ok_or_else(|| anyhow!("data_model_query filters must be an array"))?
        {
            if let Some(value) = filter.get("value") {
                push_query_value_selector(value, &mut selectors)?;
            }
        }
    }

    if let Some(page) = object.get("page") {
        push_query_value_selector(page, &mut selectors)?;
    }
    if let Some(page_size) = object.get("page_size") {
        push_query_value_selector(page_size, &mut selectors)?;
    }

    Ok(selectors)
}

fn push_query_value_selector(value: &Value, selectors: &mut Vec<Vec<String>>) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("data_model_query value input must be an object"))?;

    if object.get("kind").and_then(Value::as_str) == Some("selector") {
        selectors.push(selector_path(
            object.get("selector").unwrap_or(&Value::Null),
        )?);
    }

    Ok(())
}

fn selector_path(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or_else(|| anyhow!("selector path must be an array"))?
        .iter()
        .map(|segment| {
            segment
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("selector path segment must be a string"))
        })
        .collect()
}

fn selector_path_list(value: &Value) -> Result<Vec<Vec<String>>> {
    value
        .as_array()
        .ok_or_else(|| anyhow!("selector path list must be an array"))?
        .iter()
        .map(selector_path)
        .collect()
}

fn parse_template_selector_tokens(value: &str) -> Vec<Vec<String>> {
    let mut selectors = Vec::new();
    let mut cursor = 0;

    while let Some(start_offset) = value[cursor..].find("{{") {
        let start = cursor + start_offset + 2;
        let Some(end_offset) = value[start..].find("}}") else {
            break;
        };
        let end = start + end_offset;
        let token = value[start..end].trim();

        let selector = token
            .split('.')
            .map(str::trim)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        if selector.len() >= 2 && selector.iter().all(|segment| !segment.is_empty()) {
            selectors.push(selector);
        }

        cursor = end + 2;
    }

    selectors
}
