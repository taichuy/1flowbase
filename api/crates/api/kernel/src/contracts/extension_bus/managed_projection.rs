//! Explicit, read-only views for the frozen canonical interface inventory.
//! Field selection stays in typed owners. These primitives cannot serialize an arbitrary DTO.
use serde_json::{json, Value};

pub(crate) const MAX_TEXT_BYTES: usize = 256;

pub(crate) fn text(value: &str) -> Option<Value> {
    (value.len() <= MAX_TEXT_BYTES).then(|| Value::String(value.to_owned()))
}

pub(crate) fn text_schema() -> Value {
    json!({"type": "string", "maxLength": MAX_TEXT_BYTES})
}

pub(crate) fn tag_schema(tag: &str) -> Value {
    json!({"type": "string", "maxLength": MAX_TEXT_BYTES, "const": tag})
}

pub(crate) fn count_schema() -> Value {
    json!({"type": "integer", "minimum": 0})
}

pub(crate) fn object_schema<'a>(fields: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    let fields = fields.into_iter();
    let field_count = fields.size_hint().0;
    let mut properties = serde_json::Map::with_capacity(field_count);
    let mut required = Vec::with_capacity(field_count);
    for (name, schema) in fields {
        properties.insert(name.to_owned(), schema);
        required.push(Value::String(name.to_owned()));
    }
    object_value([
        ("type", Value::String("object".to_owned())),
        ("properties", Value::Object(properties)),
        ("required", Value::Array(required)),
        ("additionalProperties", Value::Bool(false)),
    ])
}

pub(crate) fn json_summary_schema() -> Value {
    object_schema([
        (
            "kind",
            json!({"type": "string", "maxLength": 7,
            "enum": ["null", "boolean", "number", "string", "array", "object"]}),
        ),
        ("item_count", count_schema()),
        ("byte_count", count_schema()),
    ])
}

/// O(1) metadata only: never visits keys, nested values, defaults, examples, or error text.
pub(crate) fn json_summary(value: &Value) -> Value {
    let (kind, count, bytes) = match value {
        Value::Null => ("null", 0, 0),
        Value::Bool(_) => ("boolean", 0, 0),
        Value::Number(_) => ("number", 0, 0),
        Value::String(value) => ("string", 0, value.len()),
        Value::Array(value) => ("array", value.len(), 0),
        Value::Object(value) => ("object", value.len(), 0),
    };
    json!({"kind": kind, "item_count": count, "byte_count": bytes})
}

pub(crate) fn object_value<'a>(fields: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

pub(crate) fn union_schema(variants: Vec<Value>) -> Value {
    object_value([("oneOf", Value::Array(variants))])
}
