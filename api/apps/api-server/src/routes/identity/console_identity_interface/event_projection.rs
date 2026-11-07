use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct SessionActorEvent {
    id: String,
    account: String,
    effective_display_role: String,
    current_workspace_id: String,
}
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct SessionMetadataEvent {
    user_id: String,
    tenant_id: String,
    current_workspace_id: String,
    active_role_code: String,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct SessionEvent<'a> {
    actor: SessionActorEvent,
    session: SessionMetadataEvent,
    available_roles: &'a [session::AvailableRoleResponse],
    active_role_permissions: &'a [String],
    cookie_name: &'a str,
}
// This is a separate event contract. HTTP continues to serialize the token.
#[derive(serde::Serialize, utoipa::ToSchema)]
struct ApiKeyEvent<'a> {
    id: Uuid,
    name: &'a str,
    token_prefix: &'a str,
    key_kind: &'a str,
    role_code: &'a Option<String>,
    creator_user_id: Uuid,
    tenant_id: Uuid,
    scope_kind: &'a str,
    scope_id: Uuid,
    enabled: bool,
    revoked: bool,
    expires_at: &'a Option<String>,
    last_used_at: &'a Option<String>,
    created_at: &'a str,
    updated_at: &'a str,
}
impl<'a> From<&'a user_api_keys::UserApiKeyResponse> for ApiKeyEvent<'a> {
    fn from(value: &'a user_api_keys::UserApiKeyResponse) -> Self {
        Self {
            id: value.id,
            name: &value.name,
            token_prefix: &value.token_prefix,
            key_kind: &value.key_kind,
            role_code: &value.role_code,
            creator_user_id: value.creator_user_id,
            tenant_id: value.tenant_id,
            scope_kind: &value.scope_kind,
            scope_id: value.scope_id,
            enabled: value.enabled,
            revoked: value.revoked,
            expires_at: &value.expires_at,
            last_used_at: &value.last_used_at,
            created_at: &value.created_at,
            updated_at: &value.updated_at,
        }
    }
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct ApiKeysEvent<'a> {
    items: Vec<ApiKeyEvent<'a>>,
}

impl ApiEventInput for ConsoleIdentityInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "console.identity.me.change-password").then(|| json!({"type":"object","properties":{"body":{"type":"object","properties":{},"additionalProperties":false}},"required":["body"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::GetSession
            | Self::DeleteSession
            | Self::RevokeAllSessions
            | Self::GetMe
            | Self::ListUserApiKeys
            | Self::ListUserApiKeyRoleOptions => json!({}),
            Self::SwitchWorkspace { workspace_id } => json!({"body":{"workspace_id":workspace_id}}),
            Self::SwitchRole { role_code } => json!({"body":{"role_code":role_code}}),
            Self::PatchMe(body) => json!({"body":event::to_value(body)?}),
            Self::PatchMeMeta(body) => json!({"body":event::to_value(body)?}),
            Self::ChangePassword(_) => json!({"body":{}}),
            Self::CreateUserApiKey(body) => json!({"body":event::to_value(body)?}),
            Self::RevokeUserApiKey { api_key_id } => json!({"path":{"api_key_id":api_key_id}}),
        })
    }
}
impl ApiEventOutput for ConsoleIdentityOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "console.identity.session.get"
            | "console.identity.session.switch-workspace"
            | "console.identity.session.switch-role" => {
                Some(event::schema::<SessionEvent<'static>>())
            }
            "console.identity.user-api-keys.list" => Some(event::schema::<ApiKeysEvent<'static>>()),
            "console.identity.user-api-keys.create" => {
                Some(event::schema::<ApiKeyEvent<'static>>())
            }
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Session(value) => event::to_value(&SessionEvent {
                actor: serde_json::from_value(value.actor.clone())
                    .map_err(|error| Arc::<str>::from(error.to_string()))?,
                session: serde_json::from_value(value.session.clone())
                    .map_err(|error| Arc::<str>::from(error.to_string()))?,
                available_roles: &value.available_roles,
                active_role_permissions: &value.active_role_permissions,
                cookie_name: &value.cookie_name,
            }),
            Self::Me(value) => event::to_value(value),
            Self::UserApiKeys(value) => event::to_value(&ApiKeysEvent {
                items: value.items.iter().map(ApiKeyEvent::from).collect(),
            }),
            Self::UserApiKeyRoleOptions(value) => event::to_value(value),
            Self::UserApiKeyCreated(value) => event::to_value(&ApiKeyEvent::from(value)),
            Self::UserApiKeyRevoked(value) => event::to_value(value),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/security.rs"]
mod tests;
