//! Canonical notification type metadata. Config belongs to an instance; input and output
//! belong to one send. Schemas describe provider data, never an in-app message protocol.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{ContributionId, PluginFormSchema};

pub const NOTIFICATION_POINT: &str = "1flowbase.notification.send";
pub const NOTIFICATION_CONTRACT_V1: &str = "1flowbase.notification/v1";
pub const NOTIFICATION_SEND_PERMISSION: &str = "notification.send";
pub const SYSTEM_NOTIFICATION_TYPE: &str = "system";

mod i18n;
pub use i18n::*;
mod managed;
pub use managed::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationTypeDescriptor {
    /// Package-owned presentation overrides; canonical metadata remains the fallback.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub i18n: BTreeMap<String, NotificationTypeLocale>,
    pub type_code: String,
    pub display_name: String,
    pub description: String,
    pub config_schema: Value,
    pub config_form: PluginFormSchema,
    pub input_schema: Value,
    pub input_form: PluginFormSchema,
    pub output_schema: Value,
    pub contribution_id: ContributionId,
    /// Opt-in managed authorization and typed send outcomes; absent preserves legacy sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<NotificationAuthorizationDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("notification contract violation: {0}")]
pub struct NotificationContractError(pub String);

type ContractResult<T = ()> = Result<T, NotificationContractError>;

impl NotificationTypeDescriptor {
    pub fn validate(&self) -> ContractResult {
        if !identity(&self.type_code) || self.display_name.trim().is_empty() {
            return Err(invalid("type_code and display_name are required"));
        }
        for (name, schema) in [
            ("config_schema", &self.config_schema),
            ("input_schema", &self.input_schema),
            ("output_schema", &self.output_schema),
        ] {
            compile_schema(name, schema)?;
        }
        if self.output_schema.get("type").and_then(Value::as_str) != Some("object") {
            return Err(invalid("output_schema requires an object schema"));
        }
        validate_form("config_form", &self.config_form, &self.config_schema, true)?;
        validate_form("input_form", &self.input_form, &self.input_schema, false)?;
        if let Some(authorization) = &self.authorization {
            authorization.validate()?;
            let properties = self
                .config_schema
                .get("properties")
                .and_then(Value::as_object);
            if authorization.managed_secret_keys.iter().any(|key| {
                properties.is_some_and(|fields| fields.contains_key(key))
                    || self
                        .config_form
                        .fields
                        .iter()
                        .any(|field| &field.key == key)
            }) {
                return Err(invalid(
                    "managed secret keys cannot be user configuration fields",
                ));
            }
        }
        i18n::validate(self)?;
        Ok(())
    }

    /// A package cannot claim another package's type or the Host-owned system type.
    pub fn validate_plugin_namespace(&self, publisher: &str, plugin_code: &str) -> ContractResult {
        self.validate()?;
        let prefix = format!("{publisher}/{plugin_code}/");
        if !self
            .type_code
            .strip_prefix(&prefix)
            .is_some_and(|local| !local.is_empty() && !local.contains('/') && identity(local))
        {
            return Err(invalid(
                "type_code must use publisher_namespace/plugin_code/local_type",
            ));
        }
        Ok(())
    }

    pub fn validate_config(&self, value: &Value) -> ContractResult {
        if self.authorization.as_ref().is_some_and(|authorization| {
            authorization
                .managed_secret_keys
                .iter()
                .any(|key| value.get(key).is_some())
        }) {
            return Err(invalid("managed secret keys cannot be user configuration"));
        }
        validate_value("config_schema", &self.config_schema, value)
    }

    pub fn validate_input(&self, value: &Value) -> ContractResult {
        validate_value("input_schema", &self.input_schema, value)
    }

    pub fn validate_output(&self, value: &Value) -> ContractResult {
        validate_value("output_schema", &self.output_schema, value)
    }

    /// Password is an existing form control marker, not a separate secret DSL.
    pub fn secret_config_fields(&self) -> impl Iterator<Item = &str> {
        self.config_form
            .fields
            .iter()
            .filter(|field| field.control.as_deref() == Some("password"))
            .map(|field| field.key.as_str())
    }
}

fn validate_form(
    name: &str,
    form: &PluginFormSchema,
    schema: &Value,
    config: bool,
) -> ContractResult {
    if form.schema_version != "1.0.0" {
        return Err(invalid(&format!(
            "{name} has an unsupported schema_version"
        )));
    }
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(invalid(&format!("{name} requires an object schema")));
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    let mut fields = BTreeSet::new();
    for field in &form.fields {
        if field.key.trim().is_empty()
            || field.label.trim().is_empty()
            || !fields.insert(&field.key)
        {
            return Err(invalid(&format!("{name} requires unique labeled fields")));
        }
        let property = properties
            .and_then(|properties| properties.get(&field.key))
            .ok_or_else(|| invalid(&format!("{name} field is absent from schema properties")))?;
        let expected = match field.field_type.as_str() {
            "string" => "string",
            "number" => "number",
            "boolean" => "boolean",
            "select" => "",
            _ => {
                return Err(invalid(&format!(
                    "{name} field type has no schema form control"
                )))
            }
        };
        let property_type = property.get("type").and_then(Value::as_str);
        if (!expected.is_empty()
            && property_type != Some(expected)
            && !(expected == "number" && property_type == Some("integer")))
            || (field.control.as_deref() == Some("password")
                && (!config || field.field_type != "string"))
        {
            return Err(invalid(&format!("{name} field does not match schema type")));
        }
        let validator = compile_schema(name, property)?;
        let option_schema = if field.control.as_deref() == Some("multi-select") {
            if field.field_type != "select" || property_type != Some("array") {
                return Err(invalid(&format!("{name} multi-select requires an array")));
            }
            property
                .get("items")
                .ok_or_else(|| invalid(&format!("{name} multi-select requires items")))?
        } else {
            property
        };
        let option_validator = compile_schema(name, option_schema)?;
        if field
            .default_value
            .as_ref()
            .is_some_and(|value| !validator.is_valid(value))
            || field
                .options
                .iter()
                .any(|option| !option_validator.is_valid(&option.value))
        {
            return Err(invalid(&format!(
                "{name} default or option violates schema"
            )));
        }
    }
    Ok(())
}

fn compile_schema(name: &str, schema: &Value) -> ContractResult<jsonschema::Validator> {
    // Keep registration self-contained: no schema may resolve network/file references.
    fn self_contained(value: &Value) -> bool {
        match value {
            Value::Object(fields) => {
                !fields.contains_key("$ref")
                    && !fields.contains_key("$dynamicRef")
                    && fields.values().all(self_contained)
            }
            Value::Array(values) => values.iter().all(self_contained),
            _ => true,
        }
    }
    if !self_contained(schema) {
        return Err(invalid(&format!(
            "{name} must be a self-contained JSON Schema"
        )));
    }
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(schema)
        .map_err(|_| invalid(&format!("{name} is invalid JSON Schema")))
}

fn validate_value(name: &str, schema: &Value, value: &Value) -> ContractResult {
    if !compile_schema(name, schema)?.is_valid(value) {
        // Never expose provider input/config (including credentials) in an error message.
        return Err(invalid(&format!("value violates {name}")));
    }
    Ok(())
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        })
}

fn invalid(message: &str) -> NotificationContractError {
    NotificationContractError(message.to_owned())
}
