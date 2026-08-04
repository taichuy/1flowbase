use super::*;

#[test]
fn plugin_manifest_v1_accepts_node_contribution_v2_contract() {
    let manifest = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: prompt_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Prompt Pack
description: Prompt capability plugin
source_kind: uploaded
trust_level: checksum_only
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
  entry: bin/prompt-pack
node_contributions:
  - contribution_code: openai_prompt
    node_shell: action
    category: ai
    title: OpenAI Prompt
    description: Prompt node
    icon: spark
    schema_ui:
      sections:
        - blocks:
            - kind: field
              renderer: text
              path: config.prompt
              label: Prompt
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: answer
          title: Answer
          valueType: string
    side_effect_policy: external_read
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap();

    let contribution = &manifest.node_contributions[0];
    assert_eq!(
        contribution.schema_version,
        "1flowbase.node-contribution/v2"
    );
    assert_eq!(contribution.side_effect_policy, "external_read");
}

#[test]
fn plugin_manifest_v1_rejects_node_contribution_v1_schema() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: prompt_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Prompt Pack
description: Prompt capability plugin
source_kind: uploaded
trust_level: checksum_only
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
  entry: bin/prompt-pack
node_contributions:
  - contribution_code: legacy_prompt
    node_shell: action
    category: ai
    title: Legacy Prompt
    description: Legacy node
    icon: spark
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v1
    output_schema:
      outputs:
        - key: answer
          title: Answer
          valueType: string
    side_effect_policy: none
    infra_contracts: []
    required_auth: []
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains(
        "node_contributions[].schema_version must be one of 1flowbase.node-contribution/v2"
    ));
}

#[test]
fn plugin_manifest_v1_rejects_unknown_node_contribution_renderer() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: prompt_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Prompt Pack
description: Prompt capability plugin
source_kind: uploaded
trust_level: checksum_only
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
  entry: bin/prompt-pack
node_contributions:
  - contribution_code: openai_prompt
    node_shell: action
    category: ai
    title: OpenAI Prompt
    description: Prompt node
    icon: spark
    schema_ui:
      sections:
        - blocks:
            - kind: field
              renderer: plugin_react_panel
              path: config.prompt
              label: Prompt
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: answer
          title: Answer
          valueType: string
    side_effect_policy: none
    infra_contracts: []
    required_auth: []
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("unknown node contribution renderer"));
}

#[test]
fn plugin_manifest_v1_rejects_reserved_output_and_host_infra_contracts() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: prompt_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Prompt Pack
description: Prompt capability plugin
source_kind: uploaded
trust_level: checksum_only
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
  entry: bin/prompt-pack
node_contributions:
  - contribution_code: openai_prompt
    node_shell: action
    category: ai
    title: OpenAI Prompt
    description: Prompt node
    icon: spark
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: usage
          title: Usage
          valueType: json
    side_effect_policy: none
    infra_contracts:
      - cache-store
    required_auth: []
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("reserved public output key"));
}

#[test]
fn plugin_manifest_v1_rejects_storage_host_infra_contracts() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: prompt_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Prompt Pack
description: Prompt capability plugin
source_kind: uploaded
trust_level: checksum_only
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
  entry: bin/prompt-pack
node_contributions:
  - contribution_code: openai_prompt
    node_shell: action
    category: ai
    title: OpenAI Prompt
    description: Prompt node
    icon: spark
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: answer
          title: Answer
          valueType: string
    side_effect_policy: none
    infra_contracts:
      - storage-object
      - rate_limit_store
    required_auth: []
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("host infrastructure contract"));
}

#[test]
fn plugin_manifest_v1_rejects_node_contribution_output_without_title_or_value_type() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: prompt_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Prompt Pack
description: Prompt capability plugin
source_kind: uploaded
trust_level: checksum_only
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
  entry: bin/prompt-pack
node_contributions:
  - contribution_code: openai_prompt
    node_shell: action
    category: ai
    title: OpenAI Prompt
    description: Prompt node
    icon: spark
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: answer
          title: Answer
        - key: raw
          valueType: json
    side_effect_policy: none
    infra_contracts: []
    required_auth: []
    visibility: public
    experimental: false
    dependency:
      installation_kind: required
      plugin_version_range: ">=0.1.0"
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("output_schema.outputs[].valueType cannot be empty"));
}

#[test]
fn ac_002_model_provider_manifest_accepts_only_current_contract() {
    let raw = r#"
manifest_version: 1
plugin_id: openai_compatible@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: OpenAI Compatible
description: OpenAI-compatible runtime extension
source_kind: official_registry
trust_level: verified_official
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
  entry: bin/openai-compatible-provider
"#;

    let manifest = parse_plugin_manifest(raw).expect("current provider manifest should parse");
    assert_eq!(
        manifest.consumption_kind,
        PluginConsumptionKind::RuntimeExtension
    );
    assert_eq!(manifest.slot_codes, vec!["model_provider"]);

    let legacy_raw = raw.replace("1flowbase.provider/v2", "1flowbase.provider/v1");
    let error = parse_plugin_manifest(&legacy_raw)
        .expect_err("legacy provider contract must be rejected during manifest intake");
    assert!(error
        .to_string()
        .contains("contract_version must be 1flowbase.provider/v2"));
}

#[test]
fn wp_r14a_provider_manifest_accepts_exact_protocol_context_profiles() {
    let raw = r#"
manifest_version: 1
plugin_id: anthropic@0.2.0
version: 0.2.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Anthropic
description: Anthropic provider v2
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.2.6
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
  entry: bin/anthropic-provider
  capabilities:
    - system_prompt_blocks
    - system_prompt_cache_control
    - end_user_reference
    - message_blocks.reasoning_history.v1
    - message_blocks.redacted_reasoning_history.v1
    - count_tokens
    - protocol_context.consume.anthropic_messages.v1
    - protocol_context.restore.openai_chat.v1
    - protocol_context.restore.openai_responses.v1
"#;

    let manifest = parse_plugin_manifest(raw).expect("provider v2 manifest should parse");

    assert_eq!(manifest.contract_version, "1flowbase.provider/v2");
    assert_eq!(
        manifest.runtime.capabilities,
        vec![
            "system_prompt_blocks",
            "system_prompt_cache_control",
            "end_user_reference",
            "message_blocks.reasoning_history.v1",
            "message_blocks.redacted_reasoning_history.v1",
            "count_tokens",
            "protocol_context.consume.anthropic_messages.v1",
            "protocol_context.restore.openai_chat.v1",
            "protocol_context.restore.openai_responses.v1"
        ]
    );

    for invalid in [
        "protocol_context",
        "protocol_context.forward.anthropic_messages.v1",
        "protocol_context.consume.unknown.v1",
        "protocol_context.consume.anthropic_messages.v2",
    ] {
        let invalid_raw = raw.replace("protocol_context.consume.anthropic_messages.v1", invalid);
        let error = parse_plugin_manifest(&invalid_raw)
            .expect_err("coarse or malformed protocol context profiles must be rejected");
        assert!(error.to_string().contains("runtime.capabilities"));
    }

    let conflicting_raw = raw.replace(
        "    - protocol_context.consume.anthropic_messages.v1\n",
        "    - protocol_context.consume.anthropic_messages.v1\n    - protocol_context.restore.anthropic_messages.v1\n",
    );
    let error = parse_plugin_manifest(&conflicting_raw)
        .expect_err("consume and restore must not conflict for one source profile");
    assert!(error
        .to_string()
        .contains("conflicting protocol context profiles"));

    let duplicate_raw = raw.replace(
        "    - protocol_context.consume.anthropic_messages.v1\n",
        "    - protocol_context.consume.anthropic_messages.v1\n    - protocol_context.consume.anthropic_messages.v1\n",
    );
    let error = parse_plugin_manifest(&duplicate_raw)
        .expect_err("duplicate exact protocol context profiles must be rejected");
    assert!(error.to_string().contains("duplicate value"));
}

#[test]
fn provider_v2_manifest_accepts_openai_remote_and_native_capability_rows() {
    let raw = r#"
manifest_version: 1
plugin_id: openai@0.2.13
version: 0.2.13
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: OpenAI
description: OpenAI provider v2
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: stateful_provider_worker
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.2.6
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/openai-provider
  capabilities:
    - compact.responses_compact
    - compact.responses_compaction_v2
    - responses.native_passthrough
    - protocol_context.restore.anthropic_messages.v1
    - protocol_context.consume.openai_chat.v1
    - protocol_context.consume.openai_responses.v1
"#;

    let manifest = parse_plugin_manifest(raw)
        .expect("the declared OpenAI remote and native capabilities should be accepted exactly");

    assert_eq!(
        manifest.runtime.capabilities,
        vec![
            "compact.responses_compact",
            "compact.responses_compaction_v2",
            "responses.native_passthrough",
            "protocol_context.restore.anthropic_messages.v1",
            "protocol_context.consume.openai_chat.v1",
            "protocol_context.consume.openai_responses.v1"
        ]
    );

    let anthropic_source_request_v2 = raw.replace(
        "protocol_context.restore.anthropic_messages.v1",
        "protocol_context.restore.anthropic_messages.v2",
    );
    parse_plugin_manifest(&anthropic_source_request_v2)
        .expect("Anthropic SourceProtocolContext restore v2 must be an exact capability");

    let conflicting_restore_versions = raw.replace(
        "    - protocol_context.restore.anthropic_messages.v1\n",
        "    - protocol_context.restore.anthropic_messages.v1\n    - protocol_context.restore.anthropic_messages.v2\n",
    );
    let error = parse_plugin_manifest(&conflicting_restore_versions)
        .expect_err("one provider must not declare both Anthropic restore ABI versions");
    assert!(error
        .to_string()
        .contains("conflicting protocol context profiles"));
}

#[test]
fn runtime_extension_rejects_provider_as_plugin_type_slot() {
    let raw = r#"
manifest_version: 1
plugin_id: legacy_provider@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Legacy Provider
description: Legacy provider vocabulary
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - provider
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
  entry: bin/legacy-provider
"#;

    let error = parse_plugin_manifest(raw).expect_err("provider is not a runtime slot");
    assert!(error.to_string().contains("slot_codes"));
}

#[test]
fn runtime_extension_accepts_data_import_snapshot_slot() {
    let raw = r#"
manifest_version: 1
plugin_id: snapshot_importer@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Snapshot Importer
description: Data import snapshot runtime extension
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - data_import_snapshot
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.data_source/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/snapshot-importer
"#;

    let manifest = parse_plugin_manifest(raw).expect("manifest should parse");
    assert_eq!(manifest.slot_codes, vec!["data_import_snapshot"]);
    assert_eq!(manifest.contract_version, "1flowbase.data_source/v1");
}
