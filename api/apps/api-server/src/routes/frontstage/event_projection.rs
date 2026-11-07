//! Frontstage HTTP document codecs keep tuple positions and transport state private.
use serde_json::{json, Value};

pub(super) fn request(path: Value, mut query: Value, body: Option<Value>) -> Value {
    let mut value = serde_json::Map::new();
    if path.as_object().is_some_and(|fields| !fields.is_empty()) {
        value.insert("path".into(), path);
    }
    if let Some(fields) = query.as_object_mut() {
        fields.retain(|_, value| !value.is_null());
        if !fields.is_empty() {
            value.insert("query".into(), query);
        }
    }
    if let Some(body) = body {
        value.insert("body".into(), body);
    }
    Value::Object(value)
}

/// The outer callable dispatcher cannot project arbitrary downstream credentials.
/// Its downstream typed interface remains the business argument/result owner.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CallableTargetBody {
    pub block_id: String,
    pub method: String,
    pub path: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CallableInput {
    pub path: CallablePath,
    pub body: CallableTargetBody,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CallablePath {
    pub page_id: String,
    pub tab_id: String,
}

/// This is the actual outer HTTP response status, independent of the Kernel's
/// invocation terminal and the downstream target's business result.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CallableOutput {
    pub status: u16,
}

pub(super) fn empty() -> Value {
    json!({})
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/document_event_projection.rs"]
mod tests;
