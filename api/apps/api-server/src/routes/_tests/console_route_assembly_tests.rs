use access_control::{
    ConsoleOperationRegistry, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership,
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
