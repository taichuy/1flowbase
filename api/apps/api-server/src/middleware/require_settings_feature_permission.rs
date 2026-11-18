pub use api_host_kernel::middleware::require_settings_feature_permission::*;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
mod tests {
    use std::{panic::AssertUnwindSafe, sync::Arc};

    use access_control::{
        ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
        ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteBinding, ResourceAccessAction,
        ResourceAccessRegistration, ResourceAccessScopeKind, SettingsFeatureLifecycle,
        SettingsFeatureOwnerKind, SettingsFeatureRegistry,
    };
    use domain::{
        ActorContext, ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope,
        RoleConsoleGroupPolicy, RoleConsolePolicy,
    };
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
    async fn unregistered_mounted_console_route_fails_catalog_publication() {
        let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let empty_settings = SettingsFeatureRegistry::compile([]).unwrap();
        let empty_registry =
            Arc::new(ConsoleOperationRegistry::compile(&empty_settings, [], []).unwrap());
        let state = Arc::new(crate::app_state::ApiState {
            console_operation_registry: empty_registry,
            ..(*base_state).clone()
        });
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            crate::app_with_state_and_config(state, &crate::_tests::support::test_config())
        }));
        let panic = match result {
            Ok(_) => panic!("unknown Console authorization operation must fail publication"),
            Err(panic) => panic,
        };
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .expect("router publication panic must contain a message");
        assert!(
            message.contains("uses unknown authorization operation"),
            "unexpected publication failure: {message}"
        );
    }
}
