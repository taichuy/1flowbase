//! PostgreSQL JSONB query projections with a separate, lossless row sidecar.
//!
//! The parameter envelope is adapter-owned and is unpacked by each SQL statement;
//! it is never inspected inside a user value or stored as a business payload.
use serde_json::{json, Value};

pub(super) trait JsonParameter {
    fn json_value(&self) -> Option<&Value>;
}

impl JsonParameter for Value {
    fn json_value(&self) -> Option<&Value> {
        Some(self)
    }
}
impl<T: JsonParameter> JsonParameter for Option<T> {
    fn json_value(&self) -> Option<&Value> {
        self.as_ref().and_then(JsonParameter::json_value)
    }
}
impl<T: JsonParameter + ?Sized> JsonParameter for &T {
    fn json_value(&self) -> Option<&Value> {
        (*self).json_value()
    }
}

pub(super) fn lossless_json_parameter(value: &impl JsonParameter) -> Option<Value> {
    value.json_value().map(|original| {
        let mut projection = original.clone();
        let changed = project(&mut projection);
        let raw = changed.then(|| serde_json::to_string(original).expect("JSON Value serializes"));
        json!([projection, raw])
    })
}

pub(super) fn lossless_json_columns(field: &str, value: &Value) -> (Value, Value) {
    let packet = lossless_json_parameter(value).expect("required JSON value");
    let mut originals = serde_json::Map::new();
    if !packet[1].is_null() {
        originals.insert(field.to_owned(), packet[1].clone());
    }
    let originals = Value::Object(originals);
    (packet[0].clone(), originals)
}

fn project(value: &mut Value) -> bool {
    match value {
        Value::String(text) => {
            let changed = text.contains('\0');
            if changed {
                *text = text.replace('\0', "\\u0000");
            }
            changed
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |changed, item| project(item) | changed),
        Value::Object(object) => {
            let mut changed = false;
            let entries = std::mem::take(object);
            let encode_keys = entries.keys().any(|key| key.contains('\0'));
            for (key, mut value) in entries {
                changed |= project(&mut value);
                changed |= key.contains('\0');
                let key = if encode_keys {
                    key.replace('\\', "\\\\").replace('\0', "\\u0000")
                } else {
                    key
                };
                object.insert(key, value);
            }
            changed
        }
        _ => false,
    }
}

pub(super) trait TextParameter {
    fn text_value(&self) -> Option<&str>;
}
impl TextParameter for str {
    fn text_value(&self) -> Option<&str> {
        Some(self)
    }
}
impl TextParameter for String {
    fn text_value(&self) -> Option<&str> {
        Some(self)
    }
}
impl<T: AsRef<str>> TextParameter for Option<T> {
    fn text_value(&self) -> Option<&str> {
        self.as_ref().map(AsRef::as_ref)
    }
}
impl<T: TextParameter + ?Sized> TextParameter for &T {
    fn text_value(&self) -> Option<&str> {
        (*self).text_value()
    }
}
pub(super) fn lossless_text_parameter(value: &(impl TextParameter + ?Sized)) -> Option<Value> {
    value
        .text_value()
        .and_then(|text| lossless_json_parameter(&Value::String(text.to_owned())))
}

// Optional extra SELECT columns carry serialized original text. Keep the existing
// TEXT columns for filters/order/search, and decode only at the Rust row boundary.
pub(super) fn original_optional_text(
    row: &sqlx::postgres::PgRow,
    field: &str,
) -> anyhow::Result<Option<String>> {
    use sqlx::Row;
    match row.try_get::<Option<String>, _>(format!("{field}_original").as_str()) {
        Ok(Some(raw)) => Ok(Some(serde_json::from_str(&raw)?)),
        Ok(None) | Err(sqlx::Error::ColumnNotFound(_)) => Ok(row.try_get(field)?),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn original_required_text(
    row: &sqlx::postgres::PgRow,
    field: &str,
) -> anyhow::Result<String> {
    original_optional_text(row, field)?
        .ok_or_else(|| anyhow::anyhow!("required original text column is NULL: {field}"))
}

#[cfg(test)]
#[path = "_tests/codec.rs"]
mod tests;
