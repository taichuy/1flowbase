use uuid::Uuid;

pub(crate) fn write_test_provider_package() -> String {
    use std::fs;

    let root = std::env::temp_dir().join(format!("1flowbase-provider-fixture-{}", Uuid::now_v7()));
    fs::create_dir_all(root.join("provider")).expect("create fixture provider dir");
    fs::create_dir_all(root.join("bin")).expect("create fixture runtime dir");
    fs::create_dir_all(root.join("models/llm")).expect("create fixture models dir");
    fs::create_dir_all(root.join("i18n")).expect("create fixture i18n dir");
    fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Provider
description: Fixture Provider
icon: icon.svg
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
  entry: bin/fixture_provider-provider
"#,
    )
    .expect("write manifest");
    fs::write(
        root.join("provider/fixture_provider.yaml"),
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
supports_model_fetch_without_credentials: true
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
  - key: validate_model
    type: boolean
    required: false
"#,
    )
    .expect("write provider yaml");
    let runtime_path = root.join("bin/fixture_provider-provider");
    fs::write(
        &runtime_path,
        r#"#!/usr/bin/env node
const fs = require('node:fs');

const request = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');

let result = {};
switch (request.method) {
  case 'validate':
    result = {
      sanitized: {
        api_key: request.input?.api_key ? "***" : null
      }
    };
    break;
  case 'list_models':
    result = [
      {
        model_id: "gpt-5.4-mini",
        display_name: "GPT-5.4 Mini",
        source: "dynamic",
        supports_streaming: true,
        supports_tool_call: true,
        supports_multimodal: false,
        provider_metadata: {
          tier: "default"
        }
      }
    ];
    break;
  case 'invoke': {
    const query = request.input?.messages?.[0]?.content ?? "";
    result = {
      events: [
        { type: "text_delta", delta: "reply:" + query },
        { type: "usage_snapshot", usage: { input_tokens: 5, output_tokens: 7, total_tokens: 12 } },
        { type: "finish", reason: "stop" }
      ],
      result: {
        final_content: "reply:" + query,
        usage: { input_tokens: 5, output_tokens: 7, total_tokens: 12 },
        finish_reason: "stop"
      }
    };
    break;
  }
  default:
    result = {};
}

process.stdout.write(JSON.stringify({ ok: true, result }));
"#,
    )
    .expect("write runtime");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&runtime_path)
            .expect("read runtime permissions")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&runtime_path, permissions).expect("mark runtime executable");
    }
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_chat\n",
    )
    .expect("write position");
    fs::write(
        root.join("models/llm/fixture_chat.yaml"),
        r#"model: gpt-5.4-mini
label: GPT-5.4 Mini
family: llm
capabilities:
  - stream
  - tool_call
context_window: 128000
max_output_tokens: 4096
provider_metadata:
  tier: default
"#,
    )
    .expect("write model");
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{
  "plugin": {
    "label": "Fixture Provider",
    "description": "Fixture provider"
  },
  "provider": {
    "label": "Fixture Provider"
  }
}"#,
    )
    .expect("write i18n");

    root.to_string_lossy().to_string()
}

pub(crate) fn write_test_capability_package() -> String {
    use std::fs;

    let root =
        std::env::temp_dir().join(format!("1flowbase-capability-fixture-{}", Uuid::now_v7()));
    fs::create_dir_all(root.join("bin")).expect("create fixture runtime dir");
    fs::create_dir_all(root.join("i18n")).expect("create fixture i18n dir");
    fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: fixture_capability@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Capability
description: Fixture Capability
icon: icon.svg
source_kind: uploaded
trust_level: unverified
consumption_kind: capability_plugin
execution_mode: process_per_call
slot_codes:
  - node_contribution
binding_targets:
  - workspace
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
  entry: bin/fixture_capability
  limits:
    memory_bytes: 134217728
    timeout_ms: 5000
node_contributions:
  - contribution_code: fixture_action
    node_shell: action
    category: automation
    title: Fixture Action
    description: Fixture capability node
    icon: puzzle
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: result
          title: Result
          valueType: json
    side_effect_policy: external_read
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
    .expect("write manifest");
    fs::write(
        root.join("bin/fixture_capability"),
        r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${payload}" in
  *'"method":"execute"'*)
    printf '%s' '{"ok":true,"result":{"answer":"world"}}'
    ;;
  *'"method":"validate_config"'*)
    printf '%s' '{"ok":true,"result":{"ok":true}}'
    ;;
  *'"method":"resolve_dynamic_options"'*)
    printf '%s' '{"ok":true,"result":{"fields":[]}}'
    ;;
  *'"method":"resolve_output_schema"'*)
    printf '%s' '{"ok":true,"result":{"schema_version":"1flowbase.capability.output/v1"}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"message":"unknown method"}}'
    exit 1
    ;;
esac
"#,
    )
    .expect("write runtime");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(root.join("bin/fixture_capability"))
            .expect("read runtime permissions")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(root.join("bin/fixture_capability"), permissions)
            .expect("mark runtime executable");
    }
    fs::write(root.join("i18n/en_US.json"), "{}").expect("write i18n");

    root.to_string_lossy().to_string()
}
