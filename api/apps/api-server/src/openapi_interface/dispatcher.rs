use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    http::{
        header::{ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, COOKIE},
        HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode,
    },
    response::Response,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tower::ServiceExt;
use utoipa::ToSchema;

use crate::{app_state::ApiState, openapi_interface::OpenApiInterfaceCatalogEntry};

const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");
const CALLABLE_DEPTH_HEADER: HeaderName = HeaderName::from_static("x-1flowbase-callable-depth");
const MAX_CALLABLE_DEPTH: u8 = 4;
const MAX_CALLABLE_BINARY_BYTES: usize = 16 * 1024 * 1024;

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

pub enum DispatchSuccess {
    Json(Value),
    NoContent,
    Media(Response),
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
    validate_request(entry, &arguments)?;

    let method =
        Method::from_bytes(entry.method.as_bytes()).map_err(|_| invalid("operation_id"))?;
    let uri = target_uri(&entry.path, &arguments)?;
    let has_body = entry.request_schema.pointer("/properties/body").is_some();
    let (body, content_type) = request_body(entry, &arguments, has_body)?;
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
    if let Some(content_type) = content_type {
        request.headers_mut().insert(CONTENT_TYPE, content_type);
    }

    let response = crate::console_router(state, true)
        .oneshot(request)
        .await
        .map_err(|error| anyhow::anyhow!("failed to dispatch OpenAPI operation: {error}"))?;
    if !response.status().is_success() {
        return Err(DispatchError::Target(response));
    }
    if response.status() == StatusCode::NO_CONTENT {
        validate(&entry.response_schema, &Value::Null, "response_schema")?;
        return Ok(DispatchSuccess::NoContent);
    }
    if entry
        .response_media_type
        .as_deref()
        .is_some_and(|media_type| !is_json_media_type(Some(media_type)))
    {
        return media_response(response, entry.response_media_type.as_deref()).await;
    }
    let value = parse_response(response, entry.response_media_type.as_deref()).await?;
    let contract_value = value.get("data").unwrap_or(&value);
    validate(&entry.response_schema, contract_value, "response_schema")?;
    Ok(DispatchSuccess::Json(value))
}

fn request_body(
    entry: &OpenApiInterfaceCatalogEntry,
    arguments: &DispatchArguments,
    has_body: bool,
) -> anyhow::Result<(Body, Option<HeaderValue>)> {
    if !has_body {
        return Ok((Body::empty(), None));
    }
    let media_type = entry
        .request_media_type
        .as_deref()
        .ok_or_else(|| invalid("request_media_type"))?;
    if is_json_media_type(Some(media_type)) {
        return Ok((
            Body::from(arguments.body.to_string()),
            Some(HeaderValue::from_static("application/json")),
        ));
    }
    if media_type == "application/octet-stream" {
        let binary = decode_binary_input(&arguments.body)?;
        return Ok((
            Body::from(binary.bytes),
            Some(HeaderValue::from_static("application/octet-stream")),
        ));
    }
    if media_type == "multipart/form-data" {
        let (bytes, content_type) = encode_multipart(
            &arguments.body,
            entry.request_schema.pointer("/properties/body"),
        )?;
        return Ok((Body::from(bytes), Some(content_type)));
    }
    Err(invalid("request_media_type"))
}

fn validate_request(
    entry: &OpenApiInterfaceCatalogEntry,
    arguments: &DispatchArguments,
) -> anyhow::Result<()> {
    let mut value = arguments_value(arguments);
    if entry.request_media_type.as_deref() == Some("application/octet-stream") {
        if let Some(object) = value.as_object_mut() {
            object.remove("body");
        }
        let mut schema = entry.request_schema.clone();
        if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
            properties.remove("body");
        }
        if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
            required.retain(|field| field.as_str() != Some("body"));
        }
        validate(&schema, &value, "request_schema")?;
        decode_binary_input(&arguments.body)?;
        return Ok(());
    }
    if entry.request_media_type.as_deref() == Some("multipart/form-data") {
        if let (Some(body_schema), Some(body)) = (
            entry.request_schema.pointer("/properties/body"),
            value.pointer_mut("/body"),
        ) {
            *body = normalize_binary_inputs(body_schema, body)?;
        }
    }
    validate(&entry.request_schema, &value, "request_schema")
}

fn normalize_binary_inputs(schema: &Value, value: &Value) -> anyhow::Result<Value> {
    if schema.get("format").and_then(Value::as_str) == Some("binary") {
        let input = decode_binary_input(value)?;
        return Ok(Value::String(STANDARD.encode(input.bytes)));
    }
    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        return Ok(Value::Object(
            object
                .iter()
                .map(|(name, value)| {
                    let normalized = properties
                        .get(name)
                        .map(|schema| normalize_binary_inputs(schema, value))
                        .transpose()?
                        .unwrap_or_else(|| value.clone());
                    Ok((name.clone(), normalized))
                })
                .collect::<anyhow::Result<Map<String, Value>>>()?,
        ));
    }
    Ok(value.clone())
}

struct BinaryInput {
    bytes: Vec<u8>,
    file_name: String,
    content_type: String,
}

fn decode_binary_input(value: &Value) -> anyhow::Result<BinaryInput> {
    let (encoded, file_name, content_type) = if let Some(encoded) = value.as_str() {
        (encoded, "upload.bin", "application/octet-stream")
    } else {
        let object = value.as_object().ok_or_else(|| invalid("binary_input"))?;
        (
            object
                .get("base64")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("binary_input"))?,
            object
                .get("file_name")
                .and_then(Value::as_str)
                .unwrap_or("upload.bin"),
            object
                .get("content_type")
                .and_then(Value::as_str)
                .unwrap_or("application/octet-stream"),
        )
    };
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| invalid("binary_input"))?;
    if bytes.len() > MAX_CALLABLE_BINARY_BYTES {
        return Err(invalid("binary_input_limit"));
    }
    Ok(BinaryInput {
        bytes,
        file_name: safe_disposition_value(file_name),
        content_type: safe_content_type(content_type),
    })
}

fn encode_multipart(
    body: &Value,
    body_schema: Option<&Value>,
) -> anyhow::Result<(Vec<u8>, HeaderValue)> {
    let fields = body.as_object().ok_or_else(|| invalid("request_schema"))?;
    let properties = body_schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object);
    let boundary = format!("1flowbase-{}", uuid::Uuid::new_v4().simple());
    let mut output = Vec::new();
    for (name, value) in fields {
        output.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        let is_binary = properties
            .and_then(|properties| properties.get(name))
            .and_then(|schema| schema.get("format"))
            .and_then(Value::as_str)
            == Some("binary");
        if is_binary {
            let binary = decode_binary_input(value)?;
            output.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                    safe_disposition_value(name),
                    binary.file_name,
                    binary.content_type
                )
                .as_bytes(),
            );
            output.extend_from_slice(&binary.bytes);
        } else {
            output.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"\r\n\r\n{}",
                    safe_disposition_value(name),
                    scalar_or_json(value)
                )
                .as_bytes(),
            );
        }
        output.extend_from_slice(b"\r\n");
        if output.len() > MAX_CALLABLE_BINARY_BYTES {
            return Err(invalid("binary_input_limit"));
        }
    }
    output.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let content_type = HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}"))
        .map_err(|_| invalid("request_media_type"))?;
    Ok((output, content_type))
}

fn safe_disposition_value(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '\r' | '\n' | '"'))
        .take(255)
        .collect()
}

fn safe_content_type(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '/' | '+' | '-' | '.' | ';' | '=')
        })
        .take(127)
        .collect()
}

fn scalar_or_json(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

async fn media_response(
    response: Response,
    media_type: Option<&str>,
) -> Result<DispatchSuccess, DispatchError> {
    if media_type == Some("text/event-stream") {
        return Ok(DispatchSuccess::Media(response));
    }
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, MAX_CALLABLE_BINARY_BYTES)
        .await
        .map_err(|_| invalid("binary_response_limit"))?;
    Ok(DispatchSuccess::Media(Response::from_parts(
        parts,
        Body::from(bytes),
    )))
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

    #[test]
    fn controlled_binary_input_encodes_multipart_without_exposing_transport_headers() {
        let schema = json!({
            "type": "object",
            "properties": {
                "file_table_id": { "type": "string" },
                "file": { "type": "string", "format": "binary" }
            },
            "required": ["file_table_id", "file"]
        });
        let body = json!({
            "file_table_id": "table-1",
            "file": {
                "base64": STANDARD.encode(b"hello"),
                "file_name": "hello.txt\r\nunsafe",
                "content_type": "text/plain"
            }
        });

        let normalized = normalize_binary_inputs(&schema, &body).unwrap();
        assert!(validate(&schema, &normalized, "request_schema").is_ok());
        let (encoded, content_type) = encode_multipart(&body, Some(&schema)).unwrap();
        let encoded = String::from_utf8(encoded).unwrap();
        assert!(content_type
            .to_str()
            .unwrap()
            .starts_with("multipart/form-data; boundary="));
        assert!(encoded.contains("name=\"file_table_id\"\r\n\r\ntable-1"));
        assert!(encoded.contains("filename=\"hello.txtunsafe\""));
        assert!(encoded.contains("Content-Type: text/plain"));
        assert!(encoded.contains("\r\n\r\nhello\r\n"));
    }
}
