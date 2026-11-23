use anyhow::Result;
use async_trait::async_trait;
use domain::{
    RoleTemplate, TenantRecord, TenantStatus, UserStatus, VerifiedExternalIdentity, WorkspaceRecord,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ProvisionWorkspaceTenant {
    New { code: String, name: String },
    Existing { tenant_id: Uuid },
}

#[derive(Debug, Clone)]
pub struct ProvisionWorkspaceInput {
    pub tenant: ProvisionWorkspaceTenant,
    pub workspace_name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
    pub actor_user_id: Option<Uuid>,
    /// Pass the workspace-scoped templates from the host's bootstrap role catalog.
    pub role_templates: Vec<RoleTemplate>,
    pub initial_member: Option<ProvisionWorkspaceMember>,
}

#[derive(Debug, Clone)]
pub struct ProvisionWorkspaceMember {
    pub user: ProvisionWorkspaceUser,
    pub role_codes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ProvisionWorkspaceUser {
    Existing { user_id: Uuid },
    New(NewGovernanceUser),
}

#[derive(Debug, Clone)]
pub struct NewGovernanceUser {
    pub account: String,
    pub email: String,
    pub name: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
    pub credential: GovernanceUserCredential,
}

#[derive(Clone)]
pub enum GovernanceUserCredential {
    Password { password_hash: String },
    External(VerifiedExternalIdentity),
}

impl std::fmt::Debug for GovernanceUserCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Password { .. } => "Password([redacted])",
            Self::External(_) => "External([redacted])",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedWorkspaceOutput {
    pub tenant: TenantRecord,
    pub workspace: WorkspaceRecord,
    pub is_root_workspace: bool,
    pub initial_member_user_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct GrantWorkspaceMembershipInput {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub role_codes: Vec<String>,
    pub actor_user_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantWorkspaceMembershipOutput {
    pub already_active_member: bool,
}

#[derive(Debug, Clone)]
pub struct OnboardNewUserInput {
    pub workspace_id: Uuid,
    pub user: NewGovernanceUser,
    pub role_codes: Vec<String>,
    pub actor_user_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardedUserOutput {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantWorkspaceGovernanceSummary {
    pub tenant: TenantRecord,
    pub workspaces: Vec<WorkspaceGovernanceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceWorkspace {
    pub record: WorkspaceRecord,
    pub status: TenantStatus,
    pub is_root_workspace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGovernanceSummary {
    pub workspace: GovernanceWorkspace,
    pub active_member_count: i64,
    pub workspace_application_count: i64,
    pub admins: Vec<GovernanceUserSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceQuotaCounts {
    pub active_member_count: i64,
    pub tenant_workspace_count: i64,
    pub workspace_application_count: i64,
}

#[derive(Debug, Clone)]
pub enum GovernanceUserLookup {
    UserId(Uuid),
    Account(String),
    Email(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GovernanceUserSummary {
    pub user_id: Uuid,
    pub account: String,
    pub email: String,
    pub name: String,
    pub status: UserStatus,
    pub managed_by_tenant_id: Option<Uuid>,
}

/// Transaction boundaries for trusted host-side workspace policy. The caller
/// authorizes the governance operation; the adapter enforces persisted invariants.
#[async_trait]
pub trait WorkspaceGovernanceRepository: Send + Sync {
    async fn provision_workspace(
        &self,
        input: &ProvisionWorkspaceInput,
    ) -> Result<ProvisionedWorkspaceOutput>;
    async fn grant_or_activate_workspace_membership(
        &self,
        input: &GrantWorkspaceMembershipInput,
    ) -> Result<GrantWorkspaceMembershipOutput>;
    async fn onboard_new_user_to_workspace(
        &self,
        input: &OnboardNewUserInput,
    ) -> Result<OnboardedUserOutput>;
    async fn set_workspace_status(
        &self,
        workspace_id: Uuid,
        status: TenantStatus,
        actor_user_id: Option<Uuid>,
    ) -> Result<GovernanceWorkspace>;
    async fn assign_workspace_admins(
        &self,
        workspace_ids: &[Uuid],
        user: &ProvisionWorkspaceUser,
        actor_user_id: Option<Uuid>,
    ) -> Result<GovernanceUserSummary>;
    async fn revoke_workspace_admin(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        actor_user_id: Option<Uuid>,
    ) -> Result<bool>;
    async fn set_tenant_status(
        &self,
        tenant_id: Uuid,
        status: TenantStatus,
        actor_user_id: Option<Uuid>,
    ) -> Result<TenantRecord>;
    async fn list_tenants_and_workspaces(&self) -> Result<Vec<TenantWorkspaceGovernanceSummary>>;
    async fn inspect_workspace_quota_counts(
        &self,
        tenant_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<WorkspaceQuotaCounts>;
    async fn lookup_governance_user(
        &self,
        query: &GovernanceUserLookup,
    ) -> Result<Option<GovernanceUserSummary>>;
}
