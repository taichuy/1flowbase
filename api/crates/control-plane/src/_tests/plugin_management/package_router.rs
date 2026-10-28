use control_plane::plugin_management::{
    catalog_plugin_type, route_plugin_package, RoutedPluginPackageKind,
};
use plugin_framework::parse_plugin_manifest;

fn manifest_with_slot(slot: &str, contract_version: &str) -> String {
    format!(
        r#"
manifest_version: 1
plugin_id: fixture@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Fixture
description: Fixture runtime extension
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - {slot}
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: {contract_version}
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture
"#
    )
}

fn provider_distribution_manifest() -> &'static str {
    r#"
manifest_version: 1
plugin_id: distribution_fixture
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Distribution Fixture
description: Distribution fixture
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes: [provider_distribution_rule]
binding_targets: [workspace]
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider-distribution-rule/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: host_managed
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/distribution-fixture
  capabilities: [runtime_host_call/v1]
  limits: {}
provider_distribution_rules:
  - rule_id: "@acme/fixture"
    rule_version: 0.1.0
    contract_version: 1flowbase.provider-distribution-rule/v1
    display_name: Fixture
    handler: select
    required_permissions: []
    config_fields: {}
"#
}

#[test]
fn router_detects_model_provider_runtime_extension() {
    let raw = manifest_with_slot("model_provider", "1flowbase.provider/v2");
    let manifest = parse_plugin_manifest(&raw).expect("manifest should parse");
    assert_eq!(
        route_plugin_package(&manifest).expect("should route"),
        RoutedPluginPackageKind::ModelProviderRuntime
    );
}

#[test]
fn router_detects_data_source_runtime_extension() {
    let raw = manifest_with_slot("data_source", "1flowbase.data_source/v1");
    let manifest = parse_plugin_manifest(&raw).expect("manifest should parse");
    assert_eq!(
        route_plugin_package(&manifest).expect("should route"),
        RoutedPluginPackageKind::DataSourceRuntime
    );
}

#[test]
fn router_detects_provider_distribution_rule_runtime_extension() {
    let manifest =
        parse_plugin_manifest(provider_distribution_manifest()).expect("manifest should parse");
    assert_eq!(
        route_plugin_package(&manifest).expect("should route"),
        RoutedPluginPackageKind::ProviderDistributionRuleRuntime
    );
}

#[test]
fn router_rejects_conflicting_runtime_slots() {
    let raw = manifest_with_slot("model_provider", "1flowbase.provider/v2");
    let mut manifest = parse_plugin_manifest(&raw).expect("manifest should parse");
    manifest
        .slot_codes
        .push("provider_distribution_rule".to_string());
    let error = route_plugin_package(&manifest).expect_err("conflicting slots must fail closed");
    assert!(error.to_string().contains("runtime_slot"));
}

// Root #2007 AC-001/AC-002: v2 routes through the real package router, v1 remains unchanged.
#[test]
fn root_2007_ac_001_002_manifest_routes() {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../extension-package-runtime/src/_tests/managed_manifest.yaml"
    ));
    let manifest = parse_plugin_manifest(raw).expect("managed fixture should parse");
    assert_eq!(
        route_plugin_package(&manifest).expect("managed route"),
        RoutedPluginPackageKind::ManagedContributions
    );
    let capability = raw
        .replace(
            "consumption_kind: runtime_extension",
            "consumption_kind: capability_plugin",
        )
        .replace("module_kind: runtime", "module_kind: capability");
    assert_eq!(
        route_plugin_package(
            &parse_plugin_manifest(&capability).expect("capability may execute contributions")
        )
        .expect("managed route"),
        RoutedPluginPackageKind::ManagedContributions
    );
    let legacy = parse_plugin_manifest(&manifest_with_slot(
        "model_provider",
        "1flowbase.provider/v2",
    ))
    .expect("legacy package");
    assert_eq!(
        route_plugin_package(&legacy).expect("legacy route"),
        RoutedPluginPackageKind::ModelProviderRuntime
    );
}

#[test]
fn catalog_types_preserve_managed_consumption_without_changing_install_routes() {
    let runtime = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../extension-package-runtime/src/_tests/managed_manifest.yaml"
    ));
    for (raw, expected) in [
        (runtime.to_owned(), "runtime_extension"),
        (
            runtime
                .replace(
                    "consumption_kind: runtime_extension",
                    "consumption_kind: capability_plugin",
                )
                .replace("module_kind: runtime", "module_kind: capability"),
            "capability_plugin",
        ),
    ] {
        let manifest = parse_plugin_manifest(&raw).unwrap();
        assert_eq!(catalog_plugin_type(&manifest).unwrap(), expected);
        assert_eq!(
            route_plugin_package(&manifest).unwrap(),
            RoutedPluginPackageKind::ManagedContributions
        );
        assert_eq!(
            route_plugin_package(&manifest).unwrap().as_plugin_type(),
            "managed_contributions"
        );
    }
    let host = parse_plugin_manifest(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins/host-extensions/official.identity-host/manifest.yaml"
    )))
    .unwrap();
    assert_eq!(catalog_plugin_type(&host).unwrap(), "host_extension");
    assert_eq!(
        route_plugin_package(&host).unwrap(),
        RoutedPluginPackageKind::HostExtension
    );
}

#[test]
fn catalog_types_preserve_legacy_slots_and_reject_invalid_routes() {
    for (slot, contract, expected) in [
        ("model_provider", "1flowbase.provider/v2", "model_provider"),
        ("data_source", "1flowbase.data_source/v1", "data_source"),
    ] {
        let mut manifest = parse_plugin_manifest(&manifest_with_slot(slot, contract)).unwrap();
        assert_eq!(catalog_plugin_type(&manifest).unwrap(), expected);
        manifest
            .slot_codes
            .push("provider_distribution_rule".into());
        assert!(catalog_plugin_type(&manifest)
            .unwrap_err()
            .to_string()
            .contains("runtime_slot"));
    }
    let distribution = parse_plugin_manifest(provider_distribution_manifest()).unwrap();
    assert_eq!(
        catalog_plugin_type(&distribution).unwrap(),
        "provider_distribution_rule"
    );
    let mut managed = parse_plugin_manifest(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../extension-package-runtime/src/_tests/managed_manifest.yaml"
    )))
    .unwrap();
    managed.managed = None;
    assert!(catalog_plugin_type(&managed)
        .unwrap_err()
        .to_string()
        .contains("managed_manifest"));
}
