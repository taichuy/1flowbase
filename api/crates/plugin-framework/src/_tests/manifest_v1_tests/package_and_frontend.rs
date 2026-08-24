use super::*;
use sha2::{Digest, Sha256};

#[test]
fn plugin_manifest_v1_parses_runtime_extension_provider_fields() {
    let raw = r#"
manifest_version: 1
plugin_id: openai_compatible@0.4.0
version: 0.4.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: OpenAI Compatible
description: Generic OpenAI-compatible provider runtime extension
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
  entry: bin/openai-compatible-provider
  limits:
    timeout_ms: 30000
    invoke_timeout_ms: 300000
    memory_bytes: 268435456
node_contributions: []
"#;
    let manifest = parse_plugin_manifest(raw).unwrap();

    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.plugin_id, "openai_compatible@0.4.0");
    assert_eq!(manifest.version, "0.4.0");
    assert_eq!(manifest.publisher_namespace, "1flowbase");
    assert_eq!(manifest.vendor, "1flowbase");
    assert!(manifest.keywords.is_empty());
    assert_eq!(manifest.display_name, "OpenAI Compatible");
    assert_eq!(
        manifest.consumption_kind,
        PluginConsumptionKind::RuntimeExtension
    );
    assert_eq!(manifest.execution_mode, PluginExecutionMode::ProcessPerCall);
    assert_eq!(manifest.consumption_kind.as_str(), "runtime_extension");
    assert_eq!(manifest.execution_mode.as_str(), "process_per_call");
    assert_eq!(manifest.slot_codes, vec!["model_provider"]);
    assert_eq!(manifest.binding_targets, vec!["workspace"]);
    assert_eq!(manifest.runtime.protocol, "stdio_json");
    assert_eq!(manifest.runtime.entry, "bin/openai-compatible-provider");
    assert_eq!(manifest.runtime.limits.timeout_ms, Some(30000));
    assert_eq!(manifest.runtime.limits.invoke_timeout_ms, Some(300000));
    assert_eq!(manifest.runtime.limits.memory_bytes, Some(268435456));
    assert!(manifest.node_contributions.is_empty());

    let missing_publisher =
        parse_plugin_manifest(&raw.replace("publisher_namespace: 1flowbase\n", "")).unwrap_err();
    assert!(missing_publisher
        .to_string()
        .contains("publisher_namespace"));
}

#[test]
fn publisher_cutover_legacy_installed_manifest_repairs_only_missing_namespace_in_memory() {
    let current = r#"manifest_version: 1
plugin_id: fixture@1.2.3
version: 1.2.3
publisher_namespace: fixture_org
vendor: Historical Vendor
display_name: Fixture
description: Legacy installed fixture
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes: [model_provider]
binding_targets: [workspace]
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/provider
  capabilities: [protocol_context]
node_contributions: []
"#;
    let legacy = current.replace("publisher_namespace: fixture_org\n", "");
    assert!(parse_plugin_manifest(&legacy).is_err());

    let eligibility = LegacyInstalledManifestEligibility {
        expected_publisher_namespace: "fixture_org".to_string(),
        expected_versioned_plugin_id: "fixture@1.2.3".to_string(),
        expected_raw_manifest_fingerprint: format!(
            "sha256:{:x}",
            Sha256::digest(legacy.as_bytes())
        ),
    };
    let parsed = parse_legacy_installed_plugin_manifest(&legacy, &eligibility)
        .expect("AC-001 legacy installed artifact should load from explicit durable identity");
    assert_eq!(parsed.publisher_namespace, "fixture_org");
    assert_eq!(parsed.contract_version, "1flowbase.provider/v1");
    assert_eq!(parsed.vendor, "Historical Vendor");
    assert_eq!(parsed.runtime.capabilities, vec!["protocol_context"]);

    let strict_current = current.replace(
        "contract_version: 1flowbase.provider/v1",
        "contract_version: 1flowbase.provider/v2",
    );
    let strict_error = parse_plugin_manifest(&strict_current).unwrap_err();
    assert!(strict_error.to_string().contains("runtime.capabilities"));

    let wrong_fingerprint = LegacyInstalledManifestEligibility {
        expected_raw_manifest_fingerprint: format!("sha256:{}", "0".repeat(64)),
        ..eligibility.clone()
    };
    assert!(parse_legacy_installed_plugin_manifest(&legacy, &wrong_fingerprint).is_err());

    let also_missing_contract = legacy.replace("contract_version: 1flowbase.provider/v1\n", "");
    let other_missing = LegacyInstalledManifestEligibility {
        expected_raw_manifest_fingerprint: format!(
            "sha256:{:x}",
            Sha256::digest(also_missing_contract.as_bytes())
        ),
        ..eligibility
    };
    assert!(
        parse_legacy_installed_plugin_manifest(&also_missing_contract, &other_missing).is_err()
    );
}

#[test]
fn plugin_manifest_v1_parses_stateful_provider_worker_runtime() {
    let manifest = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: openai@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: OpenAI
description: OpenAI Responses provider runtime extension
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: stateful_provider_worker
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
  protocol: stdio_json_worker
  entry: bin/openai-provider
node_contributions: []
"#,
    )
    .unwrap();

    assert_eq!(
        manifest.execution_mode,
        PluginExecutionMode::StatefulProviderWorker
    );
    assert_eq!(manifest.execution_mode.as_str(), "stateful_provider_worker");
    assert_eq!(manifest.runtime.protocol, "stdio_json_worker");
}

#[test]
fn plugin_manifest_v1_rejects_stateful_provider_worker_with_plain_stdio() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: openai@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: OpenAI
description: invalid
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: stateful_provider_worker
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
  entry: bin/openai-provider
node_contributions: []
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains(
        "stateful_provider_worker execution_mode requires runtime.protocol=stdio_json_worker"
    ));
}

#[test]
fn plugin_manifest_v1_rejects_host_extension_with_workspace_binding() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_host@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Bad Host
description: invalid
source_kind: uploaded
trust_level: unverified
consumption_kind: host_extension
execution_mode: in_process
slot_codes: []
binding_targets:
  - workspace
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: none
  storage: host_managed
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: lib/bad-host.so
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("host_extension cannot declare workspace binding_targets"));
}

#[test]
fn plugin_manifest_v1_rejects_capability_plugin_without_node_contributions() {
    let error = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_capability@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Bad Capability
description: invalid
source_kind: official_registry
trust_level: verified_official
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
  network: outbound_only
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/bad-capability
node_contributions: []
"#,
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("capability_plugin must declare node_contributions"));
}

#[test]
fn plugin_manifest_v1_parses_capability_plugin_with_js_dependency_pack() {
    let manifest = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: js_zod_pack@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: JS Zod Pack
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
  entry: bin/js-zod-pack
js_dependencies:
  - alias: zod
    package: zod
    version: 3.24.0
    targets:
      - backend_code
    artifacts:
      backend_code: artifacts/zod.backend.mjs
    integrity: sha256-example
    permissions:
      network: deny
      filesystem: deny
      env: deny
    native_addon: false
    lifecycle_scripts: false
"#,
    )
    .unwrap();

    assert_eq!(manifest.slot_codes, vec!["js_dependency_pack"]);
    assert_eq!(
        manifest.consumption_kind,
        PluginConsumptionKind::CapabilityPlugin
    );
    assert_eq!(manifest.js_dependencies.len(), 1);
    let dep = &manifest.js_dependencies[0];
    assert_eq!(dep.alias, "zod");
    assert_eq!(dep.package, "zod");
    assert_eq!(dep.version, "3.24.0");
    assert_eq!(dep.targets, vec!["backend_code"]);
    assert_eq!(
        dep.artifacts.get("backend_code"),
        Some(&"artifacts/zod.backend.mjs".to_string())
    );
    assert_eq!(dep.permissions.network, "deny");
    assert_eq!(dep.permissions.filesystem, "deny");
}

#[test]
fn plugin_manifest_v1_accepts_frontend_block_contribution() {
    let manifest = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: fixture_frontend_blocks@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Fixture Frontend Blocks
description: Frontend block contribution plugin
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/fixture-frontend-blocks
block_contributions:
  - contribution_code: hero_banner
    title: Hero Banner
    runtime: native_react
    entry: blocks/hero/index.html
    code_template: |
      export default function HeroBanner() {
        return <section>Hero</section>;
      }
    code_template_version: 1.0.0
    code_template_language: tsx
    code_modules:
      - source: "@1flowbase/block-sdk"
        version: "1.0.0"
        exports: [defineBlock]
        binding: fetched
        assets:
          - path: "assets/block-sdk.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        type_declarations: "export declare function defineBlock(input: unknown): unknown;"
      - source: "@acme/native-components"
        version: "1.2.3"
        exports: [Button]
        binding: fetched
        assets:
          - path: "assets/native-components.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        type_declarations: |
          declare module '@acme/native-components' {}
    context_contract:
      primitives:
        - text
        - image
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities:
      - responsive
      - configurable
"#,
    )
    .unwrap();

    assert_eq!(manifest.block_contributions.len(), 1);
    let block = &manifest.block_contributions[0];
    assert_eq!(block.contribution_code, "hero_banner");
    assert_eq!(block.runtime, "native_react");
    assert_eq!(block.entry, "blocks/hero/index.html");
    assert_eq!(
        block.code_template.as_deref(),
        Some("export default function HeroBanner() {\n  return <section>Hero</section>;\n}\n")
    );
    assert_eq!(block.code_template_version.as_deref(), Some("1.0.0"));
    assert_eq!(block.code_template_language.as_deref(), Some("tsx"));
    assert_eq!(block.code_modules[0].source, "@1flowbase/block-sdk");
    assert_eq!(block.code_modules[0].exports, vec!["defineBlock"]);
    assert_eq!(block.code_modules[1].source, "@acme/native-components");
    assert_eq!(block.code_modules[1].exports, vec!["Button"]);
    assert_eq!(block.context_contract.primitives, vec!["text", "image"]);
    assert_eq!(block.ui_capabilities, vec!["responsive", "configurable"]);
}

#[test]
fn d5_p3_accepts_only_registered_native_and_isolated_frontend_runtimes() {
    let isolated = parse_plugin_manifest(VALID_ISOLATED_FRONTEND_MANIFEST).unwrap();

    assert_eq!(isolated.block_contributions[0].runtime, "isolated_iframe");
}

const VALID_ISOLATED_CODE_MODULES: &str = r#"    code_modules:
      - source: "@acme/isolated-chart"
        version: "1.0.0"
        exports: [default]
        binding: fetched
        assets:
          - path: "assets/isolated-chart.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        type_declarations: "declare const block: unknown; export default block;"
"#;

const VALID_ISOLATED_FRONTEND_MANIFEST: &str = r#"
manifest_version: 1
plugin_id: isolated_frontend_block@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Isolated Frontend Block
description: isolated iframe contribution
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes: [frontend_block]
binding_targets: [workspace]
selection_mode: assignment_then_select
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
  entry: bin/isolated-frontend-block
block_contributions:
  - contribution_code: isolated_chart
    title: Isolated Chart
    runtime: isolated_iframe
    entry: "@acme/isolated-chart"
    code_modules:
      - source: "@acme/isolated-chart"
        version: "1.0.0"
        exports: [default]
        binding: fetched
        assets:
          - path: "assets/isolated-chart.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        type_declarations: "declare const block: unknown; export default block;"
    context_contract:
      primitives: [data_record]
      input_schema: { type: object }
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities: [responsive]
"#;

#[test]
fn d5_p3_isolated_runtime_requires_its_exact_fetched_entry_module() {
    let missing_module = VALID_ISOLATED_FRONTEND_MANIFEST.replace(VALID_ISOLATED_CODE_MODULES, "");
    assert!(parse_plugin_manifest(&missing_module)
        .unwrap_err()
        .to_string()
        .contains("isolated_iframe block contributions require exactly one entry code module"));

    let multiple_modules = VALID_ISOLATED_FRONTEND_MANIFEST.replace(
        "        type_declarations: \"declare const block: unknown; export default block;\"\n",
        r#"        type_declarations: "declare const block: unknown; export default block;"
      - source: "@acme/isolated-chart-support"
        version: "1.0.0"
        exports: [default]
        binding: fetched
        assets:
          - path: "assets/isolated-chart-support.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        type_declarations: "declare const support: unknown; export default support;"
"#,
    );
    assert!(parse_plugin_manifest(&multiple_modules)
        .unwrap_err()
        .to_string()
        .contains("isolated_iframe block contributions require exactly one entry code module"));

    let host_bound = VALID_ISOLATED_FRONTEND_MANIFEST
        .replace("@acme/isolated-chart", "react")
        .replace("binding: fetched", "binding: host");
    assert!(parse_plugin_manifest(&host_bound)
        .unwrap_err()
        .to_string()
        .contains("isolated_iframe entry code module must use fetched binding"));

    let mismatched_entry = VALID_ISOLATED_FRONTEND_MANIFEST.replace(
        "entry: \"@acme/isolated-chart\"",
        "entry: \"@acme/other-chart\"",
    );
    assert!(parse_plugin_manifest(&mismatched_entry)
        .unwrap_err()
        .to_string()
        .contains("isolated_iframe entry must match its fetched code module source"));
}

#[test]
fn d5_p3_isolated_runtime_requires_one_verified_browser_module_asset() {
    let missing_asset = VALID_ISOLATED_FRONTEND_MANIFEST.replace(
        r#"        assets:
          - path: "assets/isolated-chart.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
"#,
        "        assets: []\n",
    );
    assert!(parse_plugin_manifest(&missing_asset)
        .unwrap_err()
        .to_string()
        .contains("isolated_iframe entry code module requires exactly one browser_module asset"));

    let multiple_assets = VALID_ISOLATED_FRONTEND_MANIFEST.replace(
        r#"            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
"#,
        r#"            sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
          - path: "assets/isolated-chart-copy.js"
            role: browser_module
            media_type: "text/javascript; charset=utf-8"
            sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
"#,
    );
    assert!(parse_plugin_manifest(&multiple_assets)
        .unwrap_err()
        .to_string()
        .contains("isolated_iframe entry code module requires exactly one browser_module asset"));
}

#[test]
fn builtin_frontstage_manifest_keeps_the_block_without_backend_module_assets() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/capability-plugins/1flowbase/manifest.yaml");
    let source = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest = parse_plugin_manifest(&source).unwrap();
    crate::validate_frontend_module_assets(manifest_path.parent().unwrap(), &manifest).unwrap();
    assert_eq!(manifest.block_contributions.len(), 1);
    assert_eq!(
        manifest.block_contributions[0].contribution_code,
        "frontstage.js-ui-block"
    );
    assert!(manifest.block_contributions[0].code_modules.is_empty());
    assert!(!manifest_path
        .parent()
        .unwrap()
        .join("browser-assets")
        .exists());
    assert!(!source.contains("components:"));
    assert!(!source.contains("antd_facade"));
    assert!(!source.contains("FacadeCommonProps"));
}

#[test]
fn plugin_manifest_v1_keeps_missing_frontend_block_code_template_null() {
    let manifest = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: frontend_blocks_without_template@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Frontend Blocks Without Template
description: frontend blocks without a code template
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/fixture-frontend-blocks
block_contributions:
  - contribution_code: hero_banner
    title: Hero Banner
    runtime: native_react
    entry: blocks/hero/index.html
    context_contract:
      primitives: [text]
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
"#,
    )
    .unwrap();

    assert_eq!(manifest.block_contributions[0].code_template, None);
}

#[test]
fn plugin_manifest_v1_rejects_invalid_frontend_block_code_templates() {
    let empty_template =
        parse_plugin_manifest(&valid_frontend_block_manifest_with_code_template("   "))
            .unwrap_err();
    assert!(empty_template
        .to_string()
        .contains("block_contributions[].code_template cannot be empty"));

    let oversized_template = "x".repeat(256 * 1024 + 1);
    let oversized_error = parse_plugin_manifest(&valid_frontend_block_manifest_with_code_template(
        &oversized_template,
    ))
    .unwrap_err();
    assert!(oversized_error
        .to_string()
        .contains("block_contributions[].code_template exceeds 262144 bytes"));
}

fn valid_frontend_block_manifest_with_code_template(code_template: &str) -> String {
    format!(
        r#"
manifest_version: 1
plugin_id: frontend_blocks_with_template@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Frontend Blocks With Template
description: frontend blocks with a code template
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes: [frontend_block]
binding_targets: [workspace]
selection_mode: assignment_then_select
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
  entry: bin/fixture-frontend-blocks
block_contributions:
  - contribution_code: hero_banner
    title: Hero Banner
    runtime: native_react
    entry: blocks/hero/index.html
    code_template: {code_template:?}
    code_template_version: 1.0.0
    code_template_language: tsx
    code_modules: []
    context_contract:
      primitives: [text]
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
"#
    )
}

#[test]
fn plugin_manifest_v1_rejects_invalid_frontend_block_values_with_stable_errors() {
    let invalid_runtime = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_frontend_block@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Bad Frontend Block
description: invalid runtime
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/bad-frontend-block
block_contributions:
  - contribution_code: bad_runtime
    title: Bad Runtime
    runtime: react_remote
    entry: blocks/bad/index.html
    context_contract:
      primitives:
        - text
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities:
      - responsive
"#,
    )
    .unwrap_err();

    assert!(invalid_runtime
        .to_string()
        .contains("block_contributions[].runtime must be one of native_react, isolated_iframe"));

    let missing_entry = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: missing_frontend_block_entry@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Missing Frontend Block Entry
description: missing entry
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/missing-frontend-block-entry
block_contributions:
  - contribution_code: missing_entry
    title: Missing Entry
    runtime: native_react
    entry: ""
    context_contract:
      primitives:
        - text
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities:
      - responsive
"#,
    )
    .unwrap_err();

    assert!(missing_entry
        .to_string()
        .contains("block_contributions[].entry cannot be empty"));

    let invalid_permission = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_frontend_block_permission@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Bad Frontend Block Permission
description: invalid permission
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/bad-frontend-block-permission
block_contributions:
  - contribution_code: bad_permission
    title: Bad Permission
    runtime: native_react
    entry: blocks/bad/index.html
    context_contract:
      primitives:
        - text
      input_schema:
        type: object
    permissions:
      network: none
      storage: workspace_write
      secrets: none
    ui_capabilities:
      - responsive
"#,
    )
    .unwrap_err();

    assert!(invalid_permission
        .to_string()
        .contains("block_contributions[].permissions.storage must be one of none"));

    let invalid_primitive = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_frontend_block_primitive@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Bad Frontend Block Primitive
description: invalid primitive
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/bad-frontend-block-primitive
block_contributions:
  - contribution_code: bad_primitive
    title: Bad Primitive
    runtime: native_react
    entry: blocks/bad/index.html
    context_contract:
      primitives:
        - script
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities:
      - responsive
"#,
    )
    .unwrap_err();

    assert!(invalid_primitive.to_string().contains(
        "block_contributions[].context_contract.primitives[] must be one of text, image, link, button, rich_text, data_record"
    ));

    let invalid_capability = parse_plugin_manifest(
        r#"
manifest_version: 1
plugin_id: bad_frontend_block_capability@0.1.0
version: 0.1.0
publisher_namespace: acme
vendor: acme
display_name: Bad Frontend Block Capability
description: invalid capability
source_kind: uploaded
trust_level: checksum_only
consumption_kind: capability_plugin
execution_mode: declarative_only
slot_codes:
  - frontend_block
binding_targets:
  - workspace
selection_mode: assignment_then_select
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
  entry: bin/bad-frontend-block-capability
block_contributions:
  - contribution_code: bad_capability
    title: Bad Capability
    runtime: native_react
    entry: blocks/bad/index.html
    context_contract:
      primitives:
        - text
      input_schema:
        type: object
    permissions:
      network: none
      storage: none
      secrets: none
    ui_capabilities:
      - arbitrary_dom_access
"#,
    )
    .unwrap_err();

    assert!(invalid_capability.to_string().contains(
        "block_contributions[].ui_capabilities[] must be one of responsive, configurable, theming, data_binding"
    ));
}
