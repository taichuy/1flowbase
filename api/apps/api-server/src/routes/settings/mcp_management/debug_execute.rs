use std::sync::Arc;

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

use crate::app_state::ApiState;
use domain::mcp_management::{McpParameterDescriptor, McpParameterType};

const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");

#[derive(Debug, Deserialize, ToSchema)]
pub struct McpDebugExecuteBody {
    pub interface_id: String,
    #[serde(default)]
    pub debug_response_mode: McpDebugResponseMode,
    #[schema(value_type = Object)]
    pub mcp_arguments: Value,
    #[schema(value_type = Object)]
    pub input_mapping: Value,
    #[schema(value_type = Object)]
    pub output_mapping: Value,
}

#[derive(Debug, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum McpDebugResponseMode {
    ToolResult,
    DebugDetails,
}

impl Default for McpDebugResponseMode {
    fn default() -> Self {
        Self::ToolResult
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpDebugExecuteDetailsResponse {
    #[schema(value_type = Object)]
    pub mcp_arguments: Value,
    #[schema(value_type = Object)]
    pub interface_arguments: Value,
    #[schema(value_type = Object)]
    pub interface_response: Value,
    #[schema(value_type = Object)]
    pub tool_result: Value,
}

pub enum McpDebugExecuteError {
    Api(anyhow::Error),
    TargetResponse(Response),
}

impl From<anyhow::Error> for McpDebugExecuteError {
    fn from(value: anyhow::Error) -> Self {
        Self::Api(value)
    }
}

#[derive(Debug, Deserialize)]
struct McpInputMapping {
    #[serde(default)]
    mappings: Vec<McpInputMappingEntry>,
}

#[derive(Debug, Deserialize)]
struct McpInputMappingEntry {
    interface_param: String,
    mcp_param: String,
    #[serde(default)]
    required: bool,
}

#[derive(Default)]
struct TargetArguments {
    path: Map<String, Value>,
    query: Map<String, Value>,
    body: Map<String, Value>,
}

#[derive(Clone, Copy)]
enum InterfaceParameterTarget {
    Path,
    Query,
    Body,
}

pub async fn execute(
    state: Arc<ApiState>,
    headers: HeaderMap,
    interface_entry: domain::McpInterfaceCatalogEntry,
    body: McpDebugExecuteBody,
) -> Result<Value, McpDebugExecuteError> {
    let interface_arguments =
        build_interface_arguments(&interface_entry, &body.input_mapping, &body.mcp_arguments)?;
    let target_response =
        dispatch_interface_request(state, &headers, &interface_entry, &interface_arguments).await?;
    if !target_response.status().is_success() {
        return Err(McpDebugExecuteError::TargetResponse(target_response));
    }

    let interface_response = parse_target_response_body(target_response).await?;
    let tool_result = map_tool_result(&body.output_mapping, &interface_response);

    if matches!(body.debug_response_mode, McpDebugResponseMode::ToolResult) {
        return Ok(tool_result);
    }

    serde_json::to_value(McpDebugExecuteDetailsResponse {
        mcp_arguments: body.mcp_arguments,
        interface_arguments: interface_arguments.to_value(),
        interface_response,
        tool_result,
    })
    .map_err(|error| anyhow::anyhow!("failed to serialize MCP debug details: {error}").into())
}

fn build_interface_arguments(
    interface_entry: &domain::McpInterfaceCatalogEntry,
    input_mapping: &Value,
    mcp_arguments: &Value,
) -> anyhow::Result<TargetArguments> {
    if !mcp_arguments.is_object() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("mcp_arguments").into());
    }
    let input_mapping: McpInputMapping = serde_json::from_value(input_mapping.clone())
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("input_mapping"))?;
    let mut arguments = TargetArguments::default();

    for mapping in input_mapping.mappings {
        let mcp_value = match get_path_value(mcp_arguments, &mapping.mcp_param) {
            Some(value) if !is_blank_argument(value) => value.clone(),
            _ if mapping.required => {
                return Err(
                    control_plane::errors::ControlPlaneError::InvalidInput("mcp_arguments").into(),
                )
            }
            _ => continue,
        };
        let descriptor = interface_entry
            .parameter_descriptors
            .iter()
            .find(|descriptor| descriptor.name == mapping.interface_param)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "input_mapping",
            ))?;

        match parameter_target(interface_entry, descriptor) {
            InterfaceParameterTarget::Path => {
                arguments
                    .path
                    .insert(mapping.interface_param.clone(), mcp_value);
            }
            InterfaceParameterTarget::Query => {
                arguments
                    .query
                    .insert(mapping.interface_param.clone(), mcp_value);
            }
            InterfaceParameterTarget::Body => {
                set_path_value(&mut arguments.body, &mapping.interface_param, mcp_value);
            }
        }
    }

    Ok(arguments)
}

fn parameter_target(
    interface_entry: &domain::McpInterfaceCatalogEntry,
    descriptor: &McpParameterDescriptor,
) -> InterfaceParameterTarget {
    match descriptor.parameter_type {
        McpParameterType::JsonBody | McpParameterType::Form => InterfaceParameterTarget::Body,
        McpParameterType::Url => {
            if parameter_schema_has_location_field(
                &interface_entry.parameter_schema,
                "path",
                &descriptor.name,
            ) {
                InterfaceParameterTarget::Path
            } else {
                InterfaceParameterTarget::Query
            }
        }
    }
}

fn parameter_schema_has_location_field(schema: &Value, location: &str, field: &str) -> bool {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(location))
        .and_then(|location_schema| location_schema.get("properties"))
        .and_then(Value::as_object)
        .is_some_and(|properties| properties.contains_key(field))
}

async fn dispatch_interface_request(
    state: Arc<ApiState>,
    headers: &HeaderMap,
    interface_entry: &domain::McpInterfaceCatalogEntry,
    arguments: &TargetArguments,
) -> anyhow::Result<Response> {
    let method = Method::from_bytes(interface_entry.method.as_bytes())
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("interface_id"))?;
    let uri = target_uri(interface_entry, arguments)?;
    let should_send_json_body = interface_has_body_parameters(interface_entry);
    let request_body = if should_send_json_body {
        Body::from(Value::Object(arguments.body.clone()).to_string())
    } else {
        Body::empty()
    };
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .body(request_body)
        .map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("interface_arguments")
        })?;

    copy_forwarded_header(headers, request.headers_mut(), COOKIE);
    copy_forwarded_header(headers, request.headers_mut(), AUTHORIZATION);
    copy_forwarded_header(headers, request.headers_mut(), ACCEPT_LANGUAGE);
    copy_forwarded_header(headers, request.headers_mut(), CSRF_HEADER);
    if should_send_json_body {
        request.headers_mut().insert(
            CONTENT_TYPE,
            "application/json"
                .parse()
                .expect("application/json must be a valid header value"),
        );
    }

    crate::console_router(state, true)
        .oneshot(request)
        .await
        .map_err(|error| anyhow::anyhow!("failed to execute MCP interface: {error}"))
}

fn interface_has_body_parameters(interface_entry: &domain::McpInterfaceCatalogEntry) -> bool {
    interface_entry
        .parameter_descriptors
        .iter()
        .any(|descriptor| {
            matches!(
                descriptor.parameter_type,
                McpParameterType::JsonBody | McpParameterType::Form
            )
        })
}

fn copy_forwarded_header(source: &HeaderMap, target: &mut HeaderMap, header_name: HeaderName) {
    if let Some(value) = source.get(&header_name) {
        target.insert(header_name, value.clone());
    }
}

fn target_uri(
    interface_entry: &domain::McpInterfaceCatalogEntry,
    arguments: &TargetArguments,
) -> anyhow::Result<String> {
    let mut path = interface_entry.path.clone();
    for (name, value) in &arguments.path {
        let value = scalar_to_path_segment(value)?;
        path = path.replace(
            &format!("{{{name}}}"),
            &form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>(),
        );
    }
    if path.contains('{') || path.contains('}') {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("interface_arguments").into(),
        );
    }
    if arguments.query.is_empty() {
        return Ok(path);
    }

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (name, value) in &arguments.query {
        serializer.append_pair(name, &scalar_to_query_value(value));
    }
    Ok(format!("{path}?{}", serializer.finish()))
}

async fn parse_target_response_body(response: Response) -> anyhow::Result<Value> {
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| anyhow::anyhow!("failed to read MCP interface response: {error}"))?;
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("interface_response").into()
    })
}

fn map_tool_result(output_mapping: &Value, interface_response: &Value) -> Value {
    let source = interface_response.get("data").unwrap_or(interface_response);
    filter_schema_object(output_mapping, source)
}

fn filter_schema_object(schema: &Value, source: &Value) -> Value {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return source.clone();
    };

    let mut mapped = Map::new();
    for (field, field_schema) in properties {
        let Some(field_source) = get_path_value(source, field) else {
            continue;
        };
        let mapped_value = filter_schema_value(field_schema, field_source);
        set_path_value(&mut mapped, field, mapped_value);
    }

    if mapped.is_empty() {
        source.clone()
    } else {
        Value::Object(mapped)
    }
}

fn filter_schema_value(schema: &Value, source: &Value) -> Value {
    if let (Some(item_schema), Some(items)) = (schema.get("items"), source.as_array()) {
        return Value::Array(
            items
                .iter()
                .map(|item| filter_schema_value(item_schema, item))
                .collect(),
        );
    }

    if schema.get("properties").is_some() {
        return filter_schema_object(schema, source);
    }

    source.clone()
}

impl TargetArguments {
    fn to_value(&self) -> Value {
        let mut value = Map::new();
        if !self.path.is_empty() {
            value.insert("path".into(), Value::Object(self.path.clone()));
        }
        if !self.query.is_empty() {
            value.insert("query".into(), Value::Object(self.query.clone()));
        }
        if !self.body.is_empty() {
            value.insert("body".into(), Value::Object(self.body.clone()));
        }
        Value::Object(value)
    }
}

fn get_path_value<'a>(source: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cursor = source;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        cursor = cursor.get(segment)?;
    }
    Some(cursor)
}

fn set_path_value(target: &mut Map<String, Value>, path: &str, value: Value) {
    let segments = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return;
    }

    let mut cursor = target;
    for segment in &segments[..segments.len() - 1] {
        let entry = cursor
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cursor = entry
            .as_object_mut()
            .expect("entry was just initialized as an object");
    }
    cursor.insert(segments[segments.len() - 1].to_string(), value);
}

fn scalar_to_path_segment(value: &Value) -> anyhow::Result<String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(
            control_plane::errors::ControlPlaneError::InvalidInput("interface_arguments").into(),
        ),
    }
}

fn scalar_to_query_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn is_blank_argument(value: &Value) -> bool {
    matches!(value, Value::Null) || value.as_str().is_some_and(str::is_empty)
}
