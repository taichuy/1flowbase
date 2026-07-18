use super::*;

#[test]
fn plugin_manifest_v1_defaults_js_dependency_permissions_to_none() {
    let manifest = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: js_default_permissions_pack@0.1.0
version: 0.1.0
vendor: acme
display_name: JS Default Permissions Pack
description: Example JS dependency pack plugin
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - js_dependency_pack
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/js-default-permissions-pack
js_dependencies:
  - alias: zod
    package: zod
    version: 3.24.0
    targets:
      - backend_code
    artifacts:
      backend_code: artifacts/zod.backend.mjs
    integrity: sha256-example
    native_addon: false
    lifecycle_scripts: false
"#,
    )
    .unwrap();

    let permissions = &manifest.js_dependencies[0].permissions;
    assert_eq!(permissions.network, "none");
    assert_eq!(permissions.filesystem, "none");
    assert_eq!(permissions.env, "none");
}

#[test]
fn plugin_manifest_v1_rejects_js_dependency_pack_with_invalid_target() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_js_pack@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad JS Pack
description: invalid target
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - js_dependency_pack
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-js-pack
js_dependencies:
  - alias: bad
    package: bad-lib
    version: 1.0.0
    targets:
      - browser
    artifacts:
      browser: artifacts/bad.browser.js
    integrity: sha256-example
    permissions:
      network: deny
      filesystem: deny
      env: deny
    native_addon: false
    lifecycle_scripts: false
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("js_dependencies[].targets[] must be one of backend_code"));
}

#[test]
fn plugin_manifest_v1_rejects_js_dependency_pack_with_native_addon_or_lifecycle_scripts() {
    let native_addon_error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: native_addon_pack@0.1.0
version: 0.1.0
vendor: acme
display_name: Native Addon Pack
description: unsupported
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - js_dependency_pack
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/native-addon-pack
js_dependencies:
  - alias: has_native
    package: native-lib
    version: 1.0.0
    targets:
      - backend_code
    artifacts:
      backend_code: artifacts/native-lib.mjs
    integrity: sha256-native
    permissions:
      network: deny
      filesystem: deny
      env: deny
    native_addon: true
    lifecycle_scripts: false
"#,
    )
    .unwrap_err();

    assert!(native_addon_error
        .to_string()
        .contains("does not support native_addon"));

    let lifecycle_error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: lifecycle_pack@0.1.0
version: 0.1.0
vendor: acme
display_name: Lifecycle Scripts Pack
description: unsupported
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - js_dependency_pack
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/lifecycle-pack
js_dependencies:
  - alias: has_lifecycle
    package: lifecycle-lib
    version: 1.0.0
    targets:
      - backend_code
    artifacts:
      backend_code: artifacts/lifecycle-lib.mjs
    integrity: sha256-lifecycle
    permissions:
      network: deny
      filesystem: deny
      env: deny
    native_addon: false
    lifecycle_scripts: true
"#,
    )
    .unwrap_err();

    assert!(lifecycle_error
        .to_string()
        .contains("does not support lifecycle_scripts"));
}

#[test]
fn plugin_manifest_v1_rejects_manifest_version_other_than_one() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 2
plugin_id: bad_version@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Version
description: invalid
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-version
node_contributions: []
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("manifest_version must be 1"));
}

#[test]
fn plugin_manifest_v1_rejects_runtime_extension_without_supported_binding_target() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_binding@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Binding
description: invalid
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets: []
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-binding
node_contributions: []
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("runtime_extension binding_targets must only contain workspace or model"));
}

#[test]
fn plugin_manifest_v1_rejects_capability_plugin_with_incomplete_node_contribution() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_node@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Node
description: invalid
source_kind: uploaded
trust_level: unverified
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - node_contribution
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-node
node_contributions:
  - contribution_code: ""
    node_shell: action
    category: http
    title: Broken Node
    description: Broken node description
    icon: globe
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: result
          title: Result
          valueType: json
    side_effect_policy: none
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: public
    experimental: false
    dependency:
      installation_kind: optional
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("node_contributions[].contribution_code cannot be empty"));
}

#[test]
fn plugin_manifest_v1_rejects_runtime_extension_with_tenant_binding_target() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_tenant_binding@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Tenant Binding
description: invalid
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
  - tenant
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-tenant-binding
node_contributions: []
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("runtime_extension binding_targets must only contain workspace or model"));
}

#[test]
fn plugin_manifest_v1_rejects_capability_plugin_with_invalid_node_contract_values() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_node_contract@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Node Contract
description: invalid
source_kind: uploaded
trust_level: unverified
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - node_contribution
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-node-contract
node_contributions:
  - contribution_code: bad_contract
    node_shell: control_flow
    category: http
    title: Broken Node
    description: Broken node description
    icon: globe
    schema_ui: {}
    schema_version: invalid-schema
    output_schema:
      outputs:
        - key: result
          title: Result
          valueType: json
    side_effect_policy: none
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: internal
    experimental: false
    dependency:
      installation_kind: dynamic
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("node_contributions[].node_shell must be one of action"));
}

#[test]
fn plugin_manifest_v1_rejects_unknown_source_kind() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_source@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Source
description: invalid
source_kind: side_loaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-source
node_contributions: []
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("source_kind must be one of builtin, official_registry, mirror_registry, uploaded, filesystem_dropin"));
}

#[test]
fn plugin_manifest_v1_rejects_contract_version_mismatch_for_capability_plugin() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_contract_version@0.1.0
version: 0.1.0
vendor: acme
display_name: Bad Contract Version
description: invalid
source_kind: uploaded
trust_level: unverified
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - node_contribution
binding_targets: []
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-contract-version
node_contributions:
  - contribution_code: valid_node
    node_shell: action
    category: http
    title: Valid Node
    description: Valid node description
    icon: globe
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: result
          title: Result
          valueType: json
    side_effect_policy: none
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: public
    experimental: false
    dependency:
      installation_kind: optional
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("contract_version must be 1flowbase.capability/v1 for capability_plugin"));
}
