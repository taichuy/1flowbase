use super::input_defaults;

pub fn mcp_call_parameter_is_open(input_mapping: &serde_json::Value, name: &str) -> bool {
    let configured = input_mapping
        .get("interface_parameters")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|parameters| {
            parameters.iter().any(|parameter| {
                parameter
                    .pointer("/source/kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("mcp_call")
            })
        });
    if !configured {
        return input_mapping
            .get("call_parameters")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|parameters| {
                parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == Some(name))
            });
    }
    input_mapping
        .get("mappings")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|mappings| {
            mappings.iter().any(|mapping| {
                mapping
                    .get("interface_param")
                    .and_then(serde_json::Value::as_str)
                    == Some(name)
                    && !input_defaults::mapping_is_hidden(mapping)
                    && mapping.get("mcp_param").and_then(serde_json::Value::as_str) == Some(name)
                    && mapping
                        .pointer("/source/kind")
                        .and_then(serde_json::Value::as_str)
                        == Some("mcp_call")
                    && mapping
                        .pointer("/source/path")
                        .and_then(serde_json::Value::as_str)
                        == Some(name)
            })
        })
}

pub fn validate_mcp_call_parameters(input_mapping: &serde_json::Value) -> Result<(), &'static str> {
    input_defaults::validate_input_defaults(input_mapping, &[])?;
    let parameters = input_mapping
        .get("interface_parameters")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|parameter| {
            parameter
                .pointer("/source/kind")
                .and_then(serde_json::Value::as_str)
                == Some("mcp_call")
        })
        .collect::<Vec<_>>();
    if !parameters.is_empty() && parameters.len() != 3 {
        return Err("input_mapping");
    }
    let mut controls = std::collections::HashSet::new();
    for parameter in parameters {
        let name = parameter.get("name").and_then(serde_json::Value::as_str);
        if !matches!(
            name,
            Some("des_id" | "max_inline_chars" | "response_fields")
        ) || parameter
            .get("required")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
            && !(name == Some("des_id")
                && parameter
                    .get("required")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true))
            || parameter.pointer("/source/tool_id").is_some()
            || !controls.insert(name.unwrap_or_default())
        {
            return Err("input_mapping");
        }
    }
    let mut seen = std::collections::HashSet::new();
    for mapping in input_mapping
        .get("mappings")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if mapping.pointer("/source/tool_id").is_some()
            && mapping
                .get("interface_param")
                .and_then(serde_json::Value::as_str)
                != Some("des_id")
        {
            return Err("input_mapping");
        }
        let Some(name) = mapping
            .get("interface_param")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if mapping.pointer("/source/tool_id").is_some()
            && (name != "des_id"
                || mapping
                    .pointer("/source/tool_id")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|id| id.trim().is_empty())
                || mapping.get("required").and_then(serde_json::Value::as_bool) != Some(true))
        {
            return Err("input_mapping");
        }
        if name == "des_id"
            && mapping.get("required").and_then(serde_json::Value::as_bool) == Some(true)
            && (input_defaults::mapping_is_hidden(mapping)
                || input_defaults::mapping_default(mapping).is_some())
        {
            return Err("input_mapping");
        }
        let source_kind = mapping
            .pointer("/source/kind")
            .and_then(serde_json::Value::as_str);
        if mapping.pointer("/source/tool_id").is_some() && source_kind != Some("mcp_call") {
            return Err("input_mapping");
        }
        if source_kind != Some("mcp_call") && !controls.contains(name) {
            continue;
        }
        if source_kind != Some("mcp_call")
            || !controls.contains(name)
            || mapping.get("mcp_param").and_then(serde_json::Value::as_str) != Some(name)
            || mapping
                .pointer("/source/path")
                .and_then(serde_json::Value::as_str)
                != Some(name)
            || !matches!(
                mapping.get("required").and_then(serde_json::Value::as_bool),
                Some(false)
            ) && !(name == "des_id"
                && mapping.get("required").and_then(serde_json::Value::as_bool) == Some(true))
            || !seen.insert(name)
        {
            return Err("input_mapping");
        }
    }
    Ok(())
}

/// Description acknowledgement is configured by the call-control mapping, never a persisted flag.
pub fn mcp_des_id_required(input_mapping: &serde_json::Value) -> bool {
    input_mapping
        .get("mappings")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .any(|mapping| {
            mapping
                .get("interface_param")
                .and_then(serde_json::Value::as_str)
                == Some("des_id")
                && mapping
                    .pointer("/source/kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("mcp_call")
                && mapping.get("required").and_then(serde_json::Value::as_bool) == Some(true)
        })
}

pub fn mcp_description_tool_id(input_mapping: &serde_json::Value) -> Option<&str> {
    description_mapping(input_mapping)?
        .pointer("/source/tool_id")?
        .as_str()
}

fn description_mapping(input_mapping: &serde_json::Value) -> Option<&serde_json::Value> {
    input_mapping
        .get("mappings")?
        .as_array()?
        .iter()
        .find(|mapping| {
            mapping
                .get("interface_param")
                .and_then(serde_json::Value::as_str)
                == Some("des_id")
                && mapping
                    .pointer("/source/kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("mcp_call")
        })
}
