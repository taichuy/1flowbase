use crate::{parse_plugin_manifest, PluginDataFieldType, PluginStorageBinding};

fn manifest(storage_permission: &str, nullable: bool) -> String {
    format!(
        r#"
manifest_version: 1
plugin_id: managed_data@1.0.0
version: 1.0.0
publisher_namespace: example
vendor: Example
display_name: Managed Data
description: Managed data fixture
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes: [model_provider]
binding_targets: [workspace]
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: {storage_permission}
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/managed-data
  limits: {{}}
data_models:
  - contribution_version: 1flowbase.plugin-data-model/v1
    storage_binding: main
    owned_collections:
      - collection_code: affinity_bindings
        fields:
          - field_code: conversation_id
            field_type: uuid
            nullable: false
    extension_fields:
      - target_table: model_provider_instances
        field_code: affinity_hint
        field_type: string
        nullable: {nullable}
"#
    )
}

#[test]
fn pdm_001_manifest_parses_the_single_data_model_desired_state() {
    let parsed = parse_plugin_manifest(&manifest("host_managed", true)).unwrap();
    assert_eq!(parsed.data_models.len(), 1);
    assert_eq!(
        parsed.data_models[0].storage_binding,
        PluginStorageBinding::Main
    );
    assert_eq!(
        parsed.data_models[0].owned_collections[0].fields[0].field_type,
        PluginDataFieldType::Uuid
    );
}

#[test]
fn pdm_007_manifest_rejects_missing_permission_and_non_additive_field() {
    assert!(parse_plugin_manifest(&manifest("none", true))
        .unwrap_err()
        .to_string()
        .contains("permissions.storage=host_managed"));
    assert!(parse_plugin_manifest(&manifest("host_managed", false))
        .unwrap_err()
        .to_string()
        .contains("must be nullable"));
}
