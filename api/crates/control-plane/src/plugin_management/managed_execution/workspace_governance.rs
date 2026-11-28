use super::HostContributionGrantPolicy;
use crate::{auth::governance_password_user, errors::ControlPlaneError};
use control_plane_contracts::ports::*;
use extension_contracts::*;
use std::sync::Arc;
use uuid::Uuid;

pub struct ManagedWorkspaceGovernanceService {
    authority: Arc<dyn PluginContributionAuthorityRepository>,
    repository: Arc<dyn WorkspaceGovernanceRepository>,
    navigation_cache: Option<crate::navigation_cache::NavigationCache>,
}

impl ManagedWorkspaceGovernanceService {
    pub fn new(
        authority: Arc<dyn PluginContributionAuthorityRepository>,
        repository: Arc<dyn WorkspaceGovernanceRepository>,
    ) -> Self {
        Self {
            authority,
            repository,
            navigation_cache: None,
        }
    }

    pub fn with_navigation_cache(
        mut self,
        cache: crate::navigation_cache::NavigationCache,
    ) -> Self {
        self.navigation_cache = Some(cache);
        self
    }

    async fn invalidate_console_routes(&self, workspace_id: Uuid) {
        if let Some(cache) = &self.navigation_cache {
            cache
                .invalidate(
                    crate::navigation_cache::NavigationCacheDomain::ConsoleRoutes,
                    workspace_id,
                )
                .await;
        }
    }

    async fn dispatch(
        &self,
        binding: &WorkspaceGovernanceBinding,
        request: &WorkspaceGovernanceRequest,
    ) -> anyhow::Result<WorkspaceGovernanceResponse> {
        use WorkspaceGovernanceRequest as Request;
        use WorkspaceGovernanceResponse as Response;
        let repo = &self.repository;
        Ok(match request {
            Request::InspectCallerContext => Response::CallerContext {
                caller: binding.caller.clone(),
            },
            Request::ListTenantsAndWorkspaces => Response::TenantsAndWorkspaces {
                items: repo
                    .list_tenants_and_workspaces()
                    .await?
                    .into_iter()
                    .map(|item| {
                        let mut tenant = tenant_dto(item.tenant);
                        tenant.workspaces = item
                            .workspaces
                            .into_iter()
                            .map(|item| WorkspaceGovernanceWorkspaceSummaryDto {
                                workspace: workspace_dto(item.workspace),
                                active_member_count: item.active_member_count,
                                workspace_application_count: item.workspace_application_count,
                                admins: item.admins.into_iter().map(user_dto).collect(),
                            })
                            .collect();
                        tenant
                    })
                    .collect(),
            },
            Request::InspectQuotaCounts {
                tenant_id,
                workspace_id,
            } => {
                let counts = repo
                    .inspect_workspace_quota_counts(id(tenant_id)?, id(workspace_id)?)
                    .await?;
                Response::QuotaCounts {
                    active_member_count: counts.active_member_count,
                    tenant_workspace_count: counts.tenant_workspace_count,
                    workspace_application_count: counts.workspace_application_count,
                }
            }
            Request::LookupUser { by } => {
                let query = match by {
                    WorkspaceGovernanceUserQueryDto::UserId { user_id } => {
                        GovernanceUserLookup::UserId(id(user_id)?)
                    }
                    WorkspaceGovernanceUserQueryDto::Account { account } => {
                        GovernanceUserLookup::Account(account.clone())
                    }
                    WorkspaceGovernanceUserQueryDto::Email { email } => {
                        GovernanceUserLookup::Email(email.clone())
                    }
                };
                Response::UserLookup {
                    user: repo.lookup_governance_user(&query).await?.map(user_dto),
                }
            }
            Request::SearchUsers { query, limit } => Response::UsersSearched {
                items: repo
                    .search_governance_users(query.as_deref(), *limit)
                    .await?
                    .into_iter()
                    .map(user_dto)
                    .collect(),
            },
            Request::ProvisionWorkspace {
                tenant,
                workspace_name,
                logo_url,
                introduction,
                initial_admin,
                actor_user_id,
            } => {
                let tenant = match tenant {
                    WorkspaceGovernanceTenantTargetDto::New { code, name } => {
                        ProvisionWorkspaceTenant::New {
                            code: code.clone(),
                            name: name.clone(),
                        }
                    }
                    WorkspaceGovernanceTenantTargetDto::Existing { tenant_id } => {
                        ProvisionWorkspaceTenant::Existing {
                            tenant_id: id(tenant_id)?,
                        }
                    }
                };
                let initial_member = initial_admin
                    .as_ref()
                    .map(|member| {
                        Ok::<_, anyhow::Error>(ProvisionWorkspaceMember {
                            user: provision_user(member)?,
                            role_codes: vec!["admin".into()],
                        })
                    })
                    .transpose()?;
                let output = repo
                    .provision_workspace(&ProvisionWorkspaceInput {
                        tenant,
                        workspace_name: workspace_name.clone(),
                        logo_url: logo_url.clone(),
                        introduction: introduction.clone(),
                        actor_user_id: actor_user_id.as_deref().map(id).transpose()?,
                        role_templates: crate::auth::workspace_role_templates(),
                        initial_member,
                    })
                    .await?;
                Response::WorkspaceProvisioned {
                    tenant: tenant_dto(output.tenant),
                    workspace: workspace_dto(GovernanceWorkspace {
                        record: output.workspace,
                        status: domain::TenantStatus::Active,
                        is_root_workspace: output.is_root_workspace,
                    }),
                    initial_member_user_id: output.initial_member_user_id.map(|id| id.to_string()),
                }
            }
            Request::GrantWorkspaceMembership {
                workspace_id,
                member,
                role_codes,
                actor_user_id,
            } => {
                let workspace_id = id(workspace_id)?;
                let actor_user_id = actor_user_id.as_deref().map(id).transpose()?;
                let (user_id, already_active_member) = match provision_user(member)? {
                    ProvisionWorkspaceUser::Existing { user_id } => {
                        let result = repo
                            .grant_or_activate_workspace_membership(
                                &GrantWorkspaceMembershipInput {
                                    workspace_id,
                                    user_id,
                                    role_codes: role_codes.clone(),
                                    role_templates: crate::auth::workspace_role_templates(),
                                    actor_user_id,
                                },
                            )
                            .await?;
                        (user_id, result.already_active_member)
                    }
                    ProvisionWorkspaceUser::New(user) => {
                        let result = repo
                            .onboard_new_user_to_workspace(&OnboardNewUserInput {
                                workspace_id,
                                user: *user,
                                role_codes: role_codes.clone(),
                                role_templates: crate::auth::workspace_role_templates(),
                                actor_user_id,
                            })
                            .await?;
                        (result.user_id, false)
                    }
                };
                self.invalidate_console_routes(workspace_id).await;
                Response::MembershipGranted {
                    user_id: user_id.to_string(),
                    workspace_id: workspace_id.to_string(),
                    already_active_member,
                }
            }
            Request::SetWorkspaceStatus {
                workspace_id,
                status,
                actor_user_id,
            } => {
                let status = domain::TenantStatus::from_db(status)
                    .ok_or(ControlPlaneError::InvalidInput("workspace_status"))?;
                let workspace = repo
                    .set_workspace_status(
                        id(workspace_id)?,
                        status,
                        authenticated_actor(binding, actor_user_id.as_deref())?,
                    )
                    .await?;
                Response::WorkspaceStatusUpdated {
                    workspace: workspace_dto(workspace),
                    status: status.as_str().into(),
                }
            }
            Request::AssignWorkspaceAdmins {
                workspace_ids,
                member,
                actor_user_id,
            } => {
                let workspace_ids = workspace_ids
                    .iter()
                    .map(|value| id(value))
                    .collect::<anyhow::Result<std::collections::BTreeSet<_>>>()?
                    .into_iter()
                    .collect::<Vec<_>>();
                let role_templates = crate::auth::workspace_role_templates();
                let user = repo
                    .assign_workspace_admins(
                        &workspace_ids,
                        &provision_user(member)?,
                        &role_templates,
                        authenticated_actor(binding, actor_user_id.as_deref())?,
                    )
                    .await?;
                for workspace_id in &workspace_ids {
                    self.invalidate_console_routes(*workspace_id).await;
                }
                Response::WorkspaceAdminsAssigned {
                    user: user_dto(user),
                    workspace_ids: workspace_ids.into_iter().map(|id| id.to_string()).collect(),
                }
            }
            Request::RevokeWorkspaceAdmin {
                workspace_id,
                user_id,
                actor_user_id,
            } => {
                let workspace_id = id(workspace_id)?;
                let user_id = id(user_id)?;
                let role_templates = crate::auth::workspace_role_templates();
                let revoked = repo
                    .revoke_workspace_admin(
                        workspace_id,
                        user_id,
                        &role_templates,
                        authenticated_actor(binding, actor_user_id.as_deref())?,
                    )
                    .await?;
                self.invalidate_console_routes(workspace_id).await;
                Response::WorkspaceAdminRevoked {
                    workspace_id: workspace_id.to_string(),
                    user_id: user_id.to_string(),
                    revoked,
                }
            }
            Request::SetTenantStatus {
                tenant_id,
                status,
                actor_user_id,
            } => {
                let status = domain::TenantStatus::from_db(status)
                    .ok_or(ControlPlaneError::InvalidInput("tenant_status"))?;
                Response::TenantUpdated {
                    tenant: tenant_dto(
                        repo.set_tenant_status(
                            id(tenant_id)?,
                            status,
                            actor_user_id.as_deref().map(id).transpose()?,
                        )
                        .await?,
                    ),
                }
            }
        })
    }
}

impl WorkspaceGovernancePort for ManagedWorkspaceGovernanceService {
    fn execute<'a>(
        &'a self,
        binding: &'a WorkspaceGovernanceBinding,
        request: &'a WorkspaceGovernanceRequest,
    ) -> WorkspaceGovernanceFuture<'a> {
        Box::pin(async move {
            let denied = || HostServiceError::new("workspace_governance_authority_denied");
            let remaining = i128::from(binding.deadline_unix_ms)
                - time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            let remaining = u64::try_from(remaining)
                .ok()
                .filter(|ms| *ms > 0)
                .ok_or_else(|| HostServiceError::new("workspace_governance_deadline_exceeded"))?;
            if !(binding.can_manage || (!request.requires_manage() && binding.can_read)) {
                return Err(denied());
            }
            tokio::time::timeout(std::time::Duration::from_millis(remaining), async {
                let installation_id =
                    id(binding.subject.installation_id().as_str()).map_err(|_| denied())?;
                if binding.subject.workspace_id().as_str() != domain::SYSTEM_SCOPE_ID.to_string() {
                    return Err(denied());
                }
                let lease = self
                    .authority
                    .lock_managed_callback_authority(&binding.subject)
                    .await
                    .map_err(|_| denied())?;
                let installation = lease.installation(installation_id).ok_or_else(denied)?;
                if installation.desired_state != domain::PluginDesiredState::ActiveRequested
                    || installation.organization != binding.publisher_namespace
                    || installation.provider_code != binding.plugin_code
                    || installation.plugin_version != binding.plugin_version
                    || domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
                        != domain::SYSTEM_SCOPE_ID
                {
                    return Err(denied());
                }
                let managed: plugin_framework::ManagedManifest =
                    serde_json::from_value(installation.metadata_json["managed"].clone())
                        .map_err(|_| denied())?;
                let contribution = managed
                    .module
                    .contributions
                    .iter()
                    .find(|item| &item.contribution_id == binding.subject.contribution_id())
                    .ok_or_else(denied)?;
                let permissions = HostContributionGrantPolicy::root_composition()
                    .effective_permissions(
                        installation,
                        domain::SYSTEM_SCOPE_ID,
                        contribution,
                        lease.snapshot(),
                    )
                    .map_err(|_| denied())?;
                let allowed = |code| {
                    permissions.iter().any(|p| p.as_str() == code)
                        && contribution
                            .required_permissions
                            .iter()
                            .any(|p| p.as_str() == code)
                };
                if !(binding.can_manage && allowed(WORKSPACE_GOVERNANCE_MANAGE_PERMISSION)
                    || !request.requires_manage()
                        && binding.can_read
                        && allowed(WORKSPACE_GOVERNANCE_READ_PERMISSION))
                {
                    return Err(denied());
                }
                let result = self
                    .dispatch(binding, request)
                    .await
                    .map_err(governance_error);
                lease.release().await.map_err(|_| denied())?;
                result
            })
            .await
            .map_err(|_| HostServiceError::new("workspace_governance_deadline_exceeded"))?
        })
    }
}

fn governance_error(error: anyhow::Error) -> HostServiceError {
    let (code, status) = match error.downcast_ref::<ControlPlaneError>() {
        Some(ControlPlaneError::PermissionDenied(code)) => (*code, 403),
        Some(ControlPlaneError::Conflict(code)) => (*code, 409),
        Some(ControlPlaneError::InvalidInput(code)) => (*code, 400),
        Some(ControlPlaneError::NotFound(code)) => (*code, 404),
        _ => ("workspace_governance_failed", 500),
    };
    let mut error = HostServiceError::new(code);
    error.status = Some(status);
    error
}

fn id(value: &str) -> anyhow::Result<Uuid> {
    Ok(Uuid::parse_str(value)
        .map_err(|_| ControlPlaneError::InvalidInput("workspace_governance_id"))?)
}

fn authenticated_actor(
    binding: &WorkspaceGovernanceBinding,
    claimed: Option<&str>,
) -> anyhow::Result<Option<Uuid>> {
    let authenticated = binding
        .caller
        .as_ref()
        .map(|caller| id(&caller.user_id))
        .transpose()?;
    if let Some(claimed) = claimed {
        if Some(id(claimed)?) != authenticated {
            return Err(
                ControlPlaneError::PermissionDenied("workspace_governance_actor_mismatch").into(),
            );
        }
    }
    Ok(authenticated)
}

fn provision_user(
    member: &WorkspaceGovernanceInitialMemberDto,
) -> anyhow::Result<ProvisionWorkspaceUser> {
    Ok(match member {
        WorkspaceGovernanceInitialMemberDto::Existing { user_id } => {
            ProvisionWorkspaceUser::Existing {
                user_id: id(user_id)?,
            }
        }
        WorkspaceGovernanceInitialMemberDto::NewPasswordUser {
            account,
            email,
            password,
            name,
            nickname,
        } => ProvisionWorkspaceUser::New(Box::new(governance_password_user(
            account, email, password, name, nickname,
        )?)),
    })
}

fn tenant_dto(record: domain::TenantRecord) -> WorkspaceGovernanceTenantDto {
    WorkspaceGovernanceTenantDto {
        id: record.id.to_string(),
        code: record.code,
        name: record.name,
        is_root: record.is_root,
        is_hidden: record.is_hidden,
        status: record.status.as_str().into(),
        cell_id: record.cell_id,
        workspaces: Vec::new(),
    }
}

fn workspace_dto(workspace: GovernanceWorkspace) -> WorkspaceGovernanceWorkspaceDto {
    let record = workspace.record;
    WorkspaceGovernanceWorkspaceDto {
        status: workspace.status.as_str().into(),
        is_root_workspace: workspace.is_root_workspace,
        id: record.id.to_string(),
        tenant_id: record.tenant_id.to_string(),
        name: record.name,
        logo_url: record.logo_url,
        introduction: record.introduction,
    }
}

fn user_dto(record: GovernanceUserSummary) -> WorkspaceGovernanceUserDto {
    WorkspaceGovernanceUserDto {
        user_id: record.user_id.to_string(),
        account: record.account,
        email: record.email,
        name: record.name,
        status: match record.status {
            domain::UserStatus::Active => "active",
            domain::UserStatus::Disabled => "disabled",
        }
        .into(),
        managed_by_tenant_id: record.managed_by_tenant_id.map(|id| id.to_string()),
    }
}
