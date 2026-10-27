//! Declarative projections of public business messages, never runtime envelopes or secrets.
use super::{invalid, ContractResult, NotificationTypeDescriptor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationLogDescriptor {
    /// RFC 6901 pointer into the validated business input. Missing/blank titles use content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// RFC 6901 pointer to the complete text in the validated business input.
    pub content: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, NotificationLogExtraField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationLogExtraField {
    pub source: NotificationLogSource,
    pub pointer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLogSource {
    Input,
    Result,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationLogSnapshot {
    pub title: String,
    pub content: String,
    pub extra: Value,
}

impl NotificationLogDescriptor {
    pub(super) fn validate(&self, descriptor: &NotificationTypeDescriptor) -> ContractResult {
        for pointer in self.title.iter().chain(std::iter::once(&self.content)) {
            let field = schema_field(&descriptor.input_schema, pointer)?;
            if field.get("type").and_then(Value::as_str) != Some("string") {
                return Err(invalid(
                    "log title and content must reference string input fields",
                ));
            }
        }
        for (name, mapping) in &self.extra {
            if name.trim().is_empty() {
                return Err(invalid("log extra keys must be nonempty"));
            }
            let schema = match mapping.source {
                NotificationLogSource::Input => &descriptor.input_schema,
                NotificationLogSource::Result => &descriptor.output_schema,
            };
            schema_field(schema, &mapping.pointer)?;
        }
        Ok(())
    }

    /// Called only after input validation. No raw request or provider envelope is accepted.
    pub fn project_input(&self, input: &Value) -> ContractResult<NotificationLogSnapshot> {
        let content = input
            .pointer(&self.content)
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("log content is missing or is not text"))?;
        let title = match self
            .title
            .as_ref()
            .and_then(|pointer| input.pointer(pointer))
        {
            Some(Value::String(title)) if !title.trim().is_empty() => title.clone(),
            None | Some(Value::Null) | Some(Value::String(_)) => {
                content.chars().take(255).collect()
            }
            Some(_) => return Err(invalid("log title is not text")),
        };
        Ok(NotificationLogSnapshot {
            title,
            content: content.to_owned(),
            extra: self.project_extra(NotificationLogSource::Input, input),
        })
    }

    /// Only a schema-validated, credential-checked business result may reach this projection.
    pub fn project_result(&self, result: &Value) -> Value {
        self.project_extra(NotificationLogSource::Result, result)
    }

    fn project_extra(&self, source: NotificationLogSource, value: &Value) -> Value {
        let mut extra = Map::new();
        for (name, field) in &self.extra {
            if field.source == source {
                if let Some(value) = value.pointer(&field.pointer) {
                    extra.insert(name.clone(), value.clone());
                }
            }
        }
        Value::Object(extra)
    }
}

fn schema_field<'a>(schema: &'a Value, pointer: &str) -> ContractResult<&'a Value> {
    // Empty is the whole document in RFC 6901; logging requires an explicit named field.
    if !pointer.starts_with('/') {
        return Err(invalid("log mappings require non-root JSON Pointers"));
    }
    if schema.get("writeOnly") == Some(&Value::Bool(true)) {
        return Err(invalid("log mappings cannot expose write-only fields"));
    }
    let mut field = schema;
    for raw in pointer[1..].split('/') {
        let mut token = String::new();
        let mut chars = raw.chars();
        while let Some(ch) = chars.next() {
            token.push(if ch == '~' {
                match chars.next() {
                    Some('0') => '~',
                    Some('1') => '/',
                    _ => return Err(invalid("log mapping has an invalid JSON Pointer escape")),
                }
            } else {
                ch
            });
        }
        field = match field.get("type").and_then(Value::as_str) {
            Some("object") => field
                .get("properties")
                .and_then(|fields| fields.get(&token)),
            Some("array")
                if token == "0"
                    || (!token.starts_with('0')
                        && token.bytes().all(|byte| byte.is_ascii_digit())
                        && token.parse::<usize>().is_ok()) =>
            {
                field.get("items")
            }
            _ => None,
        }
        .ok_or_else(|| invalid("log mapping must reference a declared schema field"))?;
        if field.get("writeOnly") == Some(&json!(true)) {
            return Err(invalid("log mappings cannot expose write-only fields"));
        }
    }
    // Object/array extra values must not expose a nested write-only field either.
    fn has_private_field(value: &Value) -> bool {
        match value {
            Value::Object(fields) => {
                fields.get("writeOnly") == Some(&Value::Bool(true))
                    || fields.values().any(has_private_field)
            }
            Value::Array(values) => values.iter().any(has_private_field),
            _ => false,
        }
    }
    if has_private_field(field) {
        return Err(invalid("log mappings cannot expose write-only fields"));
    }
    Ok(field)
}
