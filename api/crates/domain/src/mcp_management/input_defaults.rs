use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Null means no configured fallback; false, zero and empty containers are values.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct McpInputDefaults {
    #[serde(default)]
    pub default_value: Option<Value>,
    #[serde(default)]
    pub hidden: bool,
}

impl McpInputDefaults {
    pub fn resolve<'a>(&'a self, supplied: Option<&'a Value>) -> Option<&'a Value> {
        if self.hidden {
            self.default_value.as_ref()
        } else {
            supplied.or(self.default_value.as_ref())
        }
    }
}

pub fn mapping_is_hidden(mapping: &Value) -> bool {
    mapping.get("hidden").and_then(Value::as_bool) == Some(true)
}

pub fn mapping_default(mapping: &Value) -> Option<&Value> {
    mapping
        .get("default_value")
        .filter(|value| !value.is_null())
}

/// Normalize the management projection without rewriting historical records.
pub fn input_mapping_with_defaults(mut mapping: Value) -> Value {
    if let Some(entries) = mapping.get_mut("mappings").and_then(Value::as_array_mut) {
        for entry in entries.iter_mut().filter_map(Value::as_object_mut) {
            if entry.contains_key("interface_param") {
                entry.entry("default_value").or_insert(Value::Null);
                entry.entry("hidden").or_insert(Value::Bool(false));
            }
        }
    }
    mapping
}

pub fn validate_input_defaults(
    mapping: &Value,
    parameters: &[super::McpParameterDescriptor],
) -> Result<(), &'static str> {
    for entry in mapping
        .get("mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let defaults: McpInputDefaults =
            serde_json::from_value(entry.clone()).map_err(|_| "input_mapping")?;
        let name = entry
            .get("interface_param")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let parameter = parameters.iter().find(|parameter| parameter.name == name);
        let required = entry.get("required").and_then(Value::as_bool) == Some(true)
            || parameter.is_some_and(|parameter| parameter.required);
        if defaults.hidden && required && defaults.default_value.is_none() {
            return Err("input_mapping");
        }
        let Some(value) = defaults.default_value else {
            continue;
        };
        let is_control = entry.pointer("/source/kind").and_then(Value::as_str) == Some("mcp_call");
        let valid = if is_control {
            match name {
                "des_id" => value.is_string(),
                "max_inline_chars" => value.as_i64().is_some_and(|value| value > 0),
                "response_fields" => value.as_array().is_some_and(|values| {
                    values
                        .iter()
                        .all(|value| value.as_str().is_some_and(super::is_mcp_result_pointer))
                }),
                _ => false,
            }
        } else {
            parameter.is_none_or(|parameter| value_matches_type(&value, &parameter.field_type))
        };
        if !valid {
            return Err("input_mapping");
        }
    }
    Ok(())
}

fn value_matches_type(value: &Value, field_type: &str) -> bool {
    let field_type = field_type.to_ascii_lowercase();
    if let Some(item_type) = field_type
        .strip_prefix("array<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return value
            .as_array()
            .is_some_and(|items| items.iter().all(|item| value_matches_type(item, item_type)));
    }
    match field_type.as_str() {
        "string" => value.is_string(),
        "bool" | "boolean" => value.is_boolean(),
        "int" | "integer" | "i32" | "i64" => value.is_i64() || value.is_u64(),
        "u32" | "u64" => value.is_u64(),
        "number" | "float" | "double" | "f32" | "f64" => value.is_number(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => true,
    }
}

/// Apply only call-control defaults; regular input mappings are resolved by dispatch.
pub fn resolve_call_defaults(arguments: &Value, input_mapping: &Value) -> Value {
    let mut arguments = arguments.clone();
    let Some(object) = arguments.as_object_mut() else {
        return arguments;
    };
    for entry in input_mapping
        .get("mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if entry.pointer("/source/kind").and_then(Value::as_str) != Some("mcp_call") {
            continue;
        }
        let Some(name @ ("des_id" | "max_inline_chars" | "response_fields")) =
            entry.get("interface_param").and_then(Value::as_str)
        else {
            continue;
        };
        if mapping_is_hidden(entry) {
            object.remove(name);
        }
        if !object.contains_key(name) {
            if let Some(value) = mapping_default(entry) {
                object.insert(name.to_owned(), value.clone());
            }
        }
    }
    arguments
}

#[cfg(test)]
#[path = "../_tests/mcp_management/input_defaults.rs"]
mod tests;
