use anyhow::{anyhow, Result};
use plugin_framework::provider_contract::{
    protocol_context_envelope_json_schema, validate_protocol_context_envelope,
    ProtocolContextEnvelope, PROTOCOL_CONTEXT_VALUE_TYPE,
};
use serde_json::Value;

use crate::compiled_plan::CompiledOutput;

pub fn validate_output_value(output: &CompiledOutput, value: &Value) -> Result<()> {
    validate_output_value_type(output, value)?;

    if output.value_type == PROTOCOL_CONTEXT_VALUE_TYPE {
        validate_protocol_context_value(value).map_err(|error| {
            if let Some(violation) = error.downcast_ref::<ProtocolContextContractViolation>() {
                return anyhow::Error::new(ProtocolContextContractViolation {
                    selector: output_display_path(output),
                    instance_path: violation.instance_path.clone(),
                    reason: violation.reason.clone(),
                });
            }
            error
        })?;
    }

    if let Some(schema) = &output.json_schema {
        let validator = jsonschema::validator_for(schema).map_err(|error| {
            anyhow!(
                "output `{}` declares invalid jsonSchema: {error}",
                output_display_path(output)
            )
        })?;
        validator.validate(value).map_err(|error| {
            anyhow!(
                "output `{}` does not match declared jsonSchema: {error}",
                output_display_path(output)
            )
        })?;
    }

    Ok(())
}

#[derive(Debug)]
pub struct ProtocolContextContractViolation {
    pub selector: String,
    pub instance_path: String,
    pub reason: String,
}

impl std::fmt::Display for ProtocolContextContractViolation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}{}: {}",
            self.selector, self.instance_path, self.reason
        )
    }
}

impl std::error::Error for ProtocolContextContractViolation {}

pub fn validate_protocol_context_value(value: &Value) -> Result<ProtocolContextEnvelope> {
    let schema = protocol_context_envelope_json_schema();
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| anyhow!("canonical ProtocolContextEnvelope schema is invalid: {error}"))?;
    validator.validate(value).map_err(|error| {
        anyhow::Error::new(ProtocolContextContractViolation {
            selector: String::new(),
            instance_path: error.instance_path().to_string(),
            reason: error.to_string(),
        })
    })?;
    let envelope: ProtocolContextEnvelope =
        serde_json::from_value(value.clone()).map_err(|error| {
            anyhow::Error::new(ProtocolContextContractViolation {
                selector: String::new(),
                instance_path: String::new(),
                reason: error.to_string(),
            })
        })?;
    validate_protocol_context_envelope(&envelope).map_err(|reason| {
        anyhow::Error::new(ProtocolContextContractViolation {
            selector: String::new(),
            instance_path: String::new(),
            reason,
        })
    })?;
    Ok(envelope)
}

pub fn output_schema_is_llm_context_messages(output: &CompiledOutput) -> bool {
    if output.value_type != "array" && output.value_type != "array[object]" {
        return false;
    }

    output
        .json_schema
        .as_ref()
        .is_some_and(is_llm_context_messages_schema)
}

pub fn is_llm_context_messages_schema(schema: &Value) -> bool {
    let Some(object) = schema.as_object() else {
        return false;
    };
    if object.get("type").and_then(Value::as_str) != Some("array") {
        return false;
    }

    let Some(items) = object.get("items").and_then(Value::as_object) else {
        return false;
    };
    if items.get("type").and_then(Value::as_str) != Some("object") {
        return false;
    }

    let required = items
        .get("required")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if !required.contains(&"role") || !required.contains(&"content") {
        return false;
    }

    let Some(properties) = items.get("properties").and_then(Value::as_object) else {
        return false;
    };
    property_type(properties.get("role")) == Some("string")
        && property_type(properties.get("content")) == Some("string")
}

pub fn history_messages_schema() -> Value {
    serde_json::json!({
        "type": "array",
        "items": {
            "type": "object",
            "required": ["role", "content"],
            "properties": {
                "role": {
                    "type": "string",
                    "enum": ["system", "user", "assistant", "tool"]
                },
                "content": { "type": "string" },
                "name": { "type": "string" },
                "tool_call_id": { "type": "string" },
                "is_error": { "type": "boolean" },
                "tool_calls": { "type": "array" },
                "content_blocks": { "type": "array" }
            }
        }
    })
}

pub fn value_is_llm_context_messages(value: &Value) -> bool {
    value.as_array().is_some_and(|messages| {
        messages.iter().all(|message| {
            let Some(object) = message.as_object() else {
                return false;
            };
            object.get("role").and_then(Value::as_str).is_some()
                && object.get("content").and_then(Value::as_str).is_some()
        })
    })
}

fn property_type(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
}

fn validate_output_value_type(output: &CompiledOutput, value: &Value) -> Result<()> {
    let valid = match output.value_type.as_str() {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" | "array[object]" => value.is_array(),
        "json" | "unknown" => true,
        PROTOCOL_CONTEXT_VALUE_TYPE => value.is_object(),
        _ => true,
    };

    if valid {
        Ok(())
    } else {
        Err(anyhow!(
            "output `{}` expected valueType `{}`",
            output_display_path(output),
            output.value_type
        ))
    }
}

fn output_display_path(output: &CompiledOutput) -> String {
    if output.selector.is_empty() {
        output.key.clone()
    } else {
        output.selector.join(".")
    }
}
