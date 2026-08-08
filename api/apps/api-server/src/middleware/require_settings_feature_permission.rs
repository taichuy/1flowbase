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

pub(crate) fn compiled_console_route_access<'a>(
    registry: &'a ConsoleOperationRegistry,
    method: &str,
    path: &str,
) -> Result<ConsoleRouteAccess<'a>, &'static str> {
    registry
        .access_for_console_route(method, path)
        .map_err(|_| "console_route_unregistered")
}

pub(crate) fn authorize_compiled_console_access(
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
    let path = request.uri().path();
    if path != "/api/console" && !path.starts_with("/api/console/") {
        return Ok(next.run(request).await);
    }

    let context = require_session(&state, request.headers()).await?;
    let access = compiled_console_route_access(
        &state.console_operation_registry,
        request.method().as_str(),
        path,
    )
    .map_err(ControlPlaneError::PermissionDenied)?;

    if matches!(access.authorization, ConsoleAuthorization::Authenticated) || context.actor.is_root
    {
        return Ok(next.run(request).await);
    }

    let policies = state
        .store
        .load_console_policy_for_bound_role(
            context.actor.user_id,
            context.actor.current_workspace_id,
            &context.actor.effective_display_role,
        )
        .await?;
    if authorize_compiled_console_access(&access, &context.actor, &policies) {
        return Ok(next.run(request).await);
    }

    Err(ControlPlaneError::PermissionDenied("console_operation_permission_denied").into())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use access_control::{
        ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
        ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteBinding, ResourceAccessAction,
        ResourceAccessRegistration, ResourceAccessScopeKind, SettingsFeatureLifecycle,
        SettingsFeatureOwnerKind, SettingsFeatureRegistry,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use domain::{
        ActorContext, ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope,
        RoleConsoleGroupPolicy, RoleConsolePolicy,
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::{authorize_compiled_console_access, compiled_console_route_access};

    fn test_registry() -> ConsoleOperationRegistry {
        let settings = SettingsFeatureRegistry::compile([]).unwrap();
        let owner = ConsoleOperationOwner {
            kind: SettingsFeatureOwnerKind::Core,
            owner_id: "middleware-tests".to_string(),
            version: "test".to_string(),
        };
        let group = ConsolePolicyGroup::Other("other.middleware-tests".to_string());

        ConsoleOperationRegistry::compile(
            &settings,
            [
                ConsoleOperationRegistration {
                    operation_id: "middleware.simple".to_string(),
                    authorization_profile_id: None,
                    owner: owner.clone(),
                    lifecycle: SettingsFeatureLifecycle::Active,
                    policy_group: group.clone(),
                    order: 1,
                    routes: vec![ConsoleRouteBinding {
                        method: "GET".to_string(),
                        path: "/api/console/test/simple".to_string(),
                    }],
                    authorization: ConsoleAuthorization::Simple,
                },
                ConsoleOperationRegistration {
                    operation_id: "middleware.row".to_string(),
                    authorization_profile_id: None,
                    owner: owner.clone(),
                    lifecycle: SettingsFeatureLifecycle::Active,
                    policy_group: group.clone(),
                    order: 2,
                    routes: vec![ConsoleRouteBinding {
                        method: "GET".to_string(),
                        path: "/api/console/test/row/:id".to_string(),
                    }],
                    authorization: ConsoleAuthorization::ResourceAction {
                        resource_code: "middleware_rows".to_string(),
                        action_code: "view".to_string(),
                    },
                },
                ConsoleOperationRegistration {
                    operation_id: "middleware.authenticated".to_string(),
                    authorization_profile_id: None,
                    owner,
                    lifecycle: SettingsFeatureLifecycle::Active,
                    policy_group: group,
                    order: 3,
                    routes: vec![ConsoleRouteBinding {
                        method: "GET".to_string(),
                        path: "/api/console/test/authenticated".to_string(),
                    }],
                    authorization: ConsoleAuthorization::Authenticated,
                },
            ],
            [ResourceAccessRegistration {
                resource_code: "middleware_rows".to_string(),
                owner: ConsoleOperationOwner {
                    kind: SettingsFeatureOwnerKind::Core,
                    owner_id: "middleware-tests".to_string(),
                    version: "test".to_string(),
                },
                lifecycle: SettingsFeatureLifecycle::Active,
                scope_kind: ResourceAccessScopeKind::Workspace,
                identity_field: "id".to_string(),
                scope_field: Some("scope_id".to_string()),
                owner_field: Some("created_by".to_string()),
                label_ref: "test.rows".to_string(),
                description_ref: None,
                actions: vec![ResourceAccessAction {
                    action_code: "view".to_string(),
                    label_ref: "test.rows.view".to_string(),
                    description_ref: None,
                }],
            }],
        )
        .unwrap()
    }

    fn policy_group() -> domain::ConsolePolicyGroup {
        domain::ConsolePolicyGroup::other("other.middleware-tests")
            .expect("test policy group must be valid")
    }

    fn policy(policy: ConsoleOperationPolicy) -> RoleConsolePolicy {
        RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![RoleConsoleGroupPolicy::custom(policy_group(), vec![policy])],
        )
    }

    #[test]
    fn unregistered_console_route_is_denied_even_for_root() {
        let registry = test_registry();
        let root = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");

        assert_eq!(
            compiled_console_route_access(&registry, "GET", "/api/console/test/missing")
                .unwrap_err(),
            "console_route_unregistered"
        );
        let access =
            compiled_console_route_access(&registry, "GET", "/api/console/test/simple").unwrap();
        assert!(authorize_compiled_console_access(&access, &root, &[]));
    }

    #[test]
    fn simple_console_operation_ignores_legacy_permission_codes() {
        let registry = test_registry();
        let actor = ActorContext::scoped(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "member",
            ["middleware.simple".to_string()],
        );
        let access =
            compiled_console_route_access(&registry, "GET", "/api/console/test/simple").unwrap();

        assert!(!authorize_compiled_console_access(&access, &actor, &[]));
        assert!(authorize_compiled_console_access(
            &access,
            &actor,
            &[policy(ConsoleOperationPolicy::simple(
                ConsoleOperationId::try_from("middleware.simple").unwrap(),
                true,
            ))]
        ));
    }

    #[test]
    fn resource_action_requires_row_scope_and_does_not_filter_rows() {
        let registry = test_registry();
        let actor = ActorContext::scoped(Uuid::now_v7(), Uuid::now_v7(), "member", []);
        let access =
            compiled_console_route_access(&registry, "GET", "/api/console/test/row/row-1").unwrap();

        assert!(!authorize_compiled_console_access(&access, &actor, &[]));
        assert!(!authorize_compiled_console_access(
            &access,
            &actor,
            &[policy(ConsoleOperationPolicy::simple(
                ConsoleOperationId::try_from("middleware.row").unwrap(),
                true,
            ))]
        ));
        assert!(authorize_compiled_console_access(
            &access,
            &actor,
            &[policy(ConsoleOperationPolicy::row(
                ConsoleOperationId::try_from("middleware.row").unwrap(),
                ConsoleOperationRowScope::Own,
            ))]
        ));
    }

    #[test]
    fn authenticated_operation_requires_session_but_not_role_policy() {
        let registry = test_registry();
        let actor = ActorContext::scoped(Uuid::now_v7(), Uuid::now_v7(), "member", []);
        let access =
            compiled_console_route_access(&registry, "GET", "/api/console/test/authenticated")
                .unwrap();

        assert_eq!(access.authorization, &ConsoleAuthorization::Authenticated);
        assert!(authorize_compiled_console_access(&access, &actor, &[]));
    }

    #[tokio::test]
    async fn unregistered_mounted_console_route_returns_403_for_root_session() {
        let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let empty_settings = SettingsFeatureRegistry::compile([]).unwrap();
        let empty_registry =
            Arc::new(ConsoleOperationRegistry::compile(&empty_settings, [], []).unwrap());
        let state = Arc::new(crate::app_state::ApiState {
            console_operation_registry: empty_registry,
            ..(*base_state).clone()
        });
        let app = crate::app_with_state_and_config(state, &crate::_tests::support::test_config());
        let (cookie, _) =
            crate::_tests::support::login_and_capture_cookie(&app, "root", "change-me").await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/console/frontend-blocks")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
