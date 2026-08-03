use std::{collections::BTreeMap, sync::Arc};

use axum::{http::HeaderMap, response::Response};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use crate::app_state::ApiState;
use domain::mcp_management::{McpParameterDescriptor, McpParameterType};

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

#[derive(Debug, Default, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum McpDebugResponseMode {
    #[default]
    ToolResult,
    DebugDetails,
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
    let dispatch_entry = crate::openapi_interface::OpenApiInterfaceCatalogEntry {
        operation_id: interface_entry.interface_id.clone(),
        method: interface_entry.method.clone(),
        path: interface_entry.path.clone(),
        name: interface_entry.name.clone(),
        description: interface_entry.short_description.clone(),
        parameter_descriptors: Vec::new(),
        request_schema: interface_entry.parameter_schema.clone(),
        response_schema: interface_entry.result_schema.clone(),
        request_media_type: Some("application/json".to_string()),
        response_media_type: Some("application/json".to_string()),
        security: interface_entry.security.clone(),
    };
    let dispatch_arguments = crate::openapi_interface::DispatchArguments {
        path: interface_arguments.path.clone(),
        query: interface_arguments.query.clone(),
        headers: Map::new(),
        body: if interface_arguments.body.is_empty() {
            Value::Null
        } else {
            Value::Object(interface_arguments.body.clone())
        },
    };
    let interface_response = match crate::openapi_interface::dispatch(
        state,
        &headers,
        &dispatch_entry,
        dispatch_arguments,
        BTreeMap::new(),
    )
    .await
    {
        Ok(crate::openapi_interface::DispatchSuccess::Json(value)) => value,
        Ok(crate::openapi_interface::DispatchSuccess::NoContent) => Value::Null,
        Ok(crate::openapi_interface::DispatchSuccess::Media(_)) => {
            return Err(
                anyhow::anyhow!("MCP debug execute requires a JSON interface response").into(),
            )
        }
        Err(crate::openapi_interface::DispatchError::Api(error)) => {
            return Err(McpDebugExecuteError::Api(error));
        }
        Err(crate::openapi_interface::DispatchError::Target(response)) => {
            return Err(McpDebugExecuteError::TargetResponse(response));
        }
    };
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
    let mut next_parameter_target = BTreeMap::<String, usize>::new();

    for mapping in input_mapping.mappings {
        // The persisted mapping identifies a parameter by name, so repeated names consume the
        // canonical OpenAPI location order independently of which optional values are present.
        let target_index = {
            let next_index = next_parameter_target
                .entry(mapping.interface_param.clone())
                .or_default();
            let current_index = *next_index;
            *next_index = (*next_index).saturating_add(1);
            current_index
        };
        let mcp_value = match get_path_value(mcp_arguments, &mapping.mcp_param) {
            Some(value) => value.clone(),
            _ if mapping.required => {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "mcp_arguments",
                )
                .into());
            }
            _ => continue,
        };
        let targets = parameter_targets(interface_entry, &mapping.interface_param)?;
        let target = targets
            .get(target_index)
            .or_else(|| targets.first())
            .copied()
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "input_mapping",
            ))?;

        match target {
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

    materialize_required_object_containers(
        interface_entry.parameter_schema.pointer("/properties/path"),
        &mut arguments.path,
    );
    materialize_required_object_containers(
        interface_entry
            .parameter_schema
            .pointer("/properties/query"),
        &mut arguments.query,
    );
    materialize_required_object_containers(
        interface_entry.parameter_schema.pointer("/properties/body"),
        &mut arguments.body,
    );

    Ok(arguments)
}

fn materialize_required_object_containers(schema: Option<&Value>, target: &mut Map<String, Value>) {
    let Some(schema) = schema else {
        return;
    };
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };

    for (name, property_schema) in properties {
        if required.contains(name.as_str())
            && !target.contains_key(name)
            && schema_describes_object(property_schema)
        {
            target.insert(name.clone(), Value::Object(Map::new()));
        }
        if let Some(value) = target.get_mut(name).and_then(Value::as_object_mut) {
            materialize_required_object_containers(Some(property_schema), value);
        }
    }
}

fn schema_describes_object(schema: &Value) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("object")
        || schema.get("properties").is_some_and(Value::is_object)
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

fn parameter_targets(
    interface_entry: &domain::McpInterfaceCatalogEntry,
    parameter_name: &str,
) -> anyhow::Result<Vec<InterfaceParameterTarget>> {
    let mut targets = Vec::new();
    if parameter_schema_has_location_field(
        &interface_entry.parameter_schema,
        "path",
        parameter_name,
    ) {
        targets.push(InterfaceParameterTarget::Path);
    }
    if parameter_schema_has_location_field(
        &interface_entry.parameter_schema,
        "query",
        parameter_name,
    ) {
        targets.push(InterfaceParameterTarget::Query);
    }
    if parameter_schema_has_location_field(
        &interface_entry.parameter_schema,
        "body",
        parameter_name,
    ) {
        targets.push(InterfaceParameterTarget::Body);
    }
    if !targets.is_empty() {
        return Ok(targets);
    }

    interface_entry
        .parameter_descriptors
        .iter()
        .find(|descriptor| descriptor.name == parameter_name)
        .map(|descriptor| vec![parameter_target(interface_entry, descriptor)])
        .ok_or_else(|| {
            control_plane::errors::ControlPlaneError::InvalidInput("input_mapping").into()
        })
}

fn parameter_schema_has_location_field(schema: &Value, location: &str, field: &str) -> bool {
    let Some(mut cursor) = schema
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(location))
    else {
        return false;
    };
    for segment in field.split('.').filter(|segment| !segment.is_empty()) {
        let Some(next) = cursor
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(segment))
        else {
            return false;
        };
        cursor = next;
    }
    true
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
