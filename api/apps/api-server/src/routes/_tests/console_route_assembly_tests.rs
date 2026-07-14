use access_control::{
    ConsoleAuthorization, ConsoleOperationRegistry, ConsoleRouteAssemblyBinding,
    ConsoleRouteBinding, ConsoleRouteOwnership, ResourceAccessScopeKind,
};

use crate::{
    app_state::compile_core_settings_feature_registry,
    routes::console_route_assembly::{
        compile_migrated_core_console_operation_registry, migrated_core_console_route_assembly,
    },
};

fn assembled_route(
    method: &str,
    path: &str,
    ownership: ConsoleRouteOwnership,
) -> ConsoleRouteAssemblyBinding {
    ConsoleRouteAssemblyBinding {
        route: ConsoleRouteBinding {
            method: method.to_string(),
            path: path.to_string(),
        },
        ownership,
    }
}

#[test]
fn console_route_assembly_unclassified_route_fails_coverage() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = ConsoleOperationRegistry::compile(&settings, [], []).unwrap();
    let error = registry
        .validate_console_route_coverage([assembled_route(
            "GET",
            "/api/console/session",
            ConsoleRouteOwnership::Authenticated,
        )])
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("missing compiled ownership: GET /api/console/session"));
}

#[test]
fn console_route_assembly_duplicate_ownership_fails_coverage() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = ConsoleOperationRegistry::compile(&settings, [], []).unwrap();
    let error = registry
        .validate_console_route_coverage([
            assembled_route(
                "GET",
                "/api/console/settings/applications",
                ConsoleRouteOwnership::ConsoleOperation(
                    "settings_feature.access.system.applications".to_string(),
                ),
            ),
            assembled_route(
                "GET",
                "/api/console/settings/applications",
                ConsoleRouteOwnership::Authenticated,
            ),
        ])
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("duplicate assembled console route ownership"));
}

#[test]
fn migrated_real_core_console_route_assembly_has_compiled_coverage() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();

    registry
        .validate_console_route_coverage(assembly.bindings().iter().cloned())
        .unwrap();
    assert!(assembly.bindings().iter().any(|binding| {
        binding.route.path == "/api/console/settings/applications"
            && binding.ownership
                == ConsoleRouteOwnership::ConsoleOperation(
                    "settings_feature.access.system.applications".to_string(),
                )
    }));
}

#[test]
fn applications_routes_compile_exact_operations_and_resource_metadata() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();
    let application_bindings = assembly
        .bindings()
        .iter()
        .filter(|binding| binding.route.path.starts_with("/api/console/applications"))
        .collect::<Vec<_>>();
    assert_eq!(
        application_bindings
            .iter()
            .map(|binding| {
                (
                    binding.route.method.as_str(),
                    binding.route.path.as_str(),
                    match &binding.ownership {
                        ConsoleRouteOwnership::ConsoleOperation(operation_id) => {
                            operation_id.as_str()
                        }
                        ConsoleRouteOwnership::Authenticated => "authenticated",
                    },
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("GET", "/api/console/applications", "applications.view"),
            ("POST", "/api/console/applications", "applications.create"),
            ("GET", "/api/console/applications/:id", "applications.view",),
            (
                "PATCH",
                "/api/console/applications/:id",
                "applications.update",
            ),
            (
                "DELETE",
                "/api/console/applications/:id",
                "applications.delete",
            ),
            (
                "GET",
                "/api/console/applications/catalog",
                "applications.create",
            ),
            (
                "POST",
                "/api/console/applications/tags",
                "applications.create",
            ),
            (
                "GET",
                "/api/console/applications/:id/environment-variables",
                "applications.view",
            ),
            (
                "PUT",
                "/api/console/applications/:id/environment-variables",
                "applications.update",
            ),
            (
                "GET",
                "/api/console/applications/:id/js-dependencies",
                "applications.view",
            ),
            (
                "PUT",
                "/api/console/applications/:id/js-dependencies",
                "applications.update",
            ),
        ]
    );

    let expected_routes = [
        ("GET", "/api/console/applications", "applications.view"),
        ("POST", "/api/console/applications", "applications.create"),
        (
            "GET",
            "/api/console/applications/00000000-0000-0000-0000-000000000001",
            "applications.view",
        ),
        (
            "PATCH",
            "/api/console/applications/00000000-0000-0000-0000-000000000001",
            "applications.update",
        ),
        (
            "DELETE",
            "/api/console/applications/00000000-0000-0000-0000-000000000001",
            "applications.delete",
        ),
        (
            "GET",
            "/api/console/applications/catalog",
            "applications.create",
        ),
        (
            "POST",
            "/api/console/applications/tags",
            "applications.create",
        ),
        (
            "GET",
            "/api/console/applications/00000000-0000-0000-0000-000000000001/environment-variables",
            "applications.view",
        ),
        (
            "PUT",
            "/api/console/applications/00000000-0000-0000-0000-000000000001/environment-variables",
            "applications.update",
        ),
        (
            "GET",
            "/api/console/applications/00000000-0000-0000-0000-000000000001/js-dependencies",
            "applications.view",
        ),
        (
            "PUT",
            "/api/console/applications/00000000-0000-0000-0000-000000000001/js-dependencies",
            "applications.update",
        ),
    ];
    for (method, path, operation_id) in expected_routes {
        let access = registry.access_for_console_route(method, path).unwrap();
        assert_eq!(access.operation_id, operation_id);
    }

    assert_eq!(
        registry
            .access_for_console_route("POST", "/api/console/applications")
            .unwrap()
            .authorization,
        &ConsoleAuthorization::Simple
    );
    for (operation_id, action_code) in [
        ("applications.view", "view"),
        ("applications.update", "update"),
        ("applications.delete", "delete"),
    ] {
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .unwrap();
        assert_eq!(
            operation.authorization,
            ConsoleAuthorization::ResourceAction {
                resource_code: "applications".to_string(),
                action_code: action_code.to_string(),
            }
        );
    }

    let resource = registry
        .inventory()
        .resources
        .iter()
        .find(|resource| resource.resource_code == "applications")
        .unwrap();
    assert_eq!(resource.scope_kind, ResourceAccessScopeKind::Workspace);
    assert_eq!(resource.identity_field, "id");
    assert_eq!(resource.scope_field.as_deref(), Some("scope_id"));
    assert_eq!(resource.owner_field.as_deref(), Some("created_by"));
    assert_eq!(
        resource
            .actions
            .iter()
            .map(|action| action.action_code.as_str())
            .collect::<Vec<_>>(),
        vec!["create", "delete", "update", "view"]
    );
    assert_eq!(resource.label_ref, "console.resources.applications.label");
    assert!(resource.actions.iter().all(|action| {
        action.label_ref
            == format!(
                "console.resources.applications.actions.{}.label",
                action.action_code
            )
    }));
}

#[test]
fn applications_closed_set_rejects_duplicate_or_missing_binding() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let application_bindings = assembly
        .bindings()
        .iter()
        .filter(|binding| binding.route.path.starts_with("/api/console/applications"))
        .cloned()
        .collect::<Vec<_>>();

    let mut duplicate = assembly.bindings().to_vec();
    duplicate.push(application_bindings[0].clone());
    assert!(
        compile_migrated_core_console_operation_registry(&settings, &duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate console route ownership")
    );

    let missing = assembly
        .bindings()
        .iter()
        .filter(|binding| {
            !(binding.route.method == "DELETE"
                && binding.route.path == "/api/console/applications/:id")
        })
        .cloned()
        .collect::<Vec<_>>();
    let error = compile_migrated_core_console_operation_registry(&settings, &missing).unwrap_err();
    assert!(error
        .to_string()
        .contains("operation applications.delete must own at least one console route"));
}
