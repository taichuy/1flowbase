use std::future::Future;

use axum::http::HeaderMap;
use control_plane::auth::ApiKeyService;
use control_plane::ports::SessionStore;
use domain::{ActorContext, SessionRecord, UserRecord, UserStatus};
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError};

#[derive(Clone)]
pub enum RequestCredential {
    CookieSession(SessionRecord),
    UserApiKey { api_key_id: Uuid },
    ServerDelegation,
}

#[derive(Clone)]
pub struct RequestContext {
    pub credential: RequestCredential,
    pub user: UserRecord,
    pub actor: ActorContext,
}

impl RequestContext {
    pub fn server_delegation(user: UserRecord, actor: ActorContext) -> Self {
        Self {
            credential: RequestCredential::ServerDelegation,
            user,
            actor,
        }
    }

    pub fn cookie_session(
        &self,
    ) -> Result<&SessionRecord, control_plane::errors::ControlPlaneError> {
        match &self.credential {
            RequestCredential::CookieSession(session) => Ok(session),
            RequestCredential::UserApiKey { .. } => {
                Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "cookie_session_required",
                ))
            }
            RequestCredential::ServerDelegation => {
                Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "cookie_session_required",
                ))
            }
        }
    }

    pub fn csrf_required(&self) -> bool {
        matches!(self.credential, RequestCredential::CookieSession(_))
    }

    pub fn credential_kind(&self) -> &'static str {
        match &self.credential {
            RequestCredential::CookieSession(_) => "session",
            RequestCredential::UserApiKey { .. } => "user_api_key",
            RequestCredential::ServerDelegation => "server_delegation",
        }
    }

    pub fn interface_principal(&self) -> interface_runtime::UserPrincipal {
        if let RequestCredential::CookieSession(session) = &self.credential {
            return interface_runtime::UserPrincipal::with_authenticated_session(
                self.actor.clone(),
                interface_runtime::AuthenticatedSessionIdentity::new(session.session_id.clone())
                    .expect("authenticated session identity is non-empty"),
            );
        }
        let credential_kind = match &self.credential {
            RequestCredential::CookieSession(_) => unreachable!("cookie session returned above"),
            RequestCredential::UserApiKey { api_key_id } => {
                interface_runtime::UserCredentialKind::UserApiKey {
                    api_key_id: *api_key_id,
                }
            }
            RequestCredential::ServerDelegation => {
                interface_runtime::UserCredentialKind::ServerDelegation
            }
        };
        interface_runtime::UserPrincipal::new(self.actor.clone(), credential_kind)
    }
}

tokio::task_local! {
    static SERVER_DELEGATED_REQUEST_CONTEXT: RequestContext;
}

/// Trusted in-process delegation only; request headers cannot populate this context.
pub(crate) fn server_delegated_request_context() -> Option<RequestContext> {
    SERVER_DELEGATED_REQUEST_CONTEXT.try_with(Clone::clone).ok()
}

pub async fn with_server_delegated_request_context<F>(
    context: RequestContext,
    future: F,
) -> F::Output
where
    F: Future,
{
    SERVER_DELEGATED_REQUEST_CONTEXT
        .scope(context, future)
        .await
}

fn extract_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == cookie_name).then(|| value.to_string())
    })
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    raw.strip_prefix("Bearer ")
}

pub async fn require_session(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<RequestContext, ApiError> {
    if let Ok(context) = SERVER_DELEGATED_REQUEST_CONTEXT.try_with(Clone::clone) {
        return apply_ui_management_permission(state, context).await;
    }

    if let Some(token) = extract_bearer_token(headers) {
        let user_api_key = ApiKeyService::new(state.store.clone())
            .authenticate_user_api_key(token)
            .await?;
        return apply_ui_management_permission(
            state,
            RequestContext {
                credential: RequestCredential::UserApiKey {
                    api_key_id: user_api_key.api_key.id,
                },
                user: user_api_key.user,
                actor: user_api_key.actor,
            },
        )
        .await;
    }

    let session_id = extract_cookie(headers, &state.cookie_name)
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    let session = state
        .session_store
        .get(&session_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    let user = state
        .store
        .find_user_by_id(session.user_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;

    if user.session_version != session.session_version
        || matches!(user.status, UserStatus::Disabled)
    {
        state.session_store.delete(&session.session_id).await?;
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }

    let actor = state
        .store
        .load_actor_context(
            user.id,
            session.tenant_id,
            session.current_workspace_id,
            Some(&session.active_role_code),
        )
        .await?;
    if actor.effective_display_role != session.active_role_code {
        state.session_store.delete(&session.session_id).await?;
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }

    apply_ui_management_permission(
        state,
        RequestContext {
            credential: RequestCredential::CookieSession(session),
            user,
            actor,
        },
    )
    .await
}

/// Design-mode visibility and the existing frontstage service guard are projections of the
/// current UI-management policy. A persisted legacy design grant cannot override a closed group.
async fn apply_ui_management_permission(
    state: &ApiState,
    mut context: RequestContext,
) -> Result<RequestContext, ApiError> {
    apply_ui_management_permission_to_actor(state, &mut context.actor).await?;
    Ok(context)
}

pub(crate) async fn apply_ui_management_permission_to_actor(
    state: &ApiState,
    actor: &mut ActorContext,
) -> Result<(), ApiError> {
    if actor.is_root {
        return Ok(());
    }
    let policies = state
        .console_policy_reader
        .load_role_console_policies_for_user(actor)
        .await?;
    actor.permissions.remove("frontstage.page.design");
    for operation in &state.console_operation_registry.inventory().operations {
        if operation.policy_group
            == access_control::ConsolePolicyGroup::SettingsFeature(
                access_control::SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_ID.to_string(),
            )
        {
            actor.permissions.remove(&operation.operation_id);
        }
    }
    let permissions =
        ui_management_permissions(state.console_operation_registry.inventory(), &policies);
    if !permissions.is_empty() {
        actor
            .permissions
            .insert("frontstage.page.design".to_string());
    }
    actor.permissions.extend(permissions);
    Ok(())
}

fn ui_management_permissions(
    inventory: &access_control::ConsoleOperationCompiledInventory,
    policies: &[domain::RoleConsolePolicy],
) -> std::collections::BTreeSet<String> {
    let group = domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_ID,
    )
    .expect("static UI-management group is valid");
    let ui_group = access_control::ConsolePolicyGroup::SettingsFeature(
        access_control::SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_ID.to_string(),
    );
    inventory
        .operations
        .iter()
        .filter(|operation| {
            operation.policy_group == ui_group
                && operation.lifecycle == access_control::SettingsFeatureLifecycle::Active
                && matches!(
                    operation.authorization,
                    access_control::ConsoleAuthorization::Simple
                )
                && domain::ConsoleOperationId::try_from(operation.operation_id.as_str()).is_ok_and(
                    |id| domain::effective_console_simple_operation(policies, &group, &id),
                )
        })
        .map(|operation| operation.operation_id.clone())
        .collect()
}
