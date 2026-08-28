use crate::parse_plugin_manifest;

fn manifest(slot: &str, capability: &str) -> String {
    format!(
        r#"
manifest_version: 1
plugin_id: session_retry_distribution@1.0.0
version: 1.0.0
publisher_namespace: taichuy
vendor: Taichuy
display_name: Session Retry Distribution
description: Session affinity fixture
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes: [{slot}]
binding_targets: [workspace]
selection_mode: assignment_then_select
minimum_host_version: 0.4.1
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
  entry: bin/session-retry-distribution
  capabilities: [{capability}]
  limits: {{}}
data_models:
  - contribution_version: 1flowbase.plugin-data-model/v1
    storage_binding: main
    owned_collections:
      - collection_code: affinity
        fields:
          - field_code: conversation_id
            field_type: string
            nullable: false
provider_distribution_rules:
  - rule_id: "@taichuy/session_retry"
    rule_version: 1.0.0
    contract_version: 1flowbase.provider-distribution-rule/v1
    display_name: Session retry
    handler: select
    required_permissions: [plugin_data.read, plugin_data.write]
    config_fields: {{}}
"#
    )
}

#[test]
fn drs_001_distribution_manifest_parses_exclusive_typed_contribution() {
    let parsed = parse_plugin_manifest(&manifest(
        "provider_distribution_rule",
        "runtime_host_call/v1",
    ))
    .unwrap();
    assert_eq!(parsed.provider_distribution_rules.len(), 1);
    assert_eq!(
        parsed.provider_distribution_rules[0].rule_id,
        "@taichuy/session_retry"
    );
}

#[test]
fn drs_002_wrong_slot_and_unapproved_capability_fail_closed() {
    assert!(parse_plugin_manifest(&manifest("model_provider", "runtime_host_call/v1")).is_err());
    assert!(parse_plugin_manifest(&manifest("provider_distribution_rule", "network")).is_err());
}
