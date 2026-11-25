use crate::{MultiplexEmitter, MultiplexError, MultiplexHostService};
use extension_contracts::*;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceGovernanceClientError {
    #[error(transparent)]
    Transport(#[from] MultiplexError),
    #[error(transparent)]
    Host(#[from] HostServiceError),
    #[error("invalid workspace governance response")]
    InvalidResponse,
}

#[derive(Clone)]
pub struct WorkspaceGovernanceClient(MultiplexEmitter);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    result: Option<WorkspaceGovernanceResponse>,
    error: Option<HostServiceError>,
}

type Result<T> = std::result::Result<T, WorkspaceGovernanceClientError>;

impl WorkspaceGovernanceClient {
    pub fn new(emitter: MultiplexEmitter) -> Self {
        Self(emitter)
    }

    pub async fn execute(
        &self,
        request: WorkspaceGovernanceRequest,
    ) -> Result<WorkspaceGovernanceResponse> {
        let request = serde_json::to_value(request)
            .map_err(|_| WorkspaceGovernanceClientError::InvalidResponse)?;
        let envelope = self
            .0
            .callback(MultiplexHostService::WorkspaceGovernanceV1, request)
            .await?;
        let envelope: Envelope = serde_json::from_value(envelope)
            .map_err(|_| WorkspaceGovernanceClientError::InvalidResponse)?;
        match (envelope.result, envelope.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error.into()),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn inspect_caller_context(&self) -> Result<Option<WorkspaceGovernanceCallerDto>> {
        match self
            .execute(WorkspaceGovernanceRequest::InspectCallerContext)
            .await?
        {
            WorkspaceGovernanceResponse::CallerContext { caller } => Ok(caller),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn list_tenants_and_workspaces(
        &self,
    ) -> Result<Vec<WorkspaceGovernanceTenantDto>> {
        match self
            .execute(WorkspaceGovernanceRequest::ListTenantsAndWorkspaces)
            .await?
        {
            WorkspaceGovernanceResponse::TenantsAndWorkspaces { items } => Ok(items),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn inspect_quota_counts(
        &self,
        tenant_id: String,
        workspace_id: String,
    ) -> Result<WorkspaceGovernanceQuotaCounts> {
        match self
            .execute(WorkspaceGovernanceRequest::InspectQuotaCounts {
                tenant_id,
                workspace_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::QuotaCounts {
                active_member_count,
                tenant_workspace_count,
                workspace_application_count,
            } => Ok(WorkspaceGovernanceQuotaCounts {
                active_member_count,
                tenant_workspace_count,
                workspace_application_count,
            }),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn lookup_user(
        &self,
        by: WorkspaceGovernanceUserQueryDto,
    ) -> Result<Option<WorkspaceGovernanceUserDto>> {
        match self
            .execute(WorkspaceGovernanceRequest::LookupUser { by })
            .await?
        {
            WorkspaceGovernanceResponse::UserLookup { user } => Ok(user),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn search_users(
        &self,
        query: Option<String>,
        limit: u32,
    ) -> Result<Vec<WorkspaceGovernanceUserDto>> {
        match self
            .execute(WorkspaceGovernanceRequest::SearchUsers { query, limit })
            .await?
        {
            WorkspaceGovernanceResponse::UsersSearched { items } => Ok(items),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn provision_workspace(
        &self,
        input: ProvisionWorkspace,
    ) -> Result<ProvisionedWorkspace> {
        match self
            .execute(WorkspaceGovernanceRequest::ProvisionWorkspace {
                tenant: input.tenant,
                workspace_name: input.workspace_name,
                logo_url: input.logo_url,
                introduction: input.introduction,
                initial_admin: input.initial_admin,
                actor_user_id: input.actor_user_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::WorkspaceProvisioned {
                tenant,
                workspace,
                initial_member_user_id,
            } => Ok(ProvisionedWorkspace {
                tenant,
                workspace,
                initial_member_user_id,
            }),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn grant_workspace_membership(
        &self,
        workspace_id: String,
        member: WorkspaceGovernanceInitialMemberDto,
        role_codes: Vec<String>,
        actor_user_id: Option<String>,
    ) -> Result<GrantedWorkspaceMembership> {
        match self
            .execute(WorkspaceGovernanceRequest::GrantWorkspaceMembership {
                workspace_id,
                member,
                role_codes,
                actor_user_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::MembershipGranted {
                user_id,
                workspace_id,
                already_active_member,
            } => Ok(GrantedWorkspaceMembership {
                user_id,
                workspace_id,
                already_active_member,
            }),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn set_workspace_status(
        &self,
        workspace_id: String,
        status: String,
        actor_user_id: Option<String>,
    ) -> Result<WorkspaceGovernanceWorkspaceDto> {
        match self
            .execute(WorkspaceGovernanceRequest::SetWorkspaceStatus {
                workspace_id,
                status,
                actor_user_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::WorkspaceStatusUpdated { workspace, .. } => Ok(workspace),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn assign_workspace_admins(
        &self,
        workspace_ids: Vec<String>,
        member: WorkspaceGovernanceInitialMemberDto,
        actor_user_id: Option<String>,
    ) -> Result<(WorkspaceGovernanceUserDto, Vec<String>)> {
        match self
            .execute(WorkspaceGovernanceRequest::AssignWorkspaceAdmins {
                workspace_ids,
                member,
                actor_user_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::WorkspaceAdminsAssigned {
                user,
                workspace_ids,
            } => Ok((user, workspace_ids)),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn revoke_workspace_admin(
        &self,
        workspace_id: String,
        user_id: String,
        actor_user_id: Option<String>,
    ) -> Result<bool> {
        match self
            .execute(WorkspaceGovernanceRequest::RevokeWorkspaceAdmin {
                workspace_id,
                user_id,
                actor_user_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::WorkspaceAdminRevoked { revoked, .. } => Ok(revoked),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }

    pub async fn set_tenant_status(
        &self,
        tenant_id: String,
        status: String,
        actor_user_id: Option<String>,
    ) -> Result<WorkspaceGovernanceTenantDto> {
        match self
            .execute(WorkspaceGovernanceRequest::SetTenantStatus {
                tenant_id,
                status,
                actor_user_id,
            })
            .await?
        {
            WorkspaceGovernanceResponse::TenantUpdated { tenant } => Ok(tenant),
            _ => Err(WorkspaceGovernanceClientError::InvalidResponse),
        }
    }
}

pub struct ProvisionWorkspace {
    pub tenant: WorkspaceGovernanceTenantTargetDto,
    pub workspace_name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
    pub initial_admin: Option<WorkspaceGovernanceInitialMemberDto>,
    pub actor_user_id: Option<String>,
}

pub struct ProvisionedWorkspace {
    pub tenant: WorkspaceGovernanceTenantDto,
    pub workspace: WorkspaceGovernanceWorkspaceDto,
    pub initial_member_user_id: Option<String>,
}

pub struct GrantedWorkspaceMembership {
    pub user_id: String,
    pub workspace_id: String,
    pub already_active_member: bool,
}

pub struct WorkspaceGovernanceQuotaCounts {
    pub active_member_count: i64,
    pub tenant_workspace_count: i64,
    pub workspace_application_count: i64,
}

#[cfg(test)]
#[path = "../_tests/host_services/workspace_governance_sdk_tests.rs"]
mod tests;
