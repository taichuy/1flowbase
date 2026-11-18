//! Call-owned scheduling for opaque resource groups; no worker-declared identity.
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginExecutionGateRequest {
    Acquire { key: String },
    Release { key: String, token: String },
}
impl PluginExecutionGateRequest {
    pub fn validate(&self) -> Result<(), PluginExecutionGateError> {
        let key = match self {
            Self::Acquire { key } | Self::Release { key, .. } => key,
        };
        if key.is_empty() || key.chars().any(char::is_control) {
            return Err(PluginExecutionGateError::new("execution_gate_key_invalid"));
        }
        if matches!(self, Self::Release { token, .. } if token.is_empty()) {
            return Err(PluginExecutionGateError::new(
                "execution_gate_token_invalid",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginExecutionGateResponse {
    Acquire { token: String },
    Release,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginExecutionGateError {
    pub code: String,
}
impl PluginExecutionGateError {
    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}
impl std::fmt::Display for PluginExecutionGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}
impl std::error::Error for PluginExecutionGateError {}
/// Host-only captured installation authority plus the actual execution workspace.
#[derive(Debug, Clone)]
pub struct PluginExecutionGateBinding {
    pub subject: crate::extension_bus::ManagedContributionSubject,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub workspace_id: String,
    pub deadline_unix_ms: i64,
}
pub type PluginExecutionGateAuthorityFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), PluginExecutionGateError>> + Send + 'a>>;
/// Only checks live authority. Scheduling and permit ownership belong to RuntimeExtensionHost.
pub trait PluginExecutionGateAuthority: Send + Sync {
    fn authorize<'a>(
        &'a self,
        binding: &'a PluginExecutionGateBinding,
    ) -> PluginExecutionGateAuthorityFuture<'a>;
}
