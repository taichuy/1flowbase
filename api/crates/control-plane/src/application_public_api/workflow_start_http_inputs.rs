use std::collections::BTreeSet;

use serde_json::{Map, Number, Value};
use thiserror::Error;

use super::workflow_extension::WorkflowExtensionRequestParameters;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStartHttpInputSource {
    Path,
    Query,
    Body,
    Form,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStartHttpInputValueType {
    String,
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowStartHttpInputField {
    key: String,
    value_type: WorkflowStartHttpInputValueType,
    source: WorkflowStartHttpInputSource,
    required: bool,
    default_value: Option<Value>,
}

impl WorkflowStartHttpInputField {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn source(&self) -> WorkflowStartHttpInputSource {
        self.source
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn default_value(&self) -> Option<&Value> {
        self.default_value.as_ref()
    }

    pub fn value_type(&self) -> WorkflowStartHttpInputValueType {
        self.value_type
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowStartHttpInputs {
    start_node_id: String,
    fields: Vec<WorkflowStartHttpInputField>,
}

impl WorkflowStartHttpInputs {
    pub fn start_node_id(&self) -> &str {
        &self.start_node_id
    }

    pub fn fields(&self) -> &[WorkflowStartHttpInputField] {
        &self.fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkflowStartHttpInputError {
    #[error("workflow start node is missing")]
    WorkflowStartMissing,
    #[error("workflow start input_fields must be an array")]
    InputFieldsInvalid,
    #[error("workflow start input key is missing")]
    KeyMissing,
    #[error("workflow start input key is duplicated: {0}")]
    DuplicateKey(String),
    #[error("workflow start input source is invalid for {key}: {invalid_source}")]
    InvalidSource { key: String, invalid_source: String },
    #[error("workflow start input value type is invalid for {key}: {value_type}")]
    InvalidValueType { key: String, value_type: String },
    #[error("workflow start input target selector is not allowed: {0}")]
    TargetSelectorNotAllowed(String),
    #[error("workflow start input is required: {0}")]
    RequiredValueMissing(String),
    #[error("workflow start input value is invalid for {key}: {value_type}")]
    InvalidValue { key: String, value_type: String },
}

pub fn parse_workflow_start_http_inputs(
    document_snapshot: &Value,
) -> Result<WorkflowStartHttpInputs, WorkflowStartHttpInputError> {
    parse_workflow_start_inputs(document_snapshot, WorkflowStartInputUsage::Http)
}

pub fn parse_workflow_start_schedule_inputs(
    document_snapshot: &Value,
) -> Result<WorkflowStartHttpInputs, WorkflowStartHttpInputError> {
    parse_workflow_start_inputs(document_snapshot, WorkflowStartInputUsage::Schedule)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkflowStartInputUsage {
    Http,
    Schedule,
}

fn parse_workflow_start_inputs(
    document_snapshot: &Value,
    usage: WorkflowStartInputUsage,
) -> Result<WorkflowStartHttpInputs, WorkflowStartHttpInputError> {
    let start_node = document_snapshot
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node.get("type").and_then(Value::as_str) == Some("workflow_start"))
        })
        .ok_or(WorkflowStartHttpInputError::WorkflowStartMissing)?;
    let start_node_id = start_node
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or(WorkflowStartHttpInputError::WorkflowStartMissing)?
        .to_string();
    let input_fields_value = start_node
        .get("config")
        .and_then(|config| config.get("input_fields"))
        .unwrap_or(&Value::Null);
    let input_fields = match input_fields_value {
        Value::Null => &[],
        Value::Array(input_fields) => input_fields.as_slice(),
        _ => return Err(WorkflowStartHttpInputError::InputFieldsInvalid),
    };

    let mut keys = BTreeSet::new();
    let mut fields = Vec::with_capacity(input_fields.len());
    for input_field in input_fields {
        let key = input_field
            .get("key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or(WorkflowStartHttpInputError::KeyMissing)?
            .to_string();
        if !keys.insert(key.clone()) {
            return Err(WorkflowStartHttpInputError::DuplicateKey(key));
        }
        if input_field.get("target").is_some() {
            return Err(WorkflowStartHttpInputError::TargetSelectorNotAllowed(key));
        }

        let source = if usage == WorkflowStartInputUsage::Http {
            let source_value = input_field
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match source_value {
                "path" => WorkflowStartHttpInputSource::Path,
                "query" => WorkflowStartHttpInputSource::Query,
                "body" => WorkflowStartHttpInputSource::Body,
                "form" => WorkflowStartHttpInputSource::Form,
                _ => {
                    return Err(WorkflowStartHttpInputError::InvalidSource {
                        key,
                        invalid_source: source_value.to_string(),
                    });
                }
            }
        } else {
            // Schedule defaults are keyed by the Workflow Start contract;
            // HTTP transport source is deliberately irrelevant here.
            WorkflowStartHttpInputSource::Body
        };
        let value_type_value = input_field
            .get("valueType")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let value_type = match value_type_value {
            "string" => WorkflowStartHttpInputValueType::String,
            "number" => WorkflowStartHttpInputValueType::Number,
            "boolean" => WorkflowStartHttpInputValueType::Boolean,
            _ => {
                return Err(WorkflowStartHttpInputError::InvalidValueType {
                    key,
                    value_type: value_type_value.to_string(),
                });
            }
        };
        let default_value = input_field
            .get("defaultValue")
            .filter(|value| !value.is_null())
            .map(|value| coerce_value(&key, value_type, value))
            .transpose()?;
        fields.push(WorkflowStartHttpInputField {
            key,
            value_type,
            source,
            required: input_field
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            default_value,
        });
    }

    Ok(WorkflowStartHttpInputs {
        start_node_id,
        fields,
    })
}

pub fn build_workflow_start_schedule_input_payload(
    contract: &WorkflowStartHttpInputs,
    configured_defaults: &Value,
) -> Result<Value, WorkflowStartHttpInputError> {
    let defaults = configured_defaults
        .as_object()
        .ok_or(WorkflowStartHttpInputError::InputFieldsInvalid)?;
    for key in defaults.keys() {
        if !contract.fields.iter().any(|field| field.key == *key) {
            return Err(WorkflowStartHttpInputError::TargetSelectorNotAllowed(
                key.clone(),
            ));
        }
    }

    let mut start_node_payload = Map::new();
    for field in &contract.fields {
        let value = match defaults.get(&field.key) {
            Some(value) => coerce_value(&field.key, field.value_type, value)?,
            None => match &field.default_value {
                Some(default_value) => default_value.clone(),
                None if field.required => {
                    return Err(WorkflowStartHttpInputError::RequiredValueMissing(
                        field.key.clone(),
                    ));
                }
                None => continue,
            },
        };
        start_node_payload.insert(field.key.clone(), value);
    }

    Ok(Value::Object(Map::from_iter([(
        contract.start_node_id.clone(),
        Value::Object(start_node_payload),
    )])))
}

pub fn build_workflow_start_node_input_payload(
    contract: &WorkflowStartHttpInputs,
    parameters: &WorkflowExtensionRequestParameters,
) -> Result<Value, WorkflowStartHttpInputError> {
    let mut start_node_payload = Map::new();
    for field in &contract.fields {
        let request_value = match field.source {
            WorkflowStartHttpInputSource::Path => parameters.path.get(&field.key),
            WorkflowStartHttpInputSource::Query => parameters.query.get(&field.key),
            WorkflowStartHttpInputSource::Body => parameters
                .body
                .as_object()
                .and_then(|body| body.get(&field.key)),
            WorkflowStartHttpInputSource::Form => parameters.form.get(&field.key),
        };
        let value = match request_value {
            Some(value) => coerce_value(&field.key, field.value_type, value)?,
            None => match &field.default_value {
                Some(default_value) => default_value.clone(),
                None if field.required => {
                    return Err(WorkflowStartHttpInputError::RequiredValueMissing(
                        field.key.clone(),
                    ));
                }
                None => continue,
            },
        };
        start_node_payload.insert(field.key.clone(), value);
    }

    Ok(Value::Object(Map::from_iter([(
        contract.start_node_id.clone(),
        Value::Object(start_node_payload),
    )])))
}

fn coerce_value(
    key: &str,
    value_type: WorkflowStartHttpInputValueType,
    value: &Value,
) -> Result<Value, WorkflowStartHttpInputError> {
    let invalid_value = || WorkflowStartHttpInputError::InvalidValue {
        key: key.to_string(),
        value_type: value_type.as_str().to_string(),
    };
    match value_type {
        WorkflowStartHttpInputValueType::String => match value {
            Value::String(value) => Ok(Value::String(value.clone())),
            Value::Number(value) => Ok(Value::String(value.to_string())),
            Value::Bool(value) => Ok(Value::String(value.to_string())),
            _ => Err(invalid_value()),
        },
        WorkflowStartHttpInputValueType::Number => match value {
            Value::Number(value) => Ok(Value::Number(value.clone())),
            Value::String(value) => value
                .parse::<f64>()
                .ok()
                .and_then(Number::from_f64)
                .map(Value::Number)
                .ok_or_else(invalid_value),
            _ => Err(invalid_value()),
        },
        WorkflowStartHttpInputValueType::Boolean => match value {
            Value::Bool(value) => Ok(Value::Bool(*value)),
            Value::String(value) if value == "true" => Ok(Value::Bool(true)),
            Value::String(value) if value == "false" => Ok(Value::Bool(false)),
            _ => Err(invalid_value()),
        },
    }
}

impl WorkflowStartHttpInputValueType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }
}
