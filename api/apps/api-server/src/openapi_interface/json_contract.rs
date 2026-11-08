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
pub(crate) trait JsonApiInput: InterfaceContract + Serialize {
    const LOCATION: JsonInputLocation = JsonInputLocation::Envelope;
}
#[derive(Clone, Copy)]
pub(crate) enum JsonInputLocation {
    Envelope,
    Body,
    Query,
    Path,
}
/// The unwrapped business JSON; HTTP status/envelope and MCP result mapping stay outside.
pub(crate) trait JsonApiOutput: InterfaceContract + Serialize {}

pub(crate) fn request<T: JsonApiInput>(input: &T, route: &str) -> Result<Value, Arc<str>> {
    let value = serde_json::to_value(input)
        .map_err(|_| Arc::<str>::from("api-json-input-serialization"))?;
    let value = match T::LOCATION {
        JsonInputLocation::Envelope => value,
        JsonInputLocation::Body => serde_json::json!({"body": value}),
        JsonInputLocation::Query => serde_json::json!({"query": value}),
        JsonInputLocation::Path => serde_json::json!({"path": value}),
    };
    let mut fields = match value {
        // A serde unit variant declares an empty request, not a null JSON body.
        Value::Null => Map::new(),
        Value::Object(fields) => fields,
        _ => return Err(Arc::from("api-json-input-envelope")),
    };
    let names = route
        .split('/')
        .filter_map(|segment| {
            segment.strip_prefix(':').or_else(|| {
                segment
                    .strip_prefix('{')
                    .and_then(|name| name.strip_suffix('}'))
            })
        })
        .collect::<Vec<_>>();
    let mut path = Map::new();
    for name in &names {
        if let Some(value) = fields.remove(*name) {
            path.insert((*name).to_owned(), value);
        }
    }
    if !path.is_empty() {
        if fields.insert("path".into(), Value::Object(path)).is_some() {
            return Err(Arc::from("api-json-input-path-ambiguous"));
        }
    }
    if let Some(value) = fields.remove("path") {
        let value = match value {
            Value::Object(value) => Value::Object(value),
            Value::Array(values) if values.len() == names.len() => Value::Object(
                names
                    .iter()
                    .zip(values)
                    .map(|(name, value)| ((*name).into(), value))
                    .collect(),
            ),
            value if names.len() == 1 && !value.is_array() => {
                Value::Object([(names[0].to_owned(), value)].into_iter().collect())
            }
            _ => return Err(Arc::from("api-json-input-path-arity")),
        };
        fields.insert("path".into(), value);
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
/// Remove only top-level absent query fields; never recursively rewrite body or nested JSON.
pub(super) fn omit_query_nulls(mut value: Value) -> Result<Value, &'static str> {
    let fields = value.as_object_mut().ok_or("api-json-query-object")?;
    fields.retain(|_, value| !value.is_null());
    Ok(value)
}
pub(crate) fn query_fields<T: Serialize + ?Sized, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let value = serde_json::to_value(value).map_err(serde::ser::Error::custom)?;
    omit_query_nulls(value)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}
pub(crate) fn query<T: Serialize + ?Sized, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Query<'a, T: Serialize + ?Sized> {
        #[serde(serialize_with = "query_fields")]
        query: &'a T,
    }
    Query { query: value }.serialize(serializer)
}
/// Some existing contracts deliberately retain nulls; this is an explicit owner choice.
pub(crate) fn query_with_nulls<T: Serialize + ?Sized, S: Serializer>(
    query: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Query<'a, T: Serialize + ?Sized> {
        query: &'a T,
    }
    Query { query }.serialize(serializer)
}
pub(crate) fn path<T: Serialize + ?Sized, S: Serializer>(
    path: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Path<'a, T: Serialize + ?Sized> {
        path: &'a T,
    }
    Path { path }.serialize(serializer)
}
pub(crate) fn path2<A: Serialize, B: Serialize, S: Serializer>(
    a: &A,
    b: &B,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    path(&(a, b), serializer)
}
pub(crate) fn path_body<P: Serialize, B: Serialize, S: Serializer>(
    path: &P,
    body: &B,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Parts<'a, P, B> {
        path: &'a P,
        body: &'a B,
    }
    Parts { path, body }.serialize(serializer)
}
pub(crate) fn path_query<P: Serialize, Q: Serialize, S: Serializer>(
    path: &P,
    query: &Q,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Parts<'a, P, Q> {
        path: &'a P,
        query: &'a Q,
    }
    Parts { path, query }.serialize(serializer)
}
pub(crate) fn path_query_omit_nulls<P: Serialize, Q: Serialize, S: Serializer>(
    path: &P,
    query: &Q,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Parts<'a, P, Q: Serialize> {
        path: &'a P,
        #[serde(serialize_with = "query_fields")]
        query: &'a Q,
    }
    Parts { path, query }.serialize(serializer)
}
pub(crate) fn path2_query_omit_nulls<A: Serialize, B: Serialize, Q: Serialize, S: Serializer>(
    a: &A,
    b: &B,
    query: &Q,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    path_query_omit_nulls(&(a, b), query, serializer)
}
pub(crate) fn query_body<Q: Serialize, B: Serialize, S: Serializer>(
    query: &Q,
    body: &B,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Parts<'a, Q, B> {
        query: &'a Q,
        body: &'a B,
    }
    Parts { query, body }.serialize(serializer)
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/json_contract.rs"]
mod tests;
