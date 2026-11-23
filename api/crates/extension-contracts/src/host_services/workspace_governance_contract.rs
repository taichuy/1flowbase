//! Workspace lifecycle operations; execution identity is bound by the host, not the wire.
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

pub const WORKSPACE_GOVERNANCE_READ_PERMISSION: &str = "workspace.governance.read";
pub const WORKSPACE_GOVERNANCE_MANAGE_PERMISSION: &str = "workspace.governance.manage";
pub const WORKSPACE_GOVERNANCE_CONTRACT_ID: &str = "workspace-governance";
pub const WORKSPACE_GOVERNANCE_CONTRACT_V1: &str = "1";

#[derive(Debug, Clone)]
pub struct WorkspaceGovernanceBinding {
    pub subject: crate::extension_bus::ManagedContributionSubject,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub can_read: bool,
    pub can_manage: bool,
    pub deadline_unix_ms: i64,
    pub caller: Option<WorkspaceGovernanceCallerDto>,
}

// Deliberately not Debug: these variants may carry a plaintext password.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceGovernanceRequest {
    #[serde(deserialize_with = "crate::workspace_policy_contract::deserialize_empty_variant")]
    ListTenantsAndWorkspaces,
    #[serde(deserialize_with = "crate::workspace_policy_contract::deserialize_empty_variant")]
    InspectCallerContext,
    InspectQuotaCounts {
        tenant_id: String,
        workspace_id: String,
    },
    LookupUser {
        by: WorkspaceGovernanceUserQueryDto,
    },
    ProvisionWorkspace {
        tenant: WorkspaceGovernanceTenantTargetDto,
        workspace_name: String,
        logo_url: Option<String>,
        introduction: String,
        initial_admin: Option<WorkspaceGovernanceInitialMemberDto>,
        actor_user_id: Option<String>,
    },
    GrantWorkspaceMembership {
        workspace_id: String,
        member: WorkspaceGovernanceInitialMemberDto,
        role_codes: Vec<String>,
        actor_user_id: Option<String>,
    },
    SetWorkspaceStatus {
        workspace_id: String,
        status: String,
        actor_user_id: Option<String>,
    },
    AssignWorkspaceAdmins {
        workspace_ids: Vec<String>,
        member: WorkspaceGovernanceInitialMemberDto,
        actor_user_id: Option<String>,
    },
    RevokeWorkspaceAdmin {
        workspace_id: String,
        user_id: String,
        actor_user_id: Option<String>,
    },
    SetTenantStatus {
        tenant_id: String,
        status: String,
        actor_user_id: Option<String>,
    },
}

impl WorkspaceGovernanceRequest {
    pub fn requires_manage(&self) -> bool {
        matches!(
            self,
            Self::ProvisionWorkspace { .. }
                | Self::GrantWorkspaceMembership { .. }
                | Self::SetTenantStatus { .. }
                | Self::SetWorkspaceStatus { .. }
                | Self::AssignWorkspaceAdmins { .. }
                | Self::RevokeWorkspaceAdmin { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceGovernanceResponse {
    CallerContext {
        caller: Option<WorkspaceGovernanceCallerDto>,
    },
    TenantsAndWorkspaces {
        items: Vec<WorkspaceGovernanceTenantDto>,
    },
    QuotaCounts {
        active_member_count: i64,
        tenant_workspace_count: i64,
        workspace_application_count: i64,
    },
    UserLookup {
        user: Option<WorkspaceGovernanceUserDto>,
    },
    WorkspaceProvisioned {
        tenant: WorkspaceGovernanceTenantDto,
        workspace: WorkspaceGovernanceWorkspaceDto,
        initial_member_user_id: Option<String>,
    },
    MembershipGranted {
        user_id: String,
        workspace_id: String,
        already_active_member: bool,
    },
    WorkspaceStatusUpdated {
        workspace: WorkspaceGovernanceWorkspaceDto,
        status: String,
    },
    WorkspaceAdminsAssigned {
        user: WorkspaceGovernanceUserDto,
        workspace_ids: Vec<String>,
    },
    WorkspaceAdminRevoked {
        workspace_id: String,
        user_id: String,
        revoked: bool,
    },
    TenantUpdated {
        tenant: WorkspaceGovernanceTenantDto,
    },
}

/// A read-only projection of the host-authenticated caller, not worker input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGovernanceCallerDto {
    pub user_id: String,
    pub tenant_id: String,
    pub current_workspace_id: String,
    pub is_root: bool,
    pub effective_display_role: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceGovernanceUserQueryDto {
    UserId { user_id: String },
    Account { account: String },
    Email { email: String },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceGovernanceTenantTargetDto {
    New { code: String, name: String },
    Existing { tenant_id: String },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceGovernanceInitialMemberDto {
    Existing {
        user_id: String,
    },
    NewPasswordUser {
        account: String,
        email: String,
        password: String,
        name: String,
        nickname: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGovernanceTenantDto {
    pub id: String,
    pub code: String,
    pub name: String,
    pub is_root: bool,
    pub is_hidden: bool,
    pub status: String,
    pub cell_id: String,
    pub workspaces: Vec<WorkspaceGovernanceWorkspaceSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGovernanceWorkspaceDto {
    #[serde(default = "active_workspace_status")]
    pub status: String,
    #[serde(default)]
    pub is_root_workspace: bool,
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGovernanceWorkspaceSummaryDto {
    pub workspace: WorkspaceGovernanceWorkspaceDto,
    pub active_member_count: i64,
    pub workspace_application_count: i64,
    pub admins: Vec<WorkspaceGovernanceUserDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceGovernanceUserDto {
    pub user_id: String,
    pub account: String,
    pub email: String,
    pub name: String,
    pub status: String,
    pub managed_by_tenant_id: Option<String>,
}

fn active_workspace_status() -> String {
    "active".into()
}

pub type WorkspaceGovernanceFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<WorkspaceGovernanceResponse, crate::HostServiceError>>
            + Send
            + 'a,
    >,
>;

pub trait WorkspaceGovernancePort: Send + Sync {
    fn execute<'a>(
        &'a self,
        binding: &'a WorkspaceGovernanceBinding,
        request: &'a WorkspaceGovernanceRequest,
    ) -> WorkspaceGovernanceFuture<'a>;
}

#[cfg(test)]
#[path = "../_tests/managed_contributions/workspace_governance_contract_tests.rs"]
mod tests;
