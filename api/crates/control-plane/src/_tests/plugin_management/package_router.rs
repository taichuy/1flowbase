use control_plane::plugin_management::{route_plugin_package, RoutedPluginPackageKind};
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
