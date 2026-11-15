//! Plugin-owned ephemeral JSON state. Trusted identity never appears in worker requests.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginEphemeralRequest {
    Put {
        key: String,
        value: Value,
        ttl_ms: u64,
    },
    Get {
        key: String,
    },
    Delete {
        key: String,
    },
}
impl PluginEphemeralRequest {
    pub fn key(&self) -> &str {
        match self {
            Self::Put { key, .. } | Self::Get { key } | Self::Delete { key } => key,
        }
    }
    pub fn validate(&self) -> Result<(), PluginEphemeralError> {
        let key = self.key();
        if key.is_empty() || key.chars().any(char::is_control) {
            return Err(PluginEphemeralError::new("plugin_ephemeral_key_invalid"));
        }
        if matches!(self, Self::Put { ttl_ms: 0, .. }) {
            return Err(PluginEphemeralError::new("plugin_ephemeral_ttl_invalid"));
        }
        Ok(())
    }
}
// Deliberately no Debug: requests and responses can contain plaintext secrets.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEphemeralResponse {
    // Omission means missing; an explicit JSON null remains a stored JSON value.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub value: Option<Value>,
}

fn present_value<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEphemeralError {
    pub code: String,
}
impl PluginEphemeralError {
    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}
impl std::fmt::Display for PluginEphemeralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}
impl std::error::Error for PluginEphemeralError {}

/// Host-only binding from the admitted installation and execution scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginEphemeralBinding {
    pub installation_id: String,
    pub contribution_id: String,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub scope_id: String,
    pub deadline_unix_ms: i64,
}
pub type PluginEphemeralFuture<'a> = Pin<
    Box<dyn Future<Output = Result<PluginEphemeralResponse, PluginEphemeralError>> + Send + 'a>,
>;
/// Implementations must validate live installation and secret authority before storage access.
pub trait PluginEphemeralPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginEphemeralBinding,
        request: &'a PluginEphemeralRequest,
    ) -> PluginEphemeralFuture<'a>;
}
