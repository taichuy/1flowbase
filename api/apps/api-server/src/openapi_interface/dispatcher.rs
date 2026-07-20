use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    http::{
        header::{ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, COOKIE},
        HeaderMap, HeaderName, Method, Request, StatusCode,
    },
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tower::ServiceExt;
use utoipa::ToSchema;

use crate::{app_state::ApiState, openapi_interface::OpenApiInterfaceCatalogEntry};

const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct DispatchArguments {
    #[serde(default)]
    #[schema(value_type = Object)]
    pub path: Map<String, Value>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub query: Map<String, Value>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub headers: Map<String, Value>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub body: Value,
}

pub struct DispatchSuccess {
    pub value: Value,
}

pub enum DispatchError {
    Api(anyhow::Error),
    Target(Response),
}

impl From<anyhow::Error> for DispatchError {
    fn from(value: anyhow::Error) -> Self {
        Self::Api(value)
    }
}

pub async fn dispatch(
    state: Arc<ApiState>,
    source_headers: &HeaderMap,
    entry: &OpenApiInterfaceCatalogEntry,
    mut arguments: DispatchArguments,
    injected_path: BTreeMap<String, String>,
) -> Result<DispatchSuccess, DispatchError> {
    for (name, value) in injected_path {
        if arguments.path.contains_key(&name) {
            return Err(invalid("host_injected_parameters").into());
        }
        arguments.path.insert(name, Value::String(value));
    }
    validate(
        &entry.request_schema,
        &arguments_value(&arguments),
        "request_schema",
    )?;

    let method =
        Method::from_bytes(entry.method.as_bytes()).map_err(|_| invalid("operation_id"))?;
    let uri = target_uri(&entry.path, &arguments)?;
    let has_body = entry.request_schema.pointer("/properties/body").is_some();
    let body = if has_body {
        Body::from(arguments.body.to_string())
    } else {
        Body::empty()
    };
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .map_err(|_| invalid("request_schema"))?;
    copy_header(source_headers, request.headers_mut(), COOKIE);
    copy_header(source_headers, request.headers_mut(), AUTHORIZATION);
    copy_header(source_headers, request.headers_mut(), ACCEPT_LANGUAGE);
    copy_header(source_headers, request.headers_mut(), CSRF_HEADER);
    for (name, value) in &arguments.headers {
        let name = HeaderName::try_from(name).map_err(|_| invalid("request_schema"))?;
        if name == COOKIE || name == AUTHORIZATION || name == CSRF_HEADER {
            return Err(invalid("host_injected_parameters").into());
        }
        let value = value
            .as_str()
            .ok_or_else(|| invalid("request_schema"))?
            .parse()
            .map_err(|_| invalid("request_schema"))?;
        request.headers_mut().insert(name, value);
    }
    if has_body {
        request.headers_mut().insert(
            CONTENT_TYPE,
            "application/json"
                .parse()
                .expect("application/json is a valid header value"),
        );
    }

    let response = crate::console_router(state, true)
        .oneshot(request)
        .await
        .map_err(|error| anyhow::anyhow!("failed to dispatch OpenAPI operation: {error}"))?;
    if !response.status().is_success() {
        return Err(DispatchError::Target(response));
    }
    let value = parse_response(response).await?;
    let contract_value = value.get("data").unwrap_or(&value);
    validate(&entry.response_schema, contract_value, "response_schema")?;
    Ok(DispatchSuccess { value })
}

fn arguments_value(arguments: &DispatchArguments) -> Value {
    let mut value = Map::new();
    if !arguments.path.is_empty() {
        value.insert("path".into(), Value::Object(arguments.path.clone()));
    }
    if !arguments.query.is_empty() {
        value.insert("query".into(), Value::Object(arguments.query.clone()));
    }
    if !arguments.headers.is_empty() {
        value.insert("headers".into(), Value::Object(arguments.headers.clone()));
    }
    if !arguments.body.is_null() {
        value.insert("body".into(), arguments.body.clone());
    }
    Value::Object(value)
}

fn target_uri(path_template: &str, arguments: &DispatchArguments) -> anyhow::Result<String> {
    let mut path = path_template.to_string();
    for (name, value) in &arguments.path {
        let value = scalar(value, "request_schema")?;
        path = path.replace(
            &format!("{{{name}}}"),
            &form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>(),
        );
    }
    if path.contains('{') || path.contains('}') {
        return Err(invalid("host_injected_parameters"));
    }
    if arguments.query.is_empty() {
        return Ok(path);
    }
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (name, value) in &arguments.query {
        serializer.append_pair(name, &scalar(value, "request_schema")?);
    }
    Ok(format!("{path}?{}", serializer.finish()))
}

fn scalar(value: &Value, field: &'static str) -> anyhow::Result<String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(invalid(field)),
    }
}

async fn parse_response(response: Response) -> anyhow::Result<Value> {
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| anyhow::anyhow!("failed to read OpenAPI operation response: {error}"))?;
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("response_schema"))
}

fn copy_header(source: &HeaderMap, target: &mut HeaderMap, name: HeaderName) {
    if let Some(value) = source.get(&name) {
        target.insert(name, value.clone());
    }
}

fn validate(schema: &Value, value: &Value, field: &'static str) -> anyhow::Result<()> {
    if let Some(types) = schema.get("type") {
        let valid = match types {
            Value::String(kind) => matches_type(kind, value),
            Value::Array(kinds) => kinds
                .iter()
                .filter_map(Value::as_str)
                .any(|kind| matches_type(kind, value)),
            _ => true,
        };
        if !valid {
            return Err(invalid(field));
        }
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        let object = value.as_object().ok_or_else(|| invalid(field))?;
        if required
            .iter()
            .filter_map(Value::as_str)
            .any(|name| !object.contains_key(name))
        {
            return Err(invalid(field));
        }
    }
    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        if schema.get("additionalProperties") == Some(&Value::Bool(false))
            && object.keys().any(|name| !properties.contains_key(name))
        {
            return Err(invalid(field));
        }
        for (name, child_schema) in properties {
            if let Some(child) = object.get(name) {
                validate(child_schema, child, field)?;
            }
        }
    }
    if let (Some(item_schema), Some(items)) = (schema.get("items"), value.as_array()) {
        for item in items {
            validate(item_schema, item, field)?;
        }
    }
    Ok(())
}

fn matches_type(kind: &str, value: &Value) -> bool {
    match kind {
        "null" => value.is_null(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        _ => true,
    }
}

fn invalid(field: &'static str) -> anyhow::Error {
    control_plane::errors::ControlPlaneError::InvalidInput(field).into()
}
