//! System workspace policy, executed through a managed multiplex contribution.
use serde::{Deserialize, Serialize};

pub const WORKSPACE_POLICY_POINT: &str = "1flowbase.workspace.policy";
pub const WORKSPACE_POLICY_PERMISSION: &str = "workspace.policy.decide";
pub const WORKSPACE_POLICY_CONTRACT_ID: &str = "workspace-policy";
pub const WORKSPACE_POLICY_CONTRACT_V1: &str = "1";

// No Debug: registration and redemption requests contain invitation secrets.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspacePolicyRequest {
    #[serde(deserialize_with = "deserialize_empty_variant")]
    InspectPublicPolicy,
    ResolveRegistration {
        account_hint: Option<String>,
        email: String,
        name_hint: Option<String>,
        nickname_hint: Option<String>,
        invitation_token: Option<String>,
    },
    FinalizeInvitationRedemption {
        redemption_id: String,
        committed_user_id: Option<String>,
    },
    RedeemInvitationForUser {
        user_id: String,
        email: String,
        invitation_token: String,
    },
    AdmitOperation {
        interface_id: String,
        tenant_id: String,
        workspace_id: String,
        actor_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspacePolicyResponse {
    PublicPolicy {
        onboarding_mode: WorkspaceOnboardingModeDto,
    },
    RegistrationDecision {
        decision: WorkspaceRegistrationDecisionDto,
    },
    #[serde(deserialize_with = "deserialize_empty_variant")]
    RedemptionFinalized,
    UserInvitationRedemption {
        redemption: WorkspaceUserInvitationRedemptionDto,
    },
    OperationAdmission {
        admission: WorkspaceOperationAdmissionDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceOnboardingModeDto {
    PersonalWorkspace,
    DefaultWorkspace,
    Reject,
    InviteOnly,
}

impl WorkspaceOnboardingModeDto {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PersonalWorkspace => "personal_workspace",
            Self::DefaultWorkspace => "default_workspace",
            Self::Reject => "reject",
            Self::InviteOnly => "invite_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceRegistrationDecisionDto {
    ProvisionPersonalWorkspace {
        tenant_code: String,
        tenant_name: String,
        workspace_name: String,
    },
    JoinExistingWorkspace {
        workspace_id: String,
        role_codes: Vec<String>,
        redemption_id: Option<String>,
    },
    Deny {
        reason_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceUserInvitationRedemptionDto {
    Redeemed {
        workspace_id: String,
        already_active_member: bool,
    },
    Deny {
        reason_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceOperationAdmissionDto {
    #[serde(deserialize_with = "deserialize_empty_variant")]
    Allow,
    Deny {
        reason_code: String,
    },
}

/// Host services have their own durable grants, separate from the policy decision contract.
pub fn is_workspace_policy_callback_permission(permission: &str) -> bool {
    matches!(
        permission,
        crate::WORKSPACE_GOVERNANCE_READ_PERMISSION
            | crate::WORKSPACE_GOVERNANCE_MANAGE_PERMISSION
            | "plugin_data.owned.write"
            | "execution.gate"
    )
}

// Serde's default internally tagged unit visitor ignores extra map fields.
pub(crate) fn deserialize_empty_variant<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<(), D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Empty {}
    Empty::deserialize(deserializer).map(|_| ())
}
