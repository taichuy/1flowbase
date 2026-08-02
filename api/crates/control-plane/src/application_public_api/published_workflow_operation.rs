use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{json, Map, Value};
use thiserror::Error;
use uuid::Uuid;

use super::{
    mapping::{
        WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode,
    },
    publications::ApplicationPublicationVersionRecord,
    workflow_start_http_inputs::{
        parse_workflow_start_http_inputs, WorkflowStartHttpInputField,
        WorkflowStartHttpInputSource, WorkflowStartHttpInputValueType,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublishedWorkflowOperation {
    pub interface_id: String,
    pub application_id: Uuid,
    pub workspace_id: Uuid,
    pub publication_version_id: Uuid,
    pub method: WorkflowExtensionHttpMethod,
    pub route_template: String,
    pub response_mode: WorkflowExtensionResponseMode,
    pub parameter_schema: Value,
    pub result_schema: Value,
    #[serde(skip)]
    pub publication: ApplicationPublicationVersionRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PublishedWorkflowOperationError {
    #[error("published workflow operation is invalid")]
    InvalidContract,
    #[error("workflow route template path fields do not match Workflow Start")]
    PathFieldsMismatch,
    #[error("published workflow route conflicts with another operation")]
    RouteConflict,
    #[error("published workflow operation was not found")]
    NotFound,
}

impl PublishedWorkflowOperation {
    pub fn from_publication(
        publication: ApplicationPublicationVersionRecord,
    ) -> Result<Self, PublishedWorkflowOperationError> {
        let extension = publication
            .mapping_snapshot
            .extension
            .as_ref()
            .ok_or(PublishedWorkflowOperationError::InvalidContract)?;
        let start =
            validate_published_workflow_contract(extension, &publication.document_snapshot)?;
        let route_template = normalize_route_template(&extension.slug)?;

        Ok(Self {
            interface_id: format!(
                "published_workflow_operation:{}",
                publication.application_id
            ),
            application_id: publication.application_id,
            workspace_id: publication.workspace_id,
            publication_version_id: publication.id,
            method: extension.method,
            route_template,
            response_mode: extension.response_mode,
            parameter_schema: workflow_parameter_schema(start.fields()),
            result_schema: workflow_result_schema(&publication.document_snapshot),
            publication,
        })
    }

    pub fn public_path(&self) -> String {
        format!("/api/ex/{}", self.route_template)
    }

    pub fn match_path(&self, request_path: &str) -> Option<BTreeMap<String, Value>> {
        let request_path = request_path.trim_matches('/');
        let template_segments = self.route_template.split('/').collect::<Vec<_>>();
        let request_segments = request_path.split('/').collect::<Vec<_>>();
        if template_segments.len() != request_segments.len() {
            return None;
        }
        let mut values = BTreeMap::new();
        for (template, value) in template_segments.into_iter().zip(request_segments) {
            if let Some(name) = placeholder_name(template) {
                if value.is_empty() {
                    return None;
                }
                values.insert(name.to_string(), Value::String(value.to_string()));
            } else if template != value {
                return None;
            }
        }
        Some(values)
    }
}

pub fn validate_published_workflow_contract(
    extension: &WorkflowExtensionApiConfig,
    document: &Value,
) -> Result<
    super::workflow_start_http_inputs::WorkflowStartHttpInputs,
    PublishedWorkflowOperationError,
> {
    let start = parse_workflow_start_http_inputs(document)
        .map_err(|_| PublishedWorkflowOperationError::InvalidContract)?;
    let route_template = normalize_route_template(&extension.slug)?;
    let placeholders = route_placeholders(&route_template)?;
    let path_fields = start
        .fields()
        .iter()
        .filter(|field| field.source() == WorkflowStartHttpInputSource::Path)
        .map(|field| field.key().to_string())
        .collect::<BTreeSet<_>>();
    if placeholders != path_fields {
        return Err(PublishedWorkflowOperationError::PathFieldsMismatch);
    }
    Ok(start)
}

pub fn workflow_route_shapes_conflict(left: &str, right: &str) -> bool {
    let left = left.split('/').collect::<Vec<_>>();
    let right = right.split('/').collect::<Vec<_>>();
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            *left == right || placeholder_name(left).is_some() || placeholder_name(right).is_some()
        })
}

pub fn build_published_workflow_operations(
    publications: Vec<ApplicationPublicationVersionRecord>,
) -> Result<Vec<PublishedWorkflowOperation>, PublishedWorkflowOperationError> {
    let mut operations = publications
        .into_iter()
        .map(PublishedWorkflowOperation::from_publication)
        .collect::<Result<Vec<_>, _>>()?;
    operations.sort_by(|left, right| {
        left.method
            .cmp(&right.method)
            .then_with(|| left.route_template.cmp(&right.route_template))
            .then_with(|| left.application_id.cmp(&right.application_id))
    });
    for operation in &operations {
        if operations.iter().any(|candidate| {
            candidate.application_id != operation.application_id
                && candidate.method == operation.method
                && workflow_route_shapes_conflict(
                    &candidate.route_template,
                    &operation.route_template,
                )
        }) {
            return Err(PublishedWorkflowOperationError::RouteConflict);
        }
    }
    Ok(operations)
}

pub fn resolve_published_workflow_operation(
    operations: &[PublishedWorkflowOperation],
    method: WorkflowExtensionHttpMethod,
    request_path: &str,
) -> Result<(PublishedWorkflowOperation, BTreeMap<String, Value>), PublishedWorkflowOperationError>
{
    let mut matches = operations
        .iter()
        .filter(|operation| operation.method == method)
        .filter_map(|operation| {
            operation
                .match_path(request_path)
                .map(|path| (operation.clone(), path))
        });
    let resolved = matches
        .next()
        .ok_or(PublishedWorkflowOperationError::NotFound)?;
    if matches.next().is_some() {
        return Err(PublishedWorkflowOperationError::RouteConflict);
    }
    Ok(resolved)
}

fn normalize_route_template(value: &str) -> Result<String, PublishedWorkflowOperationError> {
    let normalized = value.trim_matches('/');
    if normalized.is_empty() || normalized != value {
        return Err(PublishedWorkflowOperationError::InvalidContract);
    }
    route_placeholders(normalized)?;
    Ok(normalized.to_string())
}

fn route_placeholders(
    route_template: &str,
) -> Result<BTreeSet<String>, PublishedWorkflowOperationError> {
    let mut names = BTreeSet::new();
    for segment in route_template.split('/') {
        if segment.contains('{') || segment.contains('}') {
            let name = placeholder_name(segment)
                .ok_or(PublishedWorkflowOperationError::InvalidContract)?;
            if !names.insert(name.to_string()) {
                return Err(PublishedWorkflowOperationError::InvalidContract);
            }
        }
    }
    Ok(names)
}

fn placeholder_name(segment: &str) -> Option<&str> {
    let name = segment.strip_prefix('{')?.strip_suffix('}')?;
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_'))
    .then_some(name)
}

fn workflow_parameter_schema(fields: &[WorkflowStartHttpInputField]) -> Value {
    let mut location_properties = BTreeMap::<&str, Map<String, Value>>::new();
    let mut location_required = BTreeMap::<&str, Vec<Value>>::new();
    for field in fields {
        let location = match field.source() {
            WorkflowStartHttpInputSource::Path => "path",
            WorkflowStartHttpInputSource::Query => "query",
            WorkflowStartHttpInputSource::Body => "body",
            WorkflowStartHttpInputSource::Form => "form",
        };
        let mut schema = value_type_schema(field.value_type());
        if let Some(default_value) = field.default_value() {
            schema["default"] = default_value.clone();
        }
        location_properties
            .entry(location)
            .or_default()
            .insert(field.key().to_string(), schema);
        if field.required() || field.source() == WorkflowStartHttpInputSource::Path {
            location_required
                .entry(location)
                .or_default()
                .push(Value::String(field.key().to_string()));
        }
    }
    let properties = location_properties
        .into_iter()
        .map(|(location, properties)| {
            let required = location_required.remove(location).unwrap_or_default();
            (
                location.to_string(),
                json!({ "type": "object", "properties": properties, "required": required }),
            )
        })
        .collect::<Map<_, _>>();
    json!({ "type": "object", "properties": properties })
}

fn workflow_result_schema(document: &Value) -> Value {
    let outputs = document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node.get("type").and_then(Value::as_str) == Some("workflow_end"))
        })
        .and_then(|node| node.get("outputs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let properties = outputs
        .iter()
        .filter_map(|output| {
            let key = output.get("key").and_then(Value::as_str)?;
            let schema = match output.get("valueType").and_then(Value::as_str) {
                Some("string") => json!({ "type": "string" }),
                Some("number") => json!({ "type": "number" }),
                Some("boolean") => json!({ "type": "boolean" }),
                Some("object") => json!({ "type": "object" }),
                Some("array" | "array[object]") => json!({ "type": "array" }),
                Some("json" | "unknown") | None => json!({}),
                Some(_) => json!({}),
            };
            Some((key.to_string(), schema))
        })
        .collect::<Map<_, _>>();
    json!({
        "type": "object",
        "required": properties.keys().cloned().collect::<Vec<_>>(),
        "properties": properties
    })
}

fn value_type_schema(value_type: WorkflowStartHttpInputValueType) -> Value {
    match value_type {
        WorkflowStartHttpInputValueType::String => json!({ "type": "string" }),
        WorkflowStartHttpInputValueType::Number => json!({ "type": "number" }),
        WorkflowStartHttpInputValueType::Boolean => json!({ "type": "boolean" }),
    }
}
