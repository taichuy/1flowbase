use super::*;

pub(crate) fn validate_identifier(value: &str, field: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

pub(crate) fn validate_path(value: &str) -> Result<()> {
    if !value.starts_with('/') || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(())
}

pub(crate) fn validate_positive(value: i32, field: &'static str) -> Result<()> {
    if value <= 0 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

pub(super) fn validate_list_return_fields(value: &serde_json::Value) -> Result<()> {
    let Some(fields) = value.as_array() else {
        return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
    };
    if fields.is_empty() {
        return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
    }

    let mut seen = BTreeSet::new();
    for field in fields {
        let Some(field) = field.as_str() else {
            return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
        };
        if ![
            "id",
            "type",
            "item_kind",
            "path",
            "name",
            "description_short",
            "children_count",
            "risk_level",
        ]
        .contains(&field)
            || !seen.insert(field)
        {
            return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
        }
    }
    Ok(())
}

pub(super) fn generate_short_id() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    let mut output = String::with_capacity(8);
    for _ in 0..8 {
        let index = (OsRng.next_u32() as usize) % ALPHABET.len();
        output.push(ALPHABET[index] as char);
    }
    output
}

pub(crate) fn normalize_des_id(value: Option<String>) -> String {
    let trimmed = value.unwrap_or_default().trim().to_owned();
    if trimmed.is_empty() {
        generate_short_id()
    } else {
        trimmed
    }
}

pub(crate) fn input_mapping_requires_des_id(input_mapping: &serde_json::Value) -> bool {
    const DES_ID: &str = "des_id";

    let Some(mapping) = input_mapping.as_object() else {
        return false;
    };

    let interface_parameter_required = mapping
        .get("interface_parameters")
        .and_then(serde_json::Value::as_array)
        .and_then(|parameters| {
            parameters.iter().find_map(|parameter| {
                let parameter = parameter.as_object()?;
                (parameter.get("name").and_then(serde_json::Value::as_str) == Some(DES_ID))
                    .then(|| {
                        parameter
                            .get("required")
                            .and_then(serde_json::Value::as_bool)
                    })
                    .flatten()
            })
        });

    mapping
        .get("mappings")
        .and_then(serde_json::Value::as_array)
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let entry = entry.as_object()?;
                let maps_des_id = entry
                    .get("interface_param")
                    .and_then(serde_json::Value::as_str)
                    == Some(DES_ID)
                    || entry.get("mcp_param").and_then(serde_json::Value::as_str) == Some(DES_ID);
                maps_des_id
                    .then(|| {
                        entry
                            .get("required")
                            .and_then(serde_json::Value::as_bool)
                            .or(interface_parameter_required)
                    })
                    .flatten()
            })
        })
        .or(interface_parameter_required)
        .unwrap_or(false)
}

fn path_matches(base_path: &str, candidate: &str) -> bool {
    base_path == "/" || candidate == base_path || candidate.starts_with(&format!("{base_path}/"))
}

pub(super) fn parent_group_path(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return None;
    }
    let separator = path.rfind('/')?;
    if separator == 0 {
        Some("/")
    } else {
        Some(&path[..separator])
    }
}

pub(super) fn list_item_matches_keywords(
    keywords: Option<&[String]>,
    path: &str,
    name: &str,
    description_short: Option<&str>,
) -> bool {
    let searchable = format!(
        "{} {} {}",
        path,
        name,
        description_short.unwrap_or_default()
    )
    .to_lowercase();
    keywords
        .unwrap_or_default()
        .iter()
        .filter(|keyword| !keyword.trim().is_empty())
        .all(|keyword| searchable.contains(&keyword.to_lowercase()))
}

pub(super) fn path_matches_list_query(
    base_path: &str,
    candidate: &str,
    max_depth: i32,
    path_regex_filter: Option<&Regex>,
) -> bool {
    let Some(depth) = list_relative_depth(base_path, candidate) else {
        return false;
    };
    if depth > max_depth {
        return false;
    }
    path_regex_filter
        .map(|path_regex_filter| path_regex_filter.is_match(candidate))
        .unwrap_or(true)
}

fn list_relative_depth(base_path: &str, candidate: &str) -> Option<i32> {
    if !path_matches(base_path, candidate) {
        return None;
    }
    if candidate == base_path {
        return Some(0);
    }
    let relative_path = if base_path == "/" {
        candidate.trim_start_matches('/')
    } else {
        candidate.strip_prefix(base_path)?.trim_start_matches('/')
    };
    Some(
        relative_path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .count() as i32,
    )
}

pub(super) fn compile_list_path_regex(
    pattern: Option<&str>,
    regex_enabled: bool,
    regex_max_length: i32,
) -> Result<Option<Regex>> {
    let Some(pattern) = pattern else {
        return Ok(None);
    };
    if !regex_enabled {
        return Err(ControlPlaneError::InvalidInput("path_regex").into());
    }
    let regex_max_length = usize::try_from(regex_max_length)
        .map_err(|_| ControlPlaneError::InvalidInput("path_regex"))?;
    if pattern.chars().count() > regex_max_length {
        return Err(ControlPlaneError::InvalidInput("path_regex").into());
    }
    Regex::new(pattern)
        .map(Some)
        .map_err(|_| ControlPlaneError::InvalidInput("path_regex").into())
}

pub(super) fn bindable_interface(
    entry: domain::McpInterfaceCatalogEntry,
) -> Result<domain::McpInterfaceCatalogEntry> {
    if !entry.bindable {
        return Err(ControlPlaneError::InvalidInput("interface_id").into());
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::input_mapping_requires_des_id;

    #[test]
    fn input_mapping_des_id_required_is_derived_from_parameter_mapping() {
        assert!(!input_mapping_requires_des_id(&json!({})));

        assert!(input_mapping_requires_des_id(&json!({
            "interface_parameters": [
                {
                    "name": "des_id",
                    "field_type": "string",
                    "parameter_type": "json_body",
                    "description": "des_id",
                    "required": true
                }
            ],
            "mappings": [
                {
                    "interface_param": "des_id",
                    "mcp_param": "des_id",
                    "description": "des_id",
                    "required": true
                }
            ]
        })));

        assert!(!input_mapping_requires_des_id(&json!({
            "interface_parameters": [
                {
                    "name": "des_id",
                    "field_type": "string",
                    "parameter_type": "json_body",
                    "description": "des_id",
                    "required": false
                }
            ],
            "mappings": [
                {
                    "interface_param": "des_id",
                    "mcp_param": "des_id",
                    "description": "des_id",
                    "required": false
                }
            ]
        })));
    }
}
