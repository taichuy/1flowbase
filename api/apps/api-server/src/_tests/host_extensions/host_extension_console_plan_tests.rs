use std::sync::Arc;

use access_control::{ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteOwnership};
use axum::http::StatusCode;
use plugin_framework::parse_host_extension_contribution_manifest;

use crate::{
    app_state::{ApiState, compile_console_boot_plan},
    host_extensions::console::{
        LinkedHostConsoleRouteSource, resolve_linked_host_extension_console_contribution,
    },
    routes::console_route_assembly::{ConsoleRouteAssembly, console_get},
};

#[test]
fn ac_001_active_linked_host_console_contribution_is_compiled_and_mounted() {
    let contribution = parse_host_extension_contribution_manifest(&fixture_manifest())
        .expect("fixture HostExtension manifest should be valid");
    let source = LinkedHostConsoleRouteSource {
        extension_id: "fixture-host",
        version: "0.1.0",
        route_assembly: fixture_host_console_route_assembly,
    };

    let host = resolve_linked_host_extension_console_contribution(contribution, &[source])
        .expect("active linked HostExtension contribution should resolve");
    let plan = compile_console_boot_plan([host])
        .expect("Core and active linked HostExtension should compile as one console plan");

    let access = plan
        .console_operation_registry
        .access_for_console_route("GET", "/api/console/fixture-host/scans")
        .expect("linked HostExtension operation must be registered");
    assert_eq!(access.operation_id, "fixture-host.scan");
    assert_eq!(
        access.authorization,
        &ConsoleAuthorization::ResourceAction {
            resource_code: "fixture-host.scans".to_string(),
            action_code: "view".to_string(),
        }
    );
    assert_eq!(
        access.policy_group,
        &ConsolePolicyGroup::SettingsFeature("fixture-host.settings".to_string())
    );
    assert!(plan.route_assembly.bindings().iter().any(|binding| {
        binding.route.method == "GET"
            && binding.route.path == "/api/console/fixture-host/scans"
            && binding.ownership
                == ConsoleRouteOwnership::ConsoleOperation("fixture-host.scan".to_string())
    }));
    assert_eq!(
        plan.console_operation_registry
            .inventory()
            .locale_catalog
            .as_ref()
            .and_then(
                |catalog| catalog.text("zh_Hans", "fixture-host.console.operations.scan.label")
            ),
        Some("扫描记录")
    );
}

#[test]
fn ac_002_unlinked_host_console_contribution_fails_before_route_registration() {
    let contribution = parse_host_extension_contribution_manifest(&fixture_manifest())
        .expect("fixture HostExtension manifest should be valid");

    let error = resolve_linked_host_extension_console_contribution(contribution, &[])
        .err()
        .expect("a HostExtension console contract must have an exact linked route source");

    assert!(
        error
            .to_string()
            .contains("has no linked console route source")
    );
}

#[test]
fn ac_002_duplicate_host_settings_feature_cannot_compile_a_console_plan() {
    let first = parse_host_extension_contribution_manifest(&fixture_manifest())
        .expect("fixture HostExtension manifest should be valid");
    let second = parse_host_extension_contribution_manifest(&fixture_manifest())
        .expect("fixture HostExtension manifest should be valid");
    let source = LinkedHostConsoleRouteSource {
        extension_id: "fixture-host",
        version: "0.1.0",
        route_assembly: fixture_host_console_route_assembly,
    };

    let first = resolve_linked_host_extension_console_contribution(first, &[source])
        .expect("the first contribution should resolve");
    let second = resolve_linked_host_extension_console_contribution(second, &[source])
        .expect("the duplicate candidate should resolve before plan validation");
    let error = compile_console_boot_plan([first, second])
        .err()
        .expect("duplicate HostExtension feature must reject the whole boot plan");

    assert!(
        error
            .to_string()
            .contains("duplicate feature_id fixture-host.settings")
    );
}

fn fixture_host_console_route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/fixture-host/settings",
            console_get(
                fixture_host_settings,
                ConsoleRouteOwnership::ConsoleOperation(
                    "settings_feature.access.fixture-host.settings".to_string(),
                ),
            ),
        )
        .route(
            "/fixture-host/scans",
            console_get(
                fixture_host_scan,
                ConsoleRouteOwnership::ConsoleOperation("fixture-host.scan".to_string()),
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
    r#"
schema_version: 1flowbase.host-extension/v1
extension_id: fixture-host
version: 0.1.0
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: builtin://fixture-host
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
settings_features:
  - feature_id: fixture-host.settings
    owner:
      kind: host_extension
      owner_id: fixture-host
      version: 0.1.0
    lifecycle: active
    console_surface:
      route_id: fixture-host.settings
      surface_key: fixture-host.settings
      path: /settings/fixture-host
      label_key: fixture-host.console.settings.label
      description_key: fixture-host.console.settings.description
      order: 100
    api_routes:
      - method: GET
        path: /api/console/fixture-host/settings
console_operations:
  - operation_id: fixture-host.scan
    owner:
      kind: host_extension
      owner_id: fixture-host
      version: 0.1.0
    lifecycle: active
    policy_group: !settings_feature fixture-host.settings
    label_ref: fixture-host.console.operations.scan.label
    description_ref: fixture-host.console.operations.scan.description
    order: 110
    routes:
      - method: GET
        path: /api/console/fixture-host/scans
    authorization:
      kind: resource_action
      resource_code: fixture-host.scans
      action_code: view
console_resources:
  - resource_code: fixture-host.scans
    owner:
      kind: host_extension
      owner_id: fixture-host
      version: 0.1.0
    lifecycle: active
    scope_kind: workspace
    identity_field: id
    scope_field: scope_id
    owner_field: created_by
    label_ref: fixture-host.console.resources.scans.label
    description_ref: fixture-host.console.resources.scans.description
    actions:
      - action_code: view
        label_ref: fixture-host.console.resources.scans.actions.view.label
        description_ref: fixture-host.console.resources.scans.actions.view.description
console_locale_catalog:
  texts:
    - reference: fixture-host.console.settings.label
      en_us: Fixture settings
      zh_hans: 示例设置
    - reference: fixture-host.console.settings.description
      en_us: Manage fixture settings
      zh_hans: 管理示例设置
    - reference: fixture-host.console.operations.scan.label
      en_us: Scan records
      zh_hans: 扫描记录
    - reference: fixture-host.console.operations.scan.description
      en_us: Read fixture scan records
      zh_hans: 读取示例扫描记录
    - reference: fixture-host.console.resources.scans.label
      en_us: Scan records
      zh_hans: 扫描记录
    - reference: fixture-host.console.resources.scans.description
      en_us: Fixture records available for scanning
      zh_hans: 可供扫描的示例记录
    - reference: fixture-host.console.resources.scans.actions.view.label
      en_us: View scan records
      zh_hans: 查看扫描记录
    - reference: fixture-host.console.resources.scans.actions.view.description
      en_us: Read a fixture scan record
      zh_hans: 读取示例扫描记录
  policy_groups: []
console_surfaces: {}
routes: []
workers: []
migrations: []
"#
    .to_string()
}
