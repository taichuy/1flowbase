use anyhow::{ensure, Result};
use base64::Engine;
use opentelemetry_proto::tonic::common::v1::{any_value, AnyValue, KeyValue};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(super) type Attributes = BTreeMap<String, Value>;

pub(super) fn attributes(items: &[KeyValue]) -> Result<Attributes> {
    let mut output = Attributes::new();
    for item in items {
        ensure!(
            !output.contains_key(&item.key),
            "agent_logs.otlp_duplicate_attribute"
        );
        output.insert(item.key.clone(), any_value(item.value.as_ref()));
    }
    Ok(output)
}

fn any_value(value: Option<&AnyValue>) -> Value {
    match value.and_then(|value| value.value.as_ref()) {
        Some(any_value::Value::StringValue(value)) => Value::String(value.clone()),
        Some(any_value::Value::BoolValue(value)) => Value::Bool(*value),
        Some(any_value::Value::IntValue(value)) => Value::from(*value),
        Some(any_value::Value::DoubleValue(value)) => Value::from(*value),
        Some(any_value::Value::ArrayValue(value)) => Value::Array(
            value
                .values
                .iter()
                .map(|value| any_value(Some(value)))
                .collect(),
        ),
        Some(any_value::Value::KvlistValue(value)) => Value::Object(
            value
                .values
                .iter()
                .map(|value| (value.key.clone(), any_value(value.value.as_ref())))
                .collect::<Map<_, _>>(),
        ),
        Some(any_value::Value::BytesValue(value)) => {
            Value::String(base64::engine::general_purpose::STANDARD.encode(value))
        }
        // String-table references are profiling-only and have no Trace semantics.
        Some(any_value::Value::StringValueStrindex(_)) | None => Value::Null,
    }
}

pub(super) fn text(attrs: &Attributes, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        attrs
            .get(*key)
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(str::to_owned)
    })
}

pub(super) fn integer(attrs: &Attributes, keys: &[&str]) -> Result<Option<i64>> {
    let Some(value) = keys.iter().find_map(|key| attrs.get(*key)) else {
        return Ok(None);
    };
    let value = match value {
        Value::String(value) => value.parse::<i64>()?,
        _ => value
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("agent_logs.otlp_usage_integer"))?,
    };
    ensure!(value >= 0, "agent_logs.negative_usage");
    Ok(Some(value))
}

pub(super) fn structured(value: &Value) -> Value {
    match value {
        // SDKs also put plain text (including truncated JSON) in these attributes.
        // Keep it as source text when it is not a complete JSON blob. The self
        // metadata namespace has its own strict parser and cannot use this fallback.
        Value::String(text) => serde_json::from_str(text).unwrap_or_else(|_| value.clone()),
        value => value.clone(),
    }
}

pub(super) fn content(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    match value {
        Value::String(value) => Some(value.clone()),
        _ => Some(value.to_string()),
    }
}

pub(super) fn string_attribute(key: &str, value: impl Into<String>) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(any_value::Value::StringValue(value.into())),
        }),
        ..Default::default()
    }
}

pub(super) fn int_attribute(key: &str, value: i64) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(any_value::Value::IntValue(value)),
        }),
        ..Default::default()
    }
}
