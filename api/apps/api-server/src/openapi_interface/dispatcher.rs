use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    http::{
        header::{ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, COOKIE},
        HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode,
    },
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tower::ServiceExt;
use utoipa::ToSchema;

use crate::{app_state::ApiState, openapi_interface::OpenApiInterfaceCatalogEntry};

const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");
const CALLABLE_DEPTH_HEADER: HeaderName = HeaderName::from_static("x-1flowbase-callable-depth");
const MAX_CALLABLE_DEPTH: u8 = 4;

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
    request
        .headers_mut()
        .insert(CALLABLE_DEPTH_HEADER, next_callable_depth(source_headers)?);
    copy_header(source_headers, request.headers_mut(), COOKIE);
    copy_header(source_headers, request.headers_mut(), AUTHORIZATION);
    copy_header(source_headers, request.headers_mut(), ACCEPT_LANGUAGE);
    copy_header(source_headers, request.headers_mut(), CSRF_HEADER);
    for (name, value) in &arguments.headers {
        let name = HeaderName::try_from(name).map_err(|_| invalid("request_schema"))?;
        if name == COOKIE
            || name == AUTHORIZATION
            || name == CSRF_HEADER
            || name == CALLABLE_DEPTH_HEADER
        {
            return Err(invalid("host_injected_parameters").into());
        }
        let value = value
            .as_str()
            .ok_or_else(|| invalid("request_schema"))?
            .parse()
            .map_err(|_| invalid("request_schema"))?;
        request.headers_mut().insert(name, value);
    }
    if has_body && is_json_media_type(entry.request_media_type.as_deref()) {
        request.headers_mut().insert(
            CONTENT_TYPE,
            "application/json"
                .parse()
                .expect("application/json is a valid header value"),
        );
    } else if has_body {
        return Err(invalid("request_media_type").into());
    }

    let response = crate::console_router(state, true)
        .oneshot(request)
        .await
        .map_err(|error| anyhow::anyhow!("failed to dispatch OpenAPI operation: {error}"))?;
    if !response.status().is_success() {
        return Err(DispatchError::Target(response));
    }
    let value = parse_response(response, entry.response_media_type.as_deref()).await?;
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

async fn parse_response(
    response: Response,
    response_media_type: Option<&str>,
) -> anyhow::Result<Value> {
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| anyhow::anyhow!("failed to read OpenAPI operation response: {error}"))?;
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    if !is_json_media_type(response_media_type) {
        return Err(invalid("response_media_type"));
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("response_schema"))
}

fn is_json_media_type(media_type: Option<&str>) -> bool {
    media_type.is_some_and(|media_type| {
        media_type.eq_ignore_ascii_case("application/json") || media_type.ends_with("+json")
    })
}

fn next_callable_depth(source_headers: &HeaderMap) -> anyhow::Result<HeaderValue> {
    let depth = source_headers
        .get(&CALLABLE_DEPTH_HEADER)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| invalid("callable_dispatch_depth"))?
                .parse::<u8>()
                .map_err(|_| invalid("callable_dispatch_depth"))
        })
        .transpose()?
        .unwrap_or(0);
    if depth >= MAX_CALLABLE_DEPTH {
        return Err(invalid("callable_dispatch_depth"));
    }
    HeaderValue::from_str(&(depth + 1).to_string()).map_err(|_| invalid("callable_dispatch_depth"))
}

fn copy_header(source: &HeaderMap, target: &mut HeaderMap, name: HeaderName) {
    if let Some(value) = source.get(&name) {
        target.insert(name, value.clone());
    }
}

fn validate(schema: &Value, value: &Value, field: &'static str) -> anyhow::Result<()> {
    let validator = jsonschema::validator_for(schema).map_err(|_| invalid(field))?;
    validator.validate(value).map_err(|_| invalid(field))
}

fn invalid(field: &'static str) -> anyhow::Error {
    control_plane::errors::ControlPlaneError::InvalidInput(field).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_complete_json_schema_constraints() {
        let schema = json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": { "enum": ["ready", "done"] },
                "count": { "type": "integer", "minimum": 1 }
            },
            "additionalProperties": false
        });

        assert!(validate(
            &schema,
            &json!({ "status": "ready", "count": 1 }),
            "request_schema"
        )
        .is_ok());
        assert!(validate(
            &schema,
            &json!({ "status": "invalid", "count": 0 }),
            "request_schema"
        )
        .is_err());
    }

    #[test]
    fn callable_depth_is_internal_bounded_state() {
        let mut headers = HeaderMap::new();
        assert_eq!(next_callable_depth(&headers).unwrap(), "1");
        headers.insert(CALLABLE_DEPTH_HEADER, HeaderValue::from_static("3"));
        assert_eq!(next_callable_depth(&headers).unwrap(), "4");
        headers.insert(CALLABLE_DEPTH_HEADER, HeaderValue::from_static("4"));
        assert!(next_callable_depth(&headers).is_err());
    }
}
