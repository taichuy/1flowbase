//! Plugin-owned outbound credentials. Trusted identity never appears in worker requests.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginCredentialRequest {
    Put { credential_id: String, value: Value },
    Get { credential_id: String },
    Delete { credential_id: String },
}
impl PluginCredentialRequest {
    pub fn credential_id(&self) -> &str {
        match self {
            Self::Put { credential_id, .. }
            | Self::Get { credential_id }
            | Self::Delete { credential_id } => credential_id,
        }
    }
    pub fn validate(&self) -> Result<(), PluginCredentialError> {
        let id = self.credential_id();
        if id.is_empty() || id.chars().any(char::is_control) {
            return Err(PluginCredentialError::new("plugin_credential_id_invalid"));
        }
        Ok(())
    }
}
// Deliberately no Debug: requests and responses can contain plaintext secrets.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCredentialResponse {
    pub value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCredentialError {
    pub code: String,
}
impl PluginCredentialError {
    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}
impl std::fmt::Display for PluginCredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}
impl std::error::Error for PluginCredentialError {}

/// Host-only binding from the admitted installation and execution scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCredentialBinding {
    pub installation_id: String,
    pub contribution_id: String,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub scope_id: String,
    pub deadline_unix_ms: i64,
}
pub type PluginCredentialFuture<'a> = Pin<
    Box<dyn Future<Output = Result<PluginCredentialResponse, PluginCredentialError>> + Send + 'a>,
>;
/// Implementations must validate live installation and secret authority before storage access.
pub trait PluginCredentialPort: Send + Sync {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginCredentialBinding,
        request: &'a PluginCredentialRequest,
    ) -> PluginCredentialFuture<'a>;
}
