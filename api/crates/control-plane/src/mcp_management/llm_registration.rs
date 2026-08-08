use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAX_PROVIDER_NAME_BYTES: usize = 64;
const MAX_PREFIX_BYTES: usize = 53;
const MAX_INLINE_CHARS: usize = 16_000;
const DEFAULT_INLINE_CHARS: usize = 4_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpLlmOperation {
    List,
    Get,
    Result,
    Call,
}

impl McpLlmOperation {
    pub const ALL: [Self; 4] = [Self::List, Self::Get, Self::Result, Self::Call];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Get => "get",
            Self::Result => "result",
            Self::Call => "call",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpLlmRegistrationSource {
    pub kind: String,
    pub key: String,
}

impl McpLlmRegistrationSource {
    pub fn new(kind: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            key: key.into(),
        }
    }

    fn identity(&self) -> String {
        format!("{}:{}", self.kind, self.key)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpLlmRegistration {
    pub instance_id: String,
    pub prefix: String,
    pub operation: McpLlmOperation,
    pub provider_name: String,
    pub source: McpLlmRegistrationSource,
    pub provider_tool: Value,
}

pub fn mcp_llm_instance_registration(instance_id: &str) -> Vec<McpLlmRegistration> {
    mcp_llm_registrations(&[(
        instance_id.to_string(),
        McpLlmRegistrationSource::new("instance", instance_id),
    )])
}

/// Projects every occurrence independently. Repeated instance selections are intentionally kept;
/// only their provider wire names are qualified so every registration remains addressable.
pub fn mcp_llm_registrations(
    occurrences: &[(String, McpLlmRegistrationSource)],
) -> Vec<McpLlmRegistration> {
    let mut base_name_counts = HashMap::<String, usize>::new();
    occurrences
        .iter()
        .flat_map(|(instance_id, source)| {
            let prefix = provider_prefix(instance_id);
            McpLlmOperation::ALL.map(|operation| {
                let base_name = format!("{prefix}_mcp_{}", operation.as_str());
                let occurrence = base_name_counts.entry(base_name.clone()).or_default();
                let provider_name = if *occurrence == 0 {
                    base_name
                } else {
                    qualified_provider_name(&prefix, operation, source, *occurrence)
                };
                *occurrence += 1;
                McpLlmRegistration {
                    instance_id: instance_id.clone(),
                    prefix: prefix.clone(),
                    operation,
                    provider_tool: provider_tool(&provider_name, operation),
                    provider_name,
                    source: source.clone(),
                }
            })
        })
        .collect()
}

fn provider_prefix(instance_id: &str) -> String {
    if !instance_id.is_empty()
        && instance_id.len() <= MAX_PREFIX_BYTES
        && instance_id.bytes().all(is_provider_name_byte)
    {
        return instance_id.to_string();
    }

    let mut slug = instance_id
        .bytes()
        .map(|byte| {
            if is_provider_name_byte(byte) {
                byte as char
            } else {
                '_'
            }
        })
        .collect::<String>();
    slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        slug = "mcp".to_string();
    }
    slug.truncate(40);
    format!("{slug}_{}", stable_hash(instance_id, 12))
}

fn qualified_provider_name(
    prefix: &str,
    operation: McpLlmOperation,
    source: &McpLlmRegistrationSource,
    occurrence: usize,
) -> String {
    let qualifier = stable_hash(&format!("{}:{occurrence}", source.identity()), 10);
    let suffix = format!("_{qualifier}_mcp_{}", operation.as_str());
    let keep = MAX_PROVIDER_NAME_BYTES.saturating_sub(suffix.len());
    format!("{}{suffix}", &prefix[..prefix.len().min(keep)])
}

fn stable_hash(value: &str, length: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest
        .iter()
        .flat_map(|byte| format!("{byte:02x}").chars().collect::<Vec<_>>())
        .take(length)
        .collect()
}

fn is_provider_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn provider_tool(name: &str, operation: McpLlmOperation) -> Value {
    let (description, parameters) = match operation {
        McpLlmOperation::List => (
            "Browse this MCP instance by path before requesting full tool details.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "minLength": 1},
                    "keywords": {"type": "array", "items": {"type": "string"}},
                    "depth": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1},
                    "path_regex": {"type": "string", "minLength": 1}
                },
                "additionalProperties": false
            }),
        ),
        McpLlmOperation::Get => (
            "Get the current description, schemas, risk information, and des_id for a visible tool in this MCP instance.",
            json!({
                "type": "object",
                "properties": {"tool_id": {"type": "string"}},
                "required": ["tool_id"],
                "additionalProperties": false
            }),
        ),
        McpLlmOperation::Result => (
            "Read a cached page of MCP result detail.",
            json!({
                "type": "object",
                "properties": {
                    "result_ref": {"type": "string", "format": "uuid"},
                    "cursor": {"type": "string"},
                    "max_inline_chars": {"type": "integer", "minimum": 1, "maximum": MAX_INLINE_CHARS}
                },
                "required": ["result_ref"],
                "additionalProperties": false
            }),
        ),
        McpLlmOperation::Call => (
            "Call a visible tool in this MCP instance after inspecting it.",
            json!({
                "type": "object",
                "properties": {
                    "tool_id": {"type": "string"},
                    "des_id": {"type": "string"},
                    "arguments": {"type": "object"},
                    "max_inline_chars": {"type": "integer", "minimum": 1, "maximum": MAX_INLINE_CHARS, "default": DEFAULT_INLINE_CHARS}
                },
                "required": ["tool_id", "arguments"],
                "additionalProperties": false
            }),
        ),
    };
    json!({"type": "function", "function": {
        "name": name,
        "description": description,
        "parameters": parameters,
    }})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_normal_instance_id_as_prefix() {
        let registrations = mcp_llm_instance_registration("1flowbase");
        assert_eq!(registrations[0].provider_name, "1flowbase_mcp_list");
        assert_eq!(registrations[3].provider_name, "1flowbase_mcp_call");
    }

    #[test]
    fn projects_invalid_and_long_ids_stably() {
        let invalid = mcp_llm_instance_registration("团队 MCP/生产");
        let repeated = mcp_llm_instance_registration("团队 MCP/生产");
        assert_eq!(invalid, repeated);
        assert!(invalid.iter().all(|item| item.provider_name.len() <= 64));
        assert!(invalid
            .iter()
            .all(|item| item.provider_name.bytes().all(is_provider_name_byte)));
    }

    #[test]
    fn keeps_repeated_occurrences_with_distinct_wire_names() {
        let registrations = mcp_llm_registrations(&[
            (
                "same".into(),
                McpLlmRegistrationSource::new("assistant", "run"),
            ),
            (
                "same".into(),
                McpLlmRegistrationSource::new("node", "llm-1"),
            ),
        ]);
        assert_eq!(registrations.len(), 8);
        assert_eq!(registrations[0].provider_name, "same_mcp_list");
        assert_ne!(
            registrations[0].provider_name,
            registrations[4].provider_name
        );
    }
}
