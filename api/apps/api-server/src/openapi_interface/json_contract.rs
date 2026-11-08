//! Explicit ordinary-JSON contracts shared by protocol results and typed observation.
//! Authentication context, credentials and non-JSON carriers require their owner's own policy.
use crate::{error_response::ApiError, response::ApiSuccess};
use axum::Json;
use interface_runtime::InterfaceContract;
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};
use std::sync::Arc;

/// Opt-in means this type serializes only its public business request fields.
/// It does not make every serializable InterfaceContract observable.
pub(crate) trait JsonApiInput: InterfaceContract + Serialize {}
/// The unwrapped business JSON; HTTP status/envelope and MCP result mapping stay outside.
pub(crate) trait JsonApiOutput: InterfaceContract + Serialize {}

pub(crate) fn request<T: JsonApiInput>(input: &T, route: &str) -> Result<Value, Arc<str>> {
    let value = serde_json::to_value(input)
        .map_err(|_| Arc::<str>::from("api-json-input-serialization"))?;
    let mut fields = match value {
        // A serde unit variant declares an empty request, not a null JSON body.
        Value::Null => Map::new(),
        Value::Object(fields) => fields,
        _ => return Err(Arc::from("api-json-input-envelope")),
    };
    let mut path = Map::new();
    for segment in route.split('/') {
        let name = segment.strip_prefix(':').or_else(|| {
            segment
                .strip_prefix('{')
                .and_then(|name| name.strip_suffix('}'))
        });
        if let Some(name) = name {
            if let Some(value) = fields.remove(name) {
                path.insert(name.to_owned(), value);
            }
        }
    }
    if !path.is_empty() {
        if fields.insert("path".into(), Value::Object(path)).is_some() {
            return Err(Arc::from("api-json-input-path-ambiguous"));
        }
    }
    // Opaque host context must be skipped by the explicit contract, never implicitly copied.
    if fields
        .keys()
        .any(|key| !matches!(key.as_str(), "path" | "query" | "body"))
    {
        return Err(Arc::from("api-json-input-envelope-fields"));
    }
    Ok(Value::Object(fields))
}

pub(crate) fn result<T: JsonApiOutput>(output: &T) -> serde_json::Result<Value> {
    serde_json::to_value(output)
}
pub(crate) fn response<T: JsonApiOutput>(output: &T) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    Ok(Json(ApiSuccess::new(result(output)?)))
}

/// Serde variant annotation: borrow the actual DTO rather than repeating its field inventory.
pub(crate) fn body<T: Serialize + ?Sized, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Body<'a, T: Serialize + ?Sized> {
        body: &'a T,
    }
    Body { body: value }.serialize(serializer)
}
/// Query nulls denote omitted optional parameters; body nulls remain unchanged.
pub(crate) fn query<T: Serialize + ?Sized, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut value = serde_json::to_value(value).map_err(serde::ser::Error::custom)?;
    let fields = value
        .as_object_mut()
        .ok_or_else(|| serde::ser::Error::custom("api-json-query-object"))?;
    fields.retain(|_, value| !value.is_null());
    #[derive(Serialize)]
    struct Query {
        query: Value,
    }
    Query { query: value }.serialize(serializer)
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/json_contract.rs"]
mod tests;
