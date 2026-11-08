//! Document-shaped event values owned by the application HTTP adapters.
use crate::openapi_interface::contract::ApiOperationContext;
use serde_json::{json, Value};

pub(super) mod runtime;

/// Route parameter spelling comes from the compiled declaration, never request data.
pub(super) fn request(
    operation: &ApiOperationContext,
    mut path: Value,
    mut query: Value,
    body: Value,
) -> Value {
    if operation
        .path
        .split('/')
        .any(|part| part == ":id" || part == "{id}")
    {
        if let Some(fields) = path.as_object_mut() {
            if let Some(application_id) = fields.remove("application_id") {
                fields.insert("id".into(), application_id);
            }
        }
    }
    // An absent optional query parameter is absent, rather than a JSON null parameter.
    if let Some(fields) = query.as_object_mut() {
        fields.retain(|_, value| !value.is_null());
    }
    let mut value = serde_json::Map::new();
    if path.as_object().is_some_and(|fields| !fields.is_empty()) {
        value.insert("path".into(), path);
    }
    if query.as_object().is_some_and(|fields| !fields.is_empty()) {
        value.insert("query".into(), query);
    }
    if !body.is_null() {
        value.insert("body".into(), body);
    }
    Value::Object(value)
}

/// Only the documented preview parameters belong to this owner. Unknown raw query
/// parameters can carry credentials and must never enter an event.
pub(super) fn preview_query(raw_query: Option<&str>) -> Value {
    let mut query = serde_json::Map::new();
    let mut fields = Vec::new();
    for (key, value) in form_urlencoded::parse(raw_query.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "artifact_preview" => {
                query.insert(key.into_owned(), Value::String(value.into_owned()));
            }
            "artifact_preview_field" => fields.push(Value::String(value.into_owned())),
            _ => {}
        }
    }
    if !fields.is_empty() {
        query.insert("artifact_preview_field".into(), Value::Array(fields));
    }
    Value::Object(query)
}

// Raw environment values are private even when the HTTP resource returns them.
// These DTOs describe and serialize exactly the fields allowed in API events.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub(super) struct EnvironmentVariableBody {
    pub name: String,
    pub value_type: String,
    pub description: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(super) struct EnvironmentVariablesBody {
    pub variables: Vec<EnvironmentVariableBody>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(super) struct EnvironmentVariableResponse {
    pub name: String,
    pub value_type: String,
    pub description: String,
    pub updated_at: String,
}

pub(super) fn environment_body(
    value: &super::applications::ReplaceApplicationEnvironmentVariablesBody,
) -> EnvironmentVariablesBody {
    EnvironmentVariablesBody {
        variables: value
            .variables
            .iter()
            .map(|item| EnvironmentVariableBody {
                name: item.name.clone(),
                value_type: item.value_type.clone(),
                description: item.description.clone(),
            })
            .collect(),
    }
}

pub(super) fn environment_response(
    value: &[super::applications::ApplicationEnvironmentVariableResponse],
) -> Vec<EnvironmentVariableResponse> {
    value
        .iter()
        .map(|item| EnvironmentVariableResponse {
            name: item.name.clone(),
            value_type: item.value_type.clone(),
            description: item.description.clone(),
            updated_at: item.updated_at.clone(),
        })
        .collect()
}

pub(super) fn environment_input_schema(operation: &ApiOperationContext) -> Value {
    application_body_schema(
        operation,
        crate::openapi_interface::contract::schema::<EnvironmentVariablesBody>(),
    )
}

pub(super) fn application_body_schema(operation: &ApiOperationContext, body: Value) -> Value {
    let key = if operation
        .path
        .split('/')
        .any(|part| part == ":id" || part == "{id}")
    {
        "id"
    } else {
        "application_id"
    };
    json!({"type":"object","properties":{
        "path":{"type":"object","properties":{key:{"type":"string","format":"uuid"}},"required":[key],"additionalProperties":false},
        "body":body
    },"required":["path","body"],"additionalProperties":false})
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/document_event_projection.rs"]
mod tests;
