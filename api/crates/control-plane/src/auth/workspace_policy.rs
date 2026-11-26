//! Applies workspace decisions to the identity transaction and session owners.
use super::*;
use control_plane_contracts::ports::*;
use extension_contracts::*;

#[async_trait]
pub trait WorkspacePolicyPort: Send + Sync {
    /// None means there is no active contribution. Worker failure is always an error.
    async fn decide(
        &self,
        request: WorkspacePolicyRequest,
    ) -> Result<Option<WorkspacePolicyResponse>>;
}

pub struct WorkspacePolicyContext {
    pub policy: Arc<dyn WorkspacePolicyPort>,
    pub repository: Arc<dyn WorkspaceIdentityRepository>,
}

pub trait WorkspaceIdentityRepository: WorkspaceGovernanceRepository + WorkspaceRepository {}
impl<T: WorkspaceGovernanceRepository + WorkspaceRepository> WorkspaceIdentityRepository for T {}

#[derive(Debug, thiserror::Error)]
#[error("workspace policy denied request: {code}")]
pub struct WorkspacePolicyRejection {
    pub code: String,
    pub conflict: bool,
}

pub fn workspace_policy_denied(code: String) -> anyhow::Error {
    // Reason codes are machine identifiers, never worker diagnostics or secrets.
    let code = if !code.is_empty()
        && code.len() <= 80
        && code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        code
    } else {
        "workspace_policy_denied".into()
    };
    let conflict = code.ends_with("_quota_exceeded")
        || matches!(
            code.as_str(),
            "invitation_exhausted" | "invitation_already_redeemed" | "invitation_max_uses_reached"
        );
    WorkspacePolicyRejection { code, conflict }.into()
}

pub fn validate_invitation_token(token: Option<&str>) -> Result<Option<&str>> {
    let Some(token) = token.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if token.len() > 512
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ControlPlaneError::InvalidInput("invitation_token").into());
    }
    Ok(Some(token))
}

pub async fn redeem_workspace_invitation(
    policy: &dyn WorkspacePolicyPort,
    user: &domain::UserRecord,
    token: &str,
) -> Result<Option<Uuid>> {
    if user.status == UserStatus::Disabled {
        return Err(ControlPlaneError::PermissionDenied("user_disabled").into());
    }
    match policy
        .decide(WorkspacePolicyRequest::RedeemInvitationForUser {
            user_id: user.id.to_string(),
            email: user.email.clone(),
            invitation_token: token.to_owned(),
        })
        .await?
    {
        None => Ok(None),
        Some(WorkspacePolicyResponse::UserInvitationRedemption {
            redemption: WorkspaceUserInvitationRedemptionDto::Redeemed { workspace_id, .. },
        }) => Ok(Some(policy_id(&workspace_id)?)),
        Some(WorkspacePolicyResponse::UserInvitationRedemption {
            redemption: WorkspaceUserInvitationRedemptionDto::Deny { reason_code },
        }) => Err(workspace_policy_denied(reason_code)),
        _ => Err(invalid_policy()),
    }
}

pub(crate) fn governance_password_user(
    account: &str,
    email: &str,
    password: &str,
    name: &str,
    nickname: &str,
) -> Result<NewGovernanceUser> {
    let account = account.trim().to_lowercase();
    let email = email.trim().to_lowercase();
    validate_self_registration_input(&account, &email, password)?;
    if account.len() > 64
        || email.len() > 254
        || password.len() > 1024
        || [name, nickname]
            .iter()
            .any(|s| s.chars().count() > 128 || s.chars().any(char::is_control))
    {
        return Err(ControlPlaneError::InvalidInput("workspace_governance_user").into());
    }
    Ok(NewGovernanceUser {
        account,
        email,
        name: name.trim().into(),
        nickname: nickname.trim().into(),
        avatar_url: None,
        credential: GovernanceUserCredential::Password {
            password_hash: hash_password(password)?,
        },
    })
}

pub(crate) fn workspace_role_templates() -> Vec<domain::RoleTemplate> {
    access_control::bootstrap_role_templates()
        .into_iter()
        .filter(|role| role.scope_kind == domain::RoleScopeKind::Workspace)
        .collect()
}

pub(super) struct RegistrationHints {
    pub account: Option<String>,
    pub email: String,
    pub name: Option<String>,
    pub nickname: Option<String>,
}

impl WorkspacePolicyContext {
    pub(super) async fn resolve_registration(
        &self,
        hints: RegistrationHints,
        invitation: Option<&str>,
    ) -> Result<Option<WorkspaceRegistrationDecisionDto>> {
        match self
            .policy
            .decide(WorkspacePolicyRequest::ResolveRegistration {
                account_hint: hints.account,
                email: hints.email,
                name_hint: hints.name,
                nickname_hint: hints.nickname,
                invitation_token: invitation.map(str::to_owned),
            })
            .await?
        {
            None => Ok(None),
            Some(WorkspacePolicyResponse::RegistrationDecision {
                decision: WorkspaceRegistrationDecisionDto::Deny { reason_code },
            }) => Err(workspace_policy_denied(reason_code)),
            Some(WorkspacePolicyResponse::RegistrationDecision { decision }) => Ok(Some(decision)),
            _ => Err(invalid_policy()),
        }
    }

    pub(super) async fn release_registration(&self, decision: WorkspaceRegistrationDecisionDto) {
        if let WorkspaceRegistrationDecisionDto::JoinExistingWorkspace {
            redemption_id: Some(redemption_id),
            ..
        } = decision
        {
            if !matches!(
                self.policy
                    .decide(WorkspacePolicyRequest::FinalizeInvitationRedemption {
                        redemption_id,
                        committed_user_id: None
                    })
                    .await,
                Ok(Some(WorkspacePolicyResponse::RedemptionFinalized))
            ) {
                tracing::warn!("workspace invitation reservation rollback could not be finalized");
            }
        }
    }

    pub(super) async fn register(
        &self,
        decision: WorkspaceRegistrationDecisionDto,
        user: NewGovernanceUser,
    ) -> Result<(Uuid, domain::ScopeContext)> {
        match decision {
            WorkspaceRegistrationDecisionDto::ProvisionPersonalWorkspace {
                tenant_code,
                tenant_name,
                workspace_name,
            } => {
                let out = self
                    .repository
                    .provision_workspace(&ProvisionWorkspaceInput {
                        tenant: ProvisionWorkspaceTenant::New {
                            code: tenant_code,
                            name: tenant_name,
                        },
                        workspace_name,
                        introduction: String::new(),
                        logo_url: None,
                        actor_user_id: None,
                        role_templates: workspace_role_templates(),
                        initial_member: Some(ProvisionWorkspaceMember {
                            user: ProvisionWorkspaceUser::New(Box::new(user)),
                            role_codes: vec!["admin".into()],
                        }),
                    })
                    .await?;
                Ok((
                    out.initial_member_user_id.ok_or_else(invalid_policy)?,
                    domain::ScopeContext {
                        tenant_id: out.tenant.id,
                        workspace_id: out.workspace.id,
                    },
                ))
            }
            WorkspaceRegistrationDecisionDto::JoinExistingWorkspace {
                workspace_id,
                role_codes,
                redemption_id,
            } => {
                let result = async {
                    let scope = self.workspace_scope(policy_id(&workspace_id)?).await?;
                    let result = self
                        .repository
                        .onboard_new_user_to_workspace(&OnboardNewUserInput {
                            workspace_id: scope.workspace_id,
                            user,
                            role_codes,
                            role_templates: workspace_role_templates(),
                            actor_user_id: None,
                        })
                        .await?;
                    Ok::<_, anyhow::Error>((result.user_id, scope))
                }
                .await;
                if let Some(redemption_id) = redemption_id {
                    let finalized = self
                        .policy
                        .decide(WorkspacePolicyRequest::FinalizeInvitationRedemption {
                            redemption_id,
                            committed_user_id: result.as_ref().ok().map(|(id, _)| id.to_string()),
                        })
                        .await
                        .and_then(|response| match response {
                            Some(WorkspacePolicyResponse::RedemptionFinalized) => Ok(()),
                            _ => Err(invalid_policy()),
                        });
                    if result.is_ok() {
                        finalized?;
                    } else if finalized.is_err() {
                        tracing::warn!(
                            "workspace invitation reservation rollback could not be finalized"
                        );
                    }
                }
                result
            }
            WorkspaceRegistrationDecisionDto::Deny { reason_code } => {
                Err(workspace_policy_denied(reason_code))
            }
        }
    }

    pub(super) async fn workspace_scope(&self, workspace_id: Uuid) -> Result<domain::ScopeContext> {
        let workspace = self
            .repository
            .get_workspace(workspace_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("workspace"))?;
        Ok(domain::ScopeContext {
            workspace_id,
            tenant_id: workspace.tenant_id,
        })
    }
}

fn policy_id(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|_| invalid_policy())
}

fn invalid_policy() -> anyhow::Error {
    ControlPlaneError::UpstreamUnavailable("workspace_policy_invalid_response").into()
}
