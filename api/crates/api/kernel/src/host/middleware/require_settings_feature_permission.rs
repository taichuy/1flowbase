use std::sync::Arc;

use access_control::{
    ConsoleAuthorization, ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteAccess,
};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use control_plane::errors::ControlPlaneError;
use domain::{
    effective_console_row_scope, effective_console_simple_operation, ActorContext,
    ConsoleOperationId, ConsoleOperationRowScope, RoleConsolePolicy,
};

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
};

pub fn compiled_console_route_access<'a>(
    registry: &'a ConsoleOperationRegistry,
    method: &str,
    path: &str,
) -> Result<ConsoleRouteAccess<'a>, &'static str> {
    registry
        .access_for_console_route(method, path)
        .map_err(|_| "console_route_unregistered")
}

pub fn authorize_compiled_console_access(
    access: &ConsoleRouteAccess<'_>,
    actor: &ActorContext,
    policies: &[RoleConsolePolicy],
) -> bool {
    if actor.is_root {
        return true;
    }

    let group = match access.policy_group {
        ConsolePolicyGroup::SettingsFeature(feature_id) => {
            domain::ConsolePolicyGroup::settings_feature(feature_id).ok()
        }
        ConsolePolicyGroup::Plugin(group_id) => domain::ConsolePolicyGroup::plugin(group_id).ok(),
        ConsolePolicyGroup::Other(group_id) => domain::ConsolePolicyGroup::other(group_id).ok(),
    };
    let Some(group) = group else {
        return false;
    };
    let Ok(operation_id) = ConsoleOperationId::try_from(access.operation_id) else {
        return false;
    };

    match access.authorization {
        ConsoleAuthorization::Authenticated => true,
        ConsoleAuthorization::Simple => {
            effective_console_simple_operation(policies, &group, &operation_id)
        }
        ConsoleAuthorization::ResourceAction { .. } => {
            let Some(resource) = access.resource_access else {
                return false;
            };
            let scope = effective_console_row_scope(policies, &group, &operation_id);
            match scope {
                ConsoleOperationRowScope::Own => resource.owner_field.is_some(),
                ConsoleOperationRowScope::ScopeAll => resource.scope_field.is_some(),
                ConsoleOperationRowScope::Disabled => false,
            }
        }
    }
}

pub async fn require_settings_feature_permission(
    State(state): State<Arc<ApiState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let path = request.uri().path().to_string();
    if path != "/api/console" && !path.starts_with("/api/console/") {
        return Ok(next.run(request).await);
    }

    if let Ok(access) = compiled_console_route_access(
        &state.console_operation_registry,
        request.method().as_str(),
        &path,
    ) {
        if let ConsolePolicyGroup::SettingsFeature(feature_id) = access.policy_group {
            if state.console_surface_registry.is_native_feature(feature_id) {
                use control_plane::ports::PluginRepository;
                let target = state
                    .console_surface_registry
                    .target_for_feature(feature_id)
                    .ok_or(ControlPlaneError::Conflict(
                        "native_plugin_version_mismatch",
                    ))?;
                if !state.store.native_plugin_target_is_applied(target).await? {
                    return Err(
                        ControlPlaneError::Conflict("native_plugin_version_mismatch").into(),
                    );
                }
            }
        }
    }

    if crate::routes::host_infrastructure::interface_operation::is_active_interface_route(
        state.as_ref(),
        request.method().as_str(),
        &path,
    ) {
        // The frozen Interface binding owns credential -> Principal authentication. Running the
        // legacy Console middleware first would bypass that activation on rejection and duplicate
        // session/API-key reads on success.
        return Ok(next.run(request).await);
    }
    let context = require_session(&state, request.headers()).await?;
    let access = compiled_console_route_access(
        &state.console_operation_registry,
        request.method().as_str(),
        &path,
    )
    .map_err(ControlPlaneError::PermissionDenied)?;

    if access.root_workspace_only
        && context.actor.current_workspace_id != state.bootstrap_workspace_id
    {
        return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
    }

    if matches!(access.authorization, ConsoleAuthorization::Authenticated) || context.actor.is_root
    {
        return Ok(next.run(request).await);
    }

    let policies = state
        .console_policy_reader
        .load_role_console_policies_for_user(&context.actor)
        .await?;
    if authorize_compiled_console_access(&access, &context.actor, &policies) {
        return Ok(next.run(request).await);
    }

    Err(ControlPlaneError::PermissionDenied("console_operation_permission_denied").into())
}
