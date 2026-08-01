use serde_json::{json, Map, Value};

struct ParameterMapping<'a> {
    interface_param: &'a str,
    mcp_param: &'a str,
    description: Option<&'a str>,
    required: bool,
}

pub(super) fn mapped_schema(parameter_schema: &Value, input_mapping: &Value) -> Value {
    let mappings = input_mapping
        .get("mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parameter_mapping)
        .collect::<Vec<_>>();
    if mappings.is_empty() {
        return parameter_schema.clone();
    }

    let mut mapped_schema = object_schema();
    for mapping in mappings {
        let mut field_schema = source_field_schema(
            parameter_schema,
            input_mapping,
            mapping.interface_param,
            mapping.mcp_param,
        );
        apply_description(&mut field_schema, mapping.description);
        let path = mapping
            .mcp_param
            .split('.')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        insert_field_schema(
            &mut mapped_schema,
            path.as_slice(),
            field_schema,
            mapping.required,
        );
    }
    mapped_schema
}

fn parameter_mapping(value: &Value) -> Option<ParameterMapping<'_>> {
    let value = value.as_object()?;
    let interface_param = value.get("interface_param")?.as_str()?.trim();
    let mcp_param = value.get("mcp_param")?.as_str()?.trim();
    if interface_param.is_empty() || mcp_param.is_empty() {
        return None;
    }
    Some(ParameterMapping {
        interface_param,
        mcp_param,
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|description| !description.is_empty()),
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn source_field_schema(
    parameter_schema: &Value,
    input_mapping: &Value,
    interface_param: &str,
    mcp_param: &str,
) -> Value {
    let mut candidates = vec![mcp_param.to_owned()];
    if let Some(location) = mcp_param
        .split('.')
        .next()
        .filter(|location| matches!(*location, "path" | "query" | "body"))
    {
        push_candidate(&mut candidates, format!("{location}.{interface_param}"));
    }
    match interface_parameter_type(input_mapping, interface_param) {
        Some("json_body" | "form") => {
            push_candidate(&mut candidates, format!("body.{interface_param}"));
        }
        Some("url") => {
            push_candidate(&mut candidates, format!("path.{interface_param}"));
            push_candidate(&mut candidates, format!("query.{interface_param}"));
        }
        _ => {}
    }
    for location in ["path", "query", "body"] {
        push_candidate(&mut candidates, format!("{location}.{interface_param}"));
    }
    push_candidate(&mut candidates, interface_param.to_owned());

    candidates
        .iter()
        .find_map(|candidate| schema_at_path(parameter_schema, candidate))
        .cloned()
        .unwrap_or_else(|| fallback_field_schema(input_mapping, interface_param))
}

fn push_candidate(candidates: &mut Vec<String>, candidate: String) {
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn interface_parameter_type<'a>(
    input_mapping: &'a Value,
    interface_param: &str,
) -> Option<&'a str> {
    interface_parameter(input_mapping, interface_param)?
        .get("parameter_type")?
        .as_str()
}

fn interface_parameter<'a>(
    input_mapping: &'a Value,
    interface_param: &str,
) -> Option<&'a Map<String, Value>> {
    input_mapping
        .get("interface_parameters")?
        .as_array()?
        .iter()
        .filter_map(Value::as_object)
        .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(interface_param))
}

fn fallback_field_schema(input_mapping: &Value, interface_param: &str) -> Value {
    let Some(field_type) = interface_parameter(input_mapping, interface_param)
        .and_then(|parameter| parameter.get("field_type"))
        .and_then(Value::as_str)
    else {
        return json!({});
    };
    match field_type.to_ascii_lowercase().as_str() {
        "bool" | "boolean" => json!({"type":"boolean"}),
        "int" | "integer" | "i32" | "i64" | "u32" | "u64" => {
            json!({"type":"integer"})
        }
        "float" | "double" | "number" | "f32" | "f64" => json!({"type":"number"}),
        "array" => json!({"type":"array"}),
        "object" => json!({"type":"object"}),
        _ => json!({"type":"string"}),
    }
}

fn schema_at_path<'a>(schema: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = schema;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        current = current.get("properties")?.get(segment)?;
    }
    Some(current)
}

fn apply_description(schema: &mut Value, description: Option<&str>) {
    let Some(description) = description else {
        return;
    };
    match schema {
        Value::Object(schema) => {
            schema.insert("description".into(), Value::String(description.into()));
        }
        Value::Bool(allowed) => {
            let allowed = *allowed;
            *schema = if allowed {
                json!({"description":description})
            } else {
                json!({"allOf":[false],"description":description})
            };
        }
        _ => {}
    }
}

fn insert_field_schema(schema: &mut Value, path: &[&str], field_schema: Value, required: bool) {
    let Some((segment, remaining)) = path.split_first() else {
        return;
    };
    ensure_object_schema(schema);
    if remaining.is_empty() {
        let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
            return;
        };
        properties.insert((*segment).to_owned(), field_schema);
    } else {
        let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
            return;
        };
        let child = properties
            .entry((*segment).to_owned())
            .or_insert_with(object_schema);
        insert_field_schema(child, remaining, field_schema, required);
    }
    if required {
        add_required_property(schema, segment);
    }
}

fn object_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

fn ensure_object_schema(schema: &mut Value) {
    if !schema.is_object() {
        *schema = object_schema();
        return;
    }
    let Some(schema) = schema.as_object_mut() else {
        return;
    };
    schema
        .entry("type")
        .or_insert_with(|| Value::String("object".into()));
    schema
        .entry("properties")
        .or_insert_with(|| Value::Object(Map::new()));
    schema
        .entry("additionalProperties")
        .or_insert(Value::Bool(false));
}

fn add_required_property(schema: &mut Value, property: &str) {
    let Some(schema) = schema.as_object_mut() else {
        return;
    };
    let Some(required) = schema
        .entry("required")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
    else {
        return;
    };
    if !required
        .iter()
        .any(|existing| existing.as_str() == Some(property))
    {
        required.push(Value::String(property.to_owned()));
    }
}
