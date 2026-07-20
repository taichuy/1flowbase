use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::openapi_docs::DocsCatalogOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenApiParameterLocation {
    Path,
    Query,
    Header,
    JsonBody,
    FormBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenApiParameterDescriptor {
    pub name: String,
    pub field_type: String,
    pub location: OpenApiParameterLocation,
    pub description: Option<String>,
    pub required: bool,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenApiInterfaceCatalogEntry {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub description: String,
    pub parameter_descriptors: Vec<OpenApiParameterDescriptor>,
    pub request_schema: Value,
    pub response_schema: Value,
    pub security: Value,
}

pub fn catalog_entry_from_operation(
    operation: &DocsCatalogOperation,
    spec: &Value,
) -> Option<OpenApiInterfaceCatalogEntry> {
    let operation_node = operation_node(spec, operation)?;
    let path_item_node = path_item_node(spec, operation)?;
    Some(OpenApiInterfaceCatalogEntry {
        operation_id: operation.id.clone(),
        method: operation.method.clone(),
        path: operation.path.clone(),
        name: operation
            .summary
            .clone()
            .unwrap_or_else(|| operation.id.clone()),
        description: operation
            .description
            .clone()
            .unwrap_or_else(|| format!("{} {}", operation.method, operation.path)),
        parameter_descriptors: parameter_descriptors(spec, path_item_node, operation_node),
        request_schema: input_schema(spec, path_item_node, operation_node),
        response_schema: response_schema(spec, operation_node),
        security: operation_node
            .get("security")
            .or_else(|| spec.get("security"))
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    })
}

fn operation_node<'a>(spec: &'a Value, operation: &DocsCatalogOperation) -> Option<&'a Value> {
    spec.pointer(&format!(
        "/paths/{}/{}",
        escape_pointer(&operation.path),
        operation.method.to_ascii_lowercase()
    ))
}

fn path_item_node<'a>(spec: &'a Value, operation: &DocsCatalogOperation) -> Option<&'a Value> {
    spec.pointer(&format!("/paths/{}", escape_pointer(&operation.path)))
}

fn escape_pointer(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn input_schema(spec: &Value, path_item: &Value, operation: &Value) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for location in ["path", "query", "header"] {
        if let Some(schema) = parameter_location_schema(spec, path_item, operation, location) {
            let request_field = if location == "header" {
                "headers"
            } else {
                location
            };
            let location_required = location == "path"
                || schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty());
            properties.insert(request_field.to_string(), schema);
            if location_required {
                required.push(Value::String(request_field.to_string()));
            }
        }
    }
    if let Some((schema, body_required)) = request_body_schema(spec, operation) {
        properties.insert("body".into(), schema);
        if body_required {
            required.push(Value::String("body".into()));
        }
    }
    object_schema(properties, required)
}

fn parameter_descriptors(
    spec: &Value,
    path_item: &Value,
    operation: &Value,
) -> Vec<OpenApiParameterDescriptor> {
    let mut descriptors = Vec::new();
    for (location, target) in [
        ("path", OpenApiParameterLocation::Path),
        ("query", OpenApiParameterLocation::Query),
        ("header", OpenApiParameterLocation::Header),
    ] {
        for raw in all_parameters(path_item, operation) {
            let parameter = resolve(spec, raw, 0);
            if parameter.get("in").and_then(Value::as_str) != Some(location) {
                continue;
            }
            let Some(name) = parameter.get("name").and_then(Value::as_str) else {
                continue;
            };
            let schema = parameter
                .get("schema")
                .map(|schema| resolve(spec, schema, 0))
                .unwrap_or_else(string_schema);
            descriptors.push(OpenApiParameterDescriptor {
                name: name.to_string(),
                field_type: field_type(&schema),
                location: target,
                description: parameter
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                required: location == "path"
                    || parameter
                        .get("required")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                schema,
            });
        }
    }
    descriptors.extend(body_descriptors(spec, operation));
    descriptors
}

fn all_parameters<'a>(
    path_item: &'a Value,
    operation: &'a Value,
) -> impl Iterator<Item = &'a Value> {
    path_item
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            operation
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
}

fn parameter_location_schema(
    spec: &Value,
    path_item: &Value,
    operation: &Value,
    location: &str,
) -> Option<Value> {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for raw in all_parameters(path_item, operation) {
        let parameter = resolve(spec, raw, 0);
        if parameter.get("in").and_then(Value::as_str) != Some(location) {
            continue;
        }
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };
        let mut schema = parameter
            .get("schema")
            .map(|schema| resolve(spec, schema, 0))
            .unwrap_or_else(string_schema);
        if let (Some(description), Value::Object(schema)) = (
            parameter.get("description").and_then(Value::as_str),
            &mut schema,
        ) {
            schema
                .entry("description")
                .or_insert_with(|| Value::String(description.to_string()));
        }
        properties.insert(name.to_string(), schema);
        if location == "path"
            || parameter
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            required.push(Value::String(name.to_string()));
        }
    }
    (!properties.is_empty()).then(|| object_schema(properties, required))
}

fn request_body_schema(spec: &Value, operation: &Value) -> Option<(Value, bool)> {
    let body = resolve(spec, operation.get("requestBody")?, 0);
    let schema = json_schema(spec, body.get("content")?)?;
    Some((
        schema,
        body.get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    ))
}

fn body_descriptors(spec: &Value, operation: &Value) -> Vec<OpenApiParameterDescriptor> {
    let Some(raw_body) = operation.get("requestBody") else {
        return Vec::new();
    };
    let body = resolve(spec, raw_body, 0);
    let body_required = body
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let Some(content) = body.get("content").and_then(Value::as_object) else {
        return Vec::new();
    };
    let (schema, location) = if let Some(media) = content
        .get("application/x-www-form-urlencoded")
        .or_else(|| content.get("multipart/form-data"))
    {
        (
            resolve(spec, media.get("schema").unwrap_or(&Value::Null), 0),
            OpenApiParameterLocation::FormBody,
        )
    } else if let Some(media) = content.get("application/json").or_else(|| {
        content
            .iter()
            .find(|(kind, _)| kind.ends_with("+json"))
            .map(|(_, media)| media)
    }) {
        (
            resolve(spec, media.get("schema").unwrap_or(&Value::Null), 0),
            OpenApiParameterLocation::JsonBody,
        )
    } else {
        return Vec::new();
    };
    let mut result = Vec::new();
    append_body_descriptors(&mut result, String::new(), &schema, location, body_required);
    result
}

fn append_body_descriptors(
    result: &mut Vec<OpenApiParameterDescriptor>,
    prefix: String,
    schema: &Value,
    location: OpenApiParameterLocation,
    required: bool,
) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        let required_fields = schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        for (name, child) in properties {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            append_body_descriptors(
                result,
                path,
                child,
                location,
                required && required_fields.contains(name.as_str()),
            );
        }
        if !properties.is_empty() {
            return;
        }
    }
    result.push(OpenApiParameterDescriptor {
        name: if prefix.is_empty() {
            "body".into()
        } else {
            prefix
        },
        field_type: field_type(schema),
        location,
        description: schema
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        required,
        schema: schema.clone(),
    });
}

fn response_schema(spec: &Value, operation: &Value) -> Value {
    let Some(responses) = operation.get("responses").and_then(Value::as_object) else {
        return object_schema(Map::new(), Vec::new());
    };
    let mut statuses = responses
        .keys()
        .filter(|status| status.starts_with('2'))
        .collect::<Vec<_>>();
    statuses.sort();
    for status in statuses {
        let response = resolve(spec, &responses[status], 0);
        if let Some(schema) = response
            .get("content")
            .and_then(|value| json_schema(spec, value))
        {
            return schema;
        }
    }
    object_schema(Map::new(), Vec::new())
}

fn json_schema(spec: &Value, content: &Value) -> Option<Value> {
    let content = content.as_object()?;
    let media = content.get("application/json").or_else(|| {
        content
            .iter()
            .find(|(kind, _)| kind.ends_with("+json"))
            .map(|(_, media)| media)
    })?;
    Some(resolve(spec, media.get("schema")?, 0))
}

fn object_schema(properties: Map<String, Value>, required: Vec<Value>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert("properties".into(), Value::Object(properties));
    schema.insert("additionalProperties".into(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert("required".into(), Value::Array(required));
    }
    Value::Object(schema)
}

fn string_schema() -> Value {
    serde_json::json!({ "type": "string" })
}

fn field_type(schema: &Value) -> String {
    schema
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("object")
        .to_string()
}

fn resolve(spec: &Value, value: &Value, depth: usize) -> Value {
    if depth > 16 {
        return value.clone();
    }
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(target) = reference
                    .strip_prefix('#')
                    .and_then(|pointer| spec.pointer(pointer))
                {
                    let mut resolved = resolve(spec, target, depth + 1);
                    if let Value::Object(resolved) = &mut resolved {
                        for (key, sibling) in map {
                            if key != "$ref" {
                                resolved.insert(key.clone(), resolve(spec, sibling, depth + 1));
                            }
                        }
                    }
                    return resolved;
                }
            }
            Value::Object(
                map.iter()
                    .map(|(key, nested)| (key.clone(), resolve(spec, nested, depth + 1)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve(spec, item, depth + 1))
                .collect(),
        ),
        _ => value.clone(),
    }
}
