use std::sync::Arc;

use crate::{
    app_state::compile_console_boot_plan_with_interface_operations,
    host_extensions::console::{
        linked_host_console_route_sources, resolve_linked_host_extension_console_contribution,
    },
};
use plugin_framework::parse_host_extension_contribution_manifest;

#[test]
fn root_2014_ac_009_registered_settings_page_authority() {
    let contribution = parse_host_extension_contribution_manifest(&fixture_manifest())
        .expect("fixture HostExtension manifest should be valid");
    assert!(contribution.settings_features[0].api_routes.is_empty());
    assert!(contribution.console_operations.is_empty());
    let host = resolve_linked_host_extension_console_contribution(
        contribution,
        linked_host_console_route_sources(),
    )
    .expect("page-only contribution uses the default formal startup sources");
    let extension_assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().expect("test API workspace root should resolve"),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .expect("test extension graph input should assemble");
    let extension_boot_snapshot = crate::extension_bus::ExtensionBootSnapshot::compile_for_test(
        Arc::new(
            extension_assembly
                .compile_graph()
                .expect("test extension graph should compile"),
        ),
        extension_assembly.interface_operations(),
    )
    .expect("test extension boot snapshot should compile");
    let interface_snapshot = extension_boot_snapshot
        .interface_registry()
        .unwrap()
        .snapshot();
    let plan = compile_console_boot_plan_with_interface_operations(
        [host],
        Some(interface_snapshot.as_ref()),
    )
    .expect("Core and active linked HostExtension should compile as one console plan");

    let error = plan
        .console_operation_registry
        .access_for_console_route("GET", "/api/console/northwind.settings-page/settings")
        .err()
        .expect("page-only registration must not authorize an invented API route");
    assert_eq!(
        error.to_string(),
        "unregistered console route GET /api/console/northwind.settings-page/settings"
    );
    let package = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../../plugins/fixtures/northwind.settings-page/manifest.yaml"
    ))
    .unwrap();
    let native = parse_host_extension_contribution_manifest(&fixture_manifest()).unwrap();
    native.validate_package_settings_pages(&package).unwrap();
    let source = plugin_framework::read_plugin_settings_page_source(
        &crate::api_workspace_root()
            .unwrap()
            .join("plugins/fixtures/northwind.settings-page"),
        &package.settings_pages[0],
    )
    .unwrap();
    assert!(source.contains("Shipping preferences"));
    let denied = domain::ActorContext::scoped(uuid::Uuid::nil(), uuid::Uuid::nil(), "member", []);
    assert!(!plan
        .console_surface_registry
        .accessible_navigation(&denied)
        .route_definitions
        .iter()
        .any(|route| route.route_id == "northwind.settings-page.settings"));
    let allowed = domain::ActorContext::scoped(
        uuid::Uuid::nil(),
        uuid::Uuid::nil(),
        "member",
        ["settings_feature.access.northwind.settings-page.settings".to_string()],
    );
    assert!(plan
        .console_surface_registry
        .accessible_navigation(&allowed)
        .route_definitions
        .iter()
        .any(|route| route.route_id == "northwind.settings-page.settings"
            && route.path == "/settings/northwind.settings-page"));
    // Page access never grants a core operation or an invented plugin API.
    assert!(access_control::ensure_permission(&allowed, "system.settings.view").is_err());
    let mut with_api = parse_host_extension_contribution_manifest(&fixture_manifest()).unwrap();
    with_api.settings_features[0]
        .api_routes
        .push(access_control::SettingsApiRoute {
            method: "GET".into(),
            path: "/api/console/northwind.settings-page/settings".into(),
        });
    assert!(resolve_linked_host_extension_console_contribution(
        with_api,
        linked_host_console_route_sources()
    )
    .is_err());
}

fn fixture_manifest() -> String {
    include_str!("../../../../../plugins/fixtures/northwind.settings-page/host-extension.yaml")
        .to_string()
}

#[test]
fn root_2014_ac_009_reject_foreign_and_mismatched_page_declarations() {
    let raw = fixture_manifest();
    let foreign = raw.replace(
        "feature_id: northwind.settings-page.settings",
        "feature_id: other.settings",
    );
    assert!(parse_host_extension_contribution_manifest(&foreign).is_err());
    let traversal = raw.replace("source_file: settings.tsx", "source_file: ../settings.tsx");
    assert!(parse_host_extension_contribution_manifest(&traversal).is_err());
    let native = parse_host_extension_contribution_manifest(&raw).unwrap();
    let mut package = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../../plugins/fixtures/northwind.settings-page/manifest.yaml"
    ))
    .unwrap();
    package.settings_pages[0].contribution_code = "other".to_string();
    assert!(native.validate_package_settings_pages(&package).is_err());
    assert!(
        crate::console_surface_registry::ConsoleSurfaceRegistry::from_host_extension_contributions(
            [&native, &native]
        )
        .is_err()
    );
}
