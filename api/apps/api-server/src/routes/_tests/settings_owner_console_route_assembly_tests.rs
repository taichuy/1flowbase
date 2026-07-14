use access_control::ConsoleRouteOwnership;

fn assert_route_bindings<S>(
    assembly: &crate::routes::console_route_assembly::ConsoleRouteAssembly<S>,
    expected: &[(&str, &str, &str)],
) where
    S: Clone + Send + Sync + 'static,
{
    let actual = assembly
        .bindings()
        .iter()
        .map(|binding| {
            (
                binding.route.method.as_str(),
                binding.route.path.as_str(),
                match &binding.ownership {
                    ConsoleRouteOwnership::Authenticated => "authenticated",
                    ConsoleRouteOwnership::ConsoleOperation(operation_id) => operation_id.as_str(),
                },
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn ac_002_013_members_route_bindings_are_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::members::route_assembly(),
        &[
            ("GET", "/api/console/settings/members", "members.list"),
            ("POST", "/api/console/settings/members", "members.create"),
            (
                "GET",
                "/api/console/settings/members/role-options",
                "members.role_options.list",
            ),
            (
                "PATCH",
                "/api/console/settings/members/:id",
                "members.update",
            ),
            (
                "DELETE",
                "/api/console/settings/members/:id",
                "members.delete",
            ),
            (
                "POST",
                "/api/console/settings/members/:id/disable",
                "members.disable",
            ),
            (
                "POST",
                "/api/console/settings/members/:id/enable",
                "members.enable",
            ),
            (
                "POST",
                "/api/console/settings/members/:id/reset-password",
                "members.password.reset",
            ),
            (
                "PUT",
                "/api/console/settings/members/:id/roles",
                "members.roles.replace",
            ),
        ],
    );
}

#[test]
fn ac_002_013_permissions_route_binding_is_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::permissions::route_assembly(),
        &[(
            "GET",
            "/api/console/settings/roles/permission-options",
            "roles.permission_options.list",
        )],
    );
}

#[test]
fn ac_002_013_roles_route_bindings_are_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::roles::route_assembly(),
        &[
            ("GET", "/api/console/settings/roles", "roles.list"),
            ("POST", "/api/console/settings/roles", "roles.create"),
            (
                "GET",
                "/api/console/settings/roles/data-model-options",
                "roles.data_model_options.list",
            ),
            ("PATCH", "/api/console/settings/roles/:id", "roles.update"),
            ("DELETE", "/api/console/settings/roles/:id", "roles.delete"),
            (
                "GET",
                "/api/console/settings/roles/:id/permissions",
                "roles.permissions.view",
            ),
            (
                "PUT",
                "/api/console/settings/roles/:id/permissions",
                "roles.permissions.replace",
            ),
            (
                "GET",
                "/api/console/settings/roles/:id/frontstage-routes",
                "roles.frontstage_routes.view",
            ),
            (
                "PUT",
                "/api/console/settings/roles/:id/frontstage-routes",
                "roles.frontstage_routes.replace",
            ),
            (
                "GET",
                "/api/console/settings/roles/:id/data-policy",
                "roles.data_policy.view",
            ),
            (
                "PUT",
                "/api/console/settings/roles/:id/data-policy",
                "roles.data_policy.replace",
            ),
        ],
    );
}

#[test]
fn ac_002_013_workspace_route_bindings_are_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::workspace::route_assembly(),
        &[
            ("GET", "/api/console/workspace", "authenticated"),
            ("PATCH", "/api/console/workspace", "workspace.update"),
        ],
    );
}

#[test]
fn ac_002_013_workspaces_route_binding_is_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::workspaces::route_assembly(),
        &[("GET", "/api/console/workspaces", "authenticated")],
    );
}

#[test]
fn ac_002_013_auth_center_route_bindings_are_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::auth_center::route_assembly(),
        &[
            (
                "GET",
                "/api/console/settings/auth-center/overview",
                "auth_center.overview.view",
            ),
            (
                "POST",
                "/api/console/settings/auth-center/authenticators",
                "auth_center.authenticators.create",
            ),
            (
                "PUT",
                "/api/console/settings/auth-center/authenticators/order",
                "auth_center.authenticators.order",
            ),
            (
                "POST",
                "/api/console/settings/auth-center/authenticators/:id/actions/enable",
                "auth_center.authenticators.enable",
            ),
            (
                "POST",
                "/api/console/settings/auth-center/authenticators/:id/copy",
                "auth_center.authenticators.copy",
            ),
            (
                "PUT",
                "/api/console/settings/auth-center/authenticators/:id/config",
                "auth_center.authenticators.update",
            ),
            (
                "DELETE",
                "/api/console/settings/auth-center/authenticators/:id",
                "auth_center.authenticators.delete",
            ),
        ],
    );
}

#[test]
fn ac_002_013_system_route_bindings_are_explicit_and_stable() {
    assert_route_bindings(
        &crate::routes::system::route_assembly(),
        &[
            (
                "GET",
                "/api/console/system/runtime-profile",
                "system.runtime_profile.view",
            ),
            (
                "GET",
                "/api/console/system/release-status",
                "system.release_status.view",
            ),
        ],
    );
}
