use std::sync::Arc;

use access_control::{ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteOwnership};
use axum::http::StatusCode;
use plugin_framework::parse_host_extension_contribution_manifest;

use crate::{
    app_state::{compile_console_boot_plan_with_interface_operations, ApiState},
    host_extensions::console::{
        resolve_linked_host_extension_console_contribution, LinkedHostConsoleRouteSource,
    },
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[test]
fn root_2014_ac_009_registered_settings_page_authority() {
    let contribution = parse_host_extension_contribution_manifest(&fixture_manifest())
        .expect("fixture HostExtension manifest should be valid");
    let source = LinkedHostConsoleRouteSource {
        extension_id: "northwind.settings-page",
        version: "1.0.0",
        route_assembly: fixture_host_console_route_assembly,
    };

    let host = resolve_linked_host_extension_console_contribution(contribution, &[source])
        .expect("active linked HostExtension contribution should resolve");
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

    let access = plan
        .console_operation_registry
        .access_for_console_route("GET", "/api/console/northwind.settings-page/scans")
        .expect("linked HostExtension operation must be registered");
    assert_eq!(access.operation_id, "northwind.settings-page.scan");
    assert_eq!(
        access.authorization,
        &ConsoleAuthorization::ResourceAction {
            resource_code: "northwind.settings-page.scans".to_string(),
            action_code: "view".to_string(),
        }
    );
    assert_eq!(
        access.policy_group,
        &ConsolePolicyGroup::SettingsFeature("northwind.settings-page.settings".to_string())
    );
    assert!(plan.route_assembly.bindings().iter().any(|binding| {
        binding.route.method == "GET"
            && binding.route.path == "/api/console/northwind.settings-page/scans"
            && binding.ownership
                == ConsoleRouteOwnership::ConsoleOperation(
                    "northwind.settings-page.scan".to_string(),
                )
    }));
    let interface = plan
        .console_operation_registry
        .inventory()
        .interfaces
        .iter()
        .find(|interface| {
            interface.authorization_operation_id.as_deref() == Some("northwind.settings-page.scan")
        })
        .expect("linked HostExtension route must have static interface metadata");
    assert!(!interface.summary.trim().is_empty());
    assert!(!interface.description.trim().is_empty());
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
    // A visible page does not confer its single-interface operation permission.
    assert!(access_control::ensure_permission(&allowed, access.operation_id).is_err());
}
fn fixture_host_console_route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/northwind.settings-page/settings",
            console_get(
                fixture_host_settings,
                ConsoleRouteOwnership::ConsoleOperation(
                    "northwind.settings-page.settings.view".to_string(),
                ),
            ),
        )
        .route(
            "/northwind.settings-page/scans",
            console_get(
                fixture_host_scan,
                ConsoleRouteOwnership::ConsoleOperation("northwind.settings-page.scan".to_string()),
            ),
        )
}

async fn fixture_host_settings() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn fixture_host_scan() -> StatusCode {
    StatusCode::NO_CONTENT
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
