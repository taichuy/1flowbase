use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoginEntrySummary {
    pub id: Uuid,
    pub title: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct BindingResponse {
    pub identity_id: Uuid,
    pub state: String,
    pub subject_type: String,
    pub subject_value: String,
    pub issuer: Option<String>,
    pub account: Option<String>,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
    pub bound_at: String,
    pub unlinked_at: Option<String>,
    pub provider_available: bool,
    pub can_unlink: bool,
    pub unlink_blocked_reason: Option<String>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderResponse {
    pub auth_type: String,
    pub display_name: String,
    pub provider_available: bool,
    pub login_entries: Vec<LoginEntrySummary>,
    pub bindings: Vec<BindingResponse>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthenticationMethodsResponse {
    pub password_configured: bool,
    pub password_usable: bool,
    pub password_setup_verified: bool,
    pub password_verification_entries: Vec<LoginEntrySummary>,
    pub providers: Vec<ProviderResponse>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializePasswordBody {
    pub new_password: String,
}
