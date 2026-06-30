use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoleScopeKind {
    System,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundRole {
    pub code: String,
    pub scope_kind: RoleScopeKind,
    pub workspace_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UserRecord {
    pub id: Uuid,
    pub account: String,
    pub email: String,
    pub phone: Option<String>,
    pub password_hash: String,
    pub name: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
    pub introduction: String,
    pub preferred_locale: Option<String>,
    pub meta: serde_json::Value,
    pub default_display_role: Option<String>,
    pub email_login_enabled: bool,
    pub phone_login_enabled: bool,
    pub status: UserStatus,
    pub session_version: i64,
    pub roles: Vec<BoundRole>,
}

impl UserRecord {
    pub fn resolved_display_role(&self) -> Option<String> {
        if let Some(default_display_role) = &self.default_display_role {
            if self
                .roles
                .iter()
                .any(|role| role.code == *default_display_role)
            {
                return Some(default_display_role.clone());
            }
        }

        self.roles.first().map(|role| role.code.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorContext {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub current_workspace_id: Uuid,
    pub effective_display_role: String,
    pub is_root: bool,
    pub permissions: HashSet<String>,
}

impl ActorContext {
    pub fn root(user_id: Uuid, current_workspace_id: Uuid, effective_display_role: &str) -> Self {
        Self::root_in_scope(
            user_id,
            Uuid::nil(),
            current_workspace_id,
            effective_display_role,
        )
    }

    pub fn root_in_scope(
        user_id: Uuid,
        tenant_id: Uuid,
        current_workspace_id: Uuid,
        effective_display_role: &str,
    ) -> Self {
        Self {
            user_id,
            tenant_id,
            current_workspace_id,
            effective_display_role: effective_display_role.to_string(),
            is_root: true,
            permissions: HashSet::new(),
        }
    }

    pub fn scoped(
        user_id: Uuid,
        current_workspace_id: Uuid,
        effective_display_role: &str,
        permissions: impl IntoIterator<Item = String>,
    ) -> Self {
        Self::scoped_in_scope(
            user_id,
            Uuid::nil(),
            current_workspace_id,
            effective_display_role,
            permissions,
        )
    }

    pub fn scoped_in_scope(
        user_id: Uuid,
        tenant_id: Uuid,
        current_workspace_id: Uuid,
        effective_display_role: &str,
        permissions: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            user_id,
            tenant_id,
            current_workspace_id,
            effective_display_role: effective_display_role.to_string(),
            is_root: false,
            permissions: permissions.into_iter().collect(),
        }
    }

    pub fn has_permission(&self, code: &str) -> bool {
        self.is_root || self.permissions.contains(code)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionDefinition {
    pub code: String,
    pub resource: String,
    pub action: String,
    pub scope: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleTemplate {
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub scope_kind: RoleScopeKind,
    pub is_builtin: bool,
    pub is_editable: bool,
    pub auto_grant_new_permissions: bool,
    pub is_default_member_role: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthenticatorRecord {
    pub name: String,
    pub auth_type: String,
    pub title: String,
    pub enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i32,
    pub options: serde_json::Value,
}

pub const PASSWORD_LOCAL_AUTHENTICATOR_NAME: &str = "password-local";
pub const AUTH_SUBJECT_TYPE_ACCOUNT: &str = "account";
pub const AUTH_SUBJECT_TYPE_EMAIL: &str = "email";
pub const AUTH_SUBJECT_TYPE_PHONE: &str = "phone";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalIdentityClaim {
    pub authenticator_name: String,
    pub subject_type: String,
    pub subject_value: String,
    pub issuer: Option<String>,
    pub realm: Option<String>,
    pub profile: serde_json::Value,
    pub verified: bool,
    pub metadata: serde_json::Value,
}

pub fn password_local_identity_claims(
    account: &str,
    email: &str,
    phone: Option<&str>,
) -> Vec<ExternalIdentityClaim> {
    let mut claims = vec![
        password_local_identity_claim(AUTH_SUBJECT_TYPE_ACCOUNT, account),
        password_local_identity_claim(AUTH_SUBJECT_TYPE_EMAIL, email),
    ];
    if let Some(phone) = phone {
        claims.push(password_local_identity_claim(
            AUTH_SUBJECT_TYPE_PHONE,
            phone,
        ));
    }
    claims
}

pub fn password_local_contact_identity_claims(
    email: &str,
    phone: Option<&str>,
) -> Vec<ExternalIdentityClaim> {
    let mut claims = vec![password_local_identity_claim(
        AUTH_SUBJECT_TYPE_EMAIL,
        email,
    )];
    if let Some(phone) = phone {
        claims.push(password_local_identity_claim(
            AUTH_SUBJECT_TYPE_PHONE,
            phone,
        ));
    }
    claims
}

fn password_local_identity_claim(subject_type: &str, subject_value: &str) -> ExternalIdentityClaim {
    ExternalIdentityClaim {
        authenticator_name: PASSWORD_LOCAL_AUTHENTICATOR_NAME.to_string(),
        subject_type: subject_type.to_string(),
        subject_value: subject_value.to_string(),
        issuer: None,
        realm: None,
        profile: serde_json::json!({}),
        verified: true,
        metadata: serde_json::json!({}),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserAuthIdentity {
    pub user_id: Uuid,
    pub authenticator_name: String,
    pub subject_type: String,
    pub subject_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub current_workspace_id: Uuid,
    pub session_version: i64,
    pub csrf_token: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyKind {
    ApplicationApiKey,
    UserApiKey,
}

impl ApiKeyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApplicationApiKey => "application_api_key",
            Self::UserApiKey => "user_api_key",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "application_api_key" => Some(Self::ApplicationApiKey),
            "user_api_key" => Some(Self::UserApiKey),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ApiKeyRecord {
    pub id: Uuid,
    pub name: String,
    pub token_hash: String,
    pub token_prefix: String,
    pub key_kind: ApiKeyKind,
    pub application_id: Option<Uuid>,
    pub role_code: Option<String>,
    pub creator_user_id: Uuid,
    pub tenant_id: Uuid,
    pub scope_kind: crate::DataModelScopeKind,
    pub scope_id: Uuid,
    pub enabled: bool,
    pub expires_at: Option<OffsetDateTime>,
    pub last_used_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
