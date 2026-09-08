use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// The workspace is always the authorization record's workspace_id; request JSON cannot
/// select a second workspace through the resource scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContributionResourceScope {
    Workspace,
    OwnedCollection { collection_code: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionAuthorizationStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginContributionAuthorization {
    pub id: Uuid,
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub contribution_id: String,
    pub point_id: String,
    pub permission: String,
    pub resource_scope: ContributionResourceScope,
    pub permission_contract_id: String,
    pub permission_contract_version: String,
    pub status: ContributionAuthorizationStatus,
    pub granted_by: Uuid,
    pub revoked_by: Option<Uuid>,
    pub granted_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginContributionAuthoritySnapshot {
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub revision: i64,
    pub authorizations: Vec<PluginContributionAuthorization>,
}
