//! Stable plugin-owned record permissions, scoped to the caller workspace.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPermissionResourceKey {
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub resource_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPermissionKey {
    pub resource: RecordPermissionResourceKey,
    pub record_id: String,
    pub workspace_id: Uuid,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordPermissionMode {
    #[default]
    Inherit,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPermissionGrant {
    pub role_id: Uuid,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPermissionPolicy {
    pub mode: RecordPermissionMode,
    pub grants: Vec<RecordPermissionGrant>,
    pub revision: i64,
}

impl RecordPermissionPolicy {
    /// An additional restriction only; the caller must independently pass operation admission.
    pub fn allows(&self, action: &str, effective_role_ids: &[Uuid]) -> bool {
        self.mode == RecordPermissionMode::Inherit
            || self.grants.iter().any(|grant| {
                effective_role_ids.contains(&grant.role_id)
                    && grant.actions.iter().any(|candidate| candidate == action)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplaceRecordPermissions {
    pub mode: RecordPermissionMode,
    pub grants: Vec<RecordPermissionGrant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPermissionRole {
    pub role_id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPermissionCapabilities {
    pub record_id: String,
    pub allowed_actions: Vec<String>,
    pub can_manage_permissions: bool,
}

#[cfg(test)]
#[path = "../_tests/record_permissions.rs"]
mod tests;
