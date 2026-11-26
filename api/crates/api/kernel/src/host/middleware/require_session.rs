use control_plane::ports::AuthRepository;
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
pub fn server_delegated_request_context() -> Option<RequestContext> {
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
        state
            .store
            .verify_workspace_access(
                context.actor.user_id,
                context.actor.tenant_id,
                context.actor.current_workspace_id,
                context.actor.is_root,
            )
            .await?;
        return apply_console_policy_permissions(state, context).await;
    }

    if let Some(token) = extract_bearer_token(headers) {
        let user_api_key = ApiKeyService::new(state.store.clone())
            .authenticate_user_api_key(token)
            .await?;
        return apply_console_policy_permissions(
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
    state
        .store
        .verify_workspace_access(
            actor.user_id,
            session.tenant_id,
            actor.current_workspace_id,
            actor.is_root,
        )
        .await?;
    if actor.effective_display_role != session.active_role_code {
        state.session_store.delete(&session.session_id).await?;
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }

    apply_console_policy_permissions(
        state,
        RequestContext {
            credential: RequestCredential::CookieSession(session),
            user,
            actor,
        },
    )
    .await
}

/// Navigation, design-mode visibility and the existing frontstage service guard are
/// projections of current canonical policies, never persisted legacy feature grants.
async fn apply_console_policy_permissions(
    state: &ApiState,
    mut context: RequestContext,
) -> Result<RequestContext, ApiError> {
    apply_console_policy_permissions_to_actor(state, &mut context.actor).await?;
    Ok(context)
}

pub(crate) async fn apply_console_policy_permissions_to_actor(
    state: &ApiState,
    actor: &mut ActorContext,
) -> Result<(), ApiError> {
    let outside_bootstrap = actor.current_workspace_id != state.bootstrap_workspace_id;
    let inventory = state.console_operation_registry.inventory();
    let features = &state.settings_feature_registry.inventory().features;
    if actor.is_root {
        if outside_bootstrap {
            remove_platform_permissions(actor, inventory, features);
        } else {
            // Clients must use the explicit projection, not the root role alone,
            // to distinguish platform operations available in this session.
            actor.permissions.extend(
                inventory
                    .operations
                    .iter()
                    .filter(|operation| {
                        operation.root_workspace_only
                            && operation.lifecycle
                                == access_control::SettingsFeatureLifecycle::Active
                    })
                    .map(|operation| operation.operation_id.clone()),
            );
        }
        return Ok(());
    }
    let policies = state
        .console_policy_reader
        .load_role_console_policies_for_user(actor)
        .await?;
    // Project the same effective operations consumed by the invocation boundary. Legacy
    // role grants cannot retain stale operation IDs after a group or operation is revoked.
    actor
        .permissions
        .retain(|permission| !permission.starts_with("settings_feature.access."));
    actor.permissions.remove("frontstage.page.design");
    for operation in &inventory.operations {
        actor.permissions.remove(&operation.operation_id);
    }
    let permissions = console_operation_permissions(inventory, &policies);
    if inventory.operations.iter().any(|operation| {
        permissions.contains(&operation.operation_id)
            && operation
                .authorization_profile_id
                .starts_with("frontstage.")
            && operation
                .routes
                .iter()
                .any(|route| matches!(route.method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE"))
    }) {
        actor
            .permissions
            .insert("frontstage.page.design".to_string());
    }
    actor.permissions.extend(permissions);
    actor
        .permissions
        .extend(settings_feature_navigation_permissions(
            inventory,
            &policies,
            &state.settings_feature_registry.inventory().features,
        ));
    if outside_bootstrap {
        remove_platform_permissions(actor, inventory, features);
    }
    Ok(())
}

fn remove_platform_permissions(
    actor: &mut ActorContext,
    inventory: &access_control::ConsoleOperationCompiledInventory,
    features: &[access_control::SettingsFeatureInventoryEntry],
) {
    for operation in inventory
        .operations
        .iter()
        .filter(|op| op.root_workspace_only)
    {
        actor.permissions.remove(&operation.operation_id);
    }
    for feature in features
        .iter()
        .filter(|feature| feature.root_workspace_only)
    {
        actor.permissions.remove(&feature.permission_code);
    }
}

/// A settings surface is discoverable if at least one active operation is granted.
/// Row visibility is not a row-data grant: the request's resource/workspace checks still apply.
pub fn settings_feature_navigation_permissions(
    inventory: &access_control::ConsoleOperationCompiledInventory,
    policies: &[domain::RoleConsolePolicy],
    settings_features: &[access_control::SettingsFeatureInventoryEntry],
) -> std::collections::BTreeSet<String> {
    let mut permissions = std::collections::BTreeSet::new();
    for operation in &inventory.operations {
        if operation.lifecycle != access_control::SettingsFeatureLifecycle::Active {
            continue;
        }
        let (group, feature_ids) = match &operation.policy_group {
            access_control::ConsolePolicyGroup::SettingsFeature(id) => (
                domain::ConsolePolicyGroup::settings_feature(id),
                vec![id.as_str()],
            ),
            access_control::ConsolePolicyGroup::Plugin(id) => (
                domain::ConsolePolicyGroup::plugin(id),
                settings_features
                    .iter()
                    .filter(|feature| feature.owner == operation.owner)
                    .map(|feature| feature.feature_id.as_str())
                    .collect(),
            ),
            access_control::ConsolePolicyGroup::Other(_) => continue,
        };
        let (Ok(group), Ok(id)) = (
            group,
            domain::ConsoleOperationId::try_from(operation.operation_id.as_str()),
        ) else {
            continue;
        };
        let granted = match operation.authorization {
            access_control::ConsoleAuthorization::Simple => {
                domain::effective_console_simple_operation(policies, &group, &id)
            }
            access_control::ConsoleAuthorization::ResourceAction { .. } => {
                domain::effective_console_row_scope(policies, &group, &id)
                    != domain::ConsoleOperationRowScope::Disabled
            }
            access_control::ConsoleAuthorization::Authenticated => false,
        };
        if granted {
            permissions.extend(
                feature_ids
                    .into_iter()
                    .map(|id| format!("settings_feature.access.{id}")),
            );
        }
    }
    permissions
}

fn console_operation_permissions(
    inventory: &access_control::ConsoleOperationCompiledInventory,
    policies: &[domain::RoleConsolePolicy],
) -> std::collections::BTreeSet<String> {
    inventory
        .operations
        .iter()
        .filter(|operation| operation.lifecycle == access_control::SettingsFeatureLifecycle::Active)
        .filter(|operation| {
            let group = match &operation.policy_group {
                access_control::ConsolePolicyGroup::SettingsFeature(id) => {
                    domain::ConsolePolicyGroup::settings_feature(id)
                }
                access_control::ConsolePolicyGroup::Plugin(id) => {
                    domain::ConsolePolicyGroup::plugin(id)
                }
                access_control::ConsolePolicyGroup::Other(id) => {
                    domain::ConsolePolicyGroup::other(id)
                }
            };
            let (Ok(group), Ok(id)) = (
                group,
                domain::ConsoleOperationId::try_from(operation.operation_id.as_str()),
            ) else {
                return false;
            };
            match operation.authorization {
                access_control::ConsoleAuthorization::Simple => {
                    domain::effective_console_simple_operation(policies, &group, &id)
                }
                access_control::ConsoleAuthorization::ResourceAction { .. } => {
                    domain::effective_console_row_scope(policies, &group, &id)
                        != domain::ConsoleOperationRowScope::Disabled
                }
                access_control::ConsoleAuthorization::Authenticated => false,
            }
        })
        .map(|operation| operation.operation_id.clone())
        .collect()
}
