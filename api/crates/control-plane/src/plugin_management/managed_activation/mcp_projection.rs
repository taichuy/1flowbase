//! Deterministic managed operation projections shared by discovery and durable templates.
use serde_json::{json, Value};

// Managed MCP arguments share DTO field names, without HTTP transport containers.
pub fn parameter_schema(schema: &Value) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for location in ["path", "query", "body"] {
        let Some(location_schema) = schema.pointer(&format!("/properties/{location}")) else {
            continue;
        };
        let Some(fields) = location_schema.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, field_schema) in fields {
            // Intake rejects duplicate field names across locations.
            properties.insert(name.clone(), field_schema.clone());
            if location == "path"
                || location_schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|value| value.as_str() == Some(name)))
            {
                required.push(Value::String(name.clone()));
            }
        }
    }
    json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false})
}

pub fn input_mapping(schema: &Value) -> Value {
    let mut mappings = Vec::new();
    let mut parameters = Vec::new();
    for location in ["path", "query", "body"] {
        let Some(location_schema) = schema.pointer(&format!("/properties/{location}")) else {
            continue;
        };
        let Some(properties) = location_schema.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, property) in properties {
            let required = location == "path"
                || location_schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(name)));
            let path = name;
            let mut mapping = json!({"interface_param":name,"mcp_param":path,"source":{"kind":"mcp_argument","path":path},"required":required,"description":property.get("description").and_then(Value::as_str).unwrap_or("")});
            if let Some(default) = property.get("default") {
                mapping["default_value"] = default.clone();
            }
            parameters.push(json!({
                "name": name,
                "field_type": property.get("type").and_then(Value::as_str).unwrap_or("object"),
                "parameter_type": if location == "body" { "json_body" } else { "url" },
                "description": property.get("description").and_then(Value::as_str),
                "required": required,
                "schema": property,
            }));
            mappings.push(mapping);
        }
    }
    json!({"interface_parameters":parameters,"mappings":mappings})
}

// Project only a real declared metadata subschema; unresolved schemas remain unconstrained.
pub fn media_result_schema(schema: &Value, mapping: Option<&Value>) -> Value {
    let Some(mapping) = mapping else {
        return schema.clone();
    };
    let Ok(mapping) = plugin_framework::ManagedMcpMediaMapping::parse(mapping) else {
        return json!({});
    };
    let mut current = schema;
    for segment in mapping.structured_content_path.split('/').skip(1) {
        let name = segment.replace("~1", "/").replace("~0", "~");
        let next = if current.get("type").and_then(Value::as_str) == Some("array") {
            name.parse::<usize>()
                .ok()
                .and_then(|_| current.get("items"))
        } else {
            current
                .get("properties")
                .and_then(|properties| properties.get(&name))
        };
        let Some(next) = next else {
            return json!({});
        };
        current = next;
    }
    current.clone()
}
