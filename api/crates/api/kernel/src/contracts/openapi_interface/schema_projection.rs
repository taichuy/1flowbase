//! Shared schema and JSON view mechanics; field-selection policy belongs to each API owner.
use serde_json::Value;

pub fn openapi_schema_to_value(
    schema: utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
) -> serde_json::Result<Value> {
    serde_json::to_value(schema)
}

pub fn openapi_schema_from_value(
    value: Value,
) -> serde_json::Result<utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>> {
    serde_json::from_value(value)
}
pub fn query_fields(value: Value) -> Value {
    super::json_contract::omit_query_nulls(value).expect("typed query object")
}
pub fn without_fields(mut schema: Value, fields: &[&str]) -> Value {
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for field in fields {
            properties.remove(*field);
        }
    }
    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|value| !fields.iter().any(|field| value.as_str() == Some(field)));
    }
    schema
}
pub fn body_schema(body: Value) -> Value {
    serde_json::json!({"type":"object","properties":{"body":body},"required":["body"],"additionalProperties":false})
}
pub fn path_body_schema(path: Value, body: Value) -> Value {
    serde_json::json!({"type":"object","properties":{"path":path,"body":body},"required":["path","body"],"additionalProperties":false})
}

/// Remove only fields explicitly selected by a family owner. HTTP DTOs stay unchanged.
pub fn without_values(mut value: Value, fields: &[&str]) -> Value {
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            object.remove(*field);
        }
    }
    value
}
pub fn replace_property(mut schema: Value, field: &str, replacement: Value) -> Value {
    schema["properties"][field] = replacement;
    schema
}
pub fn array_schema(items: Value) -> Value {
    serde_json::json!({"type":"array","items":items})
}
