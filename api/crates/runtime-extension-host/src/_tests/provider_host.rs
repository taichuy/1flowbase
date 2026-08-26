use super::*;
use extension_package_runtime::provider_contract::{
    NativeModelRequestContext, NativePromptBlock, NativePromptCacheControl,
    NativePromptCacheControlType, ProtocolContextEnvelope, ProviderAuthOperation,
    ProviderCompactProfile, ProviderInvocationCapability, ProviderResetCreditOperation,
    ProviderResetCreditResult, PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY,
    PROVIDER_RESET_CREDITS_CAPABILITY, PROVIDER_USAGE_WINDOWS_CAPABILITY,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::time::sleep;

use crate::package_loader::PackageLoader;
use crate::stdio_runtime::ProviderWorkerLifecycleState;

struct TempProviderPackage {
    root: PathBuf,
}

impl TempProviderPackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("provider-host-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let package = Self { root };
        package.write_provider_package("Fixture Provider");
        package
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn write_provider_package(&self, display_name: &str) {
        self.write_provider_package_with_runtime_timeout(display_name, 30_000);
    }

    fn declare_device_code_auth(&self) {
        let provider_path = self.path().join("provider/fixture_provider.yaml");
        let provider = fs::read_to_string(&provider_path)
            .expect("fixture provider definition should be readable");
        let provider = provider.replace(
            "config_schema: []",
            r#"auth:
  actions:
    - code: device_code
      label: Device Code
      user_action_kinds:
        - device_code
  managed_secret_keys:
    - access_token
config_schema: []"#,
        );
        fs::write(provider_path, provider)
            .expect("fixture provider definition should declare device-code auth");
    }

    fn write_auth_runtime(&self, result: &str) {
        self.write(
            "bin/fixture_provider",
            &format!(
                r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${{payload}}" in
  *'"method":"auth"'*) printf '%s' '{{"ok":true,"result":{result}}}' ;;
  *) printf '%s' '{{"ok":false,"error":{{"kind":"provider_invalid_response","message":"expected auth method"}}}}' ;;
esac
"#
            ),
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn remove_publisher_namespace(&self) -> String {
        let path = self.path().join("manifest.yaml");
        let raw = fs::read_to_string(&path).expect("fixture manifest should be readable");
        let legacy = raw.replace("publisher_namespace: 1flowbase\n", "");
        fs::write(path, &legacy).expect("fixture manifest should become a legacy artifact");
        legacy
    }

    fn write_provider_package_with_runtime_timeout(&self, display_name: &str, timeout_ms: u64) {
        self.write(
            "manifest.yaml",
            &format!(
                r#"manifest_version: 1
plugin_id: fixture_provider
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: {display_name}
description: Fixture provider
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: {CURRENT_PROVIDER_CONTRACT}
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_provider
  limits:
    timeout_ms: {timeout_ms}
node_contributions: []
"#
            ),
        );
        self.write(
            "provider/fixture_provider.yaml",
            r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema: []
"#,
        );
        self.write(
            "i18n/en_US.json",
            r#"{ "plugin": { "label": "Fixture Provider" } }"#,
        );
        self.write("bin/fixture_provider", "#!/usr/bin/env bash\n");
    }

    fn write_spawn_side_effect_runtime(&self, marker: &Path) {
        self.write(
                "bin/fixture_provider",
                &format!(
                    "#!/usr/bin/env bash\nset -euo pipefail\ntouch '{}'\nprintf '%s\\n' '{{\"type\":\"result\",\"result\":{{\"final_content\":\"unexpected spawn\",\"finish_reason\":\"stop\"}}}}'\n",
                    marker.display()
                ),
            );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn declare_count_tokens_capability(&self) {
        let manifest_path = self.path().join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path)
            .expect("fixture provider manifest should be readable");
        let manifest = manifest.replace(
            "  limits:\n",
            "  capabilities:\n    - count_tokens\n  limits:\n",
        );
        fs::write(&manifest_path, manifest)
            .expect("fixture provider manifest should declare CountTokens");
    }

    fn declare_compact_capability(&self, capability: &str) {
        let manifest_path = self.path().join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path)
            .expect("fixture provider manifest should be readable");
        let manifest = manifest.replace(
            "  limits:\n",
            &format!("  capabilities:\n    - {capability}\n  limits:\n"),
        );
        fs::write(&manifest_path, manifest)
            .expect("fixture provider manifest should declare Compact capability");
    }

    fn declare_usage_and_reset_credit_capabilities(&self) {
        let manifest_path = self.path().join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path)
            .expect("fixture provider manifest should be readable");
        let manifest = manifest.replace(
            "  limits:\n",
            &format!(
                "  capabilities:\n    - {PROVIDER_USAGE_WINDOWS_CAPABILITY}\n    - {PROVIDER_RESET_CREDITS_CAPABILITY}\n  limits:\n"
            ),
        );
        fs::write(&manifest_path, manifest)
            .expect("fixture provider manifest should declare account operations");
    }

    fn write_usage_and_reset_credit_runtime(&self) {
        self.write(
            "bin/fixture_provider",
            r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${payload}" in
  *'"method":"usage"'*)
    printf '%s' '{"ok":true,"result":{"windows":[{"limit_window_seconds":18000,"used_percent":42.0,"reset_at":"2026-08-20T10:00:00Z"},{"limit_window_seconds":604800,"used_percent":61.0}],"queried_at":"2026-08-20T05:00:00Z"}}'
    ;;
  *'"method":"reset_credit"'*'"type":"count"'*)
    printf '%s' '{"ok":true,"result":{"type":"count","available_count":2}}'
    ;;
  *'"method":"reset_credit"'*'"type":"consume"'*'"idempotency_key":"attempt-123"'*)
    printf '%s' '{"ok":true,"result":{"type":"consumed"}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"kind":"provider_invalid_response","message":"unknown method"}}'
    exit 1
    ;;
esac
"#,
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn write_count_tokens_response_runtime(&self, response: &str) {
        self.write(
                "bin/fixture_provider",
                &format!(
                    r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${{payload}}" in
  *'"operation":"count_tokens"'*) printf '%s\n' '{response}' ;;
  *) printf '%s\n' '{{"ok":false,"error":{{"kind":"provider_invalid_response","message":"missing CountTokens tag"}}}}' ;;
esac
"#
                ),
            );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn write_slow_invoke_runtime(&self) {
        self.write(
            "bin/fixture_provider",
            r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${payload}" in
  *'"method":"invoke"'*)
    printf '%s\n' '{"type":"text_delta","delta":"started"}'
    sleep 0.08
    printf '%s\n' '{"type":"result","result":{"final_content":"done","finish_reason":"stop"}}'
    ;;
  *)
    printf '%s' '{"ok":true,"result":{}}'
    ;;
esac
"#,
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn write_generate_translation_runtime(&self) {
        self.write(
                "bin/fixture_provider",
                r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${payload}" in
  *'cache_control'*|*'end_user_reference'*|*'client_protocol_envelope'*)
    printf '%s\n' '{"type":"error","error":{"kind":"provider_invalid_response","message":"optional context was not translated"}}'
    ;;
  *)
    printf '%s\n' '{"type":"result","result":{"final_content":"translated","finish_reason":"stop","provider_metadata":{"provider":"fixture"}}}'
    ;;
esac
"#,
            );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn write_stateful_provider_package(
        &self,
        plugin_id: &str,
        provider_code: &str,
        display_name: &str,
    ) {
        self.write(
            "manifest.yaml",
            &format!(
                r#"manifest_version: 1
plugin_id: {plugin_id}
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: {display_name}
description: Fixture provider
source_kind: uploaded
trust_level: checksum_only
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
  network: none
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture_provider
  capabilities:
    - config.validate
    - models.list
  limits:
    timeout_ms: 30000
node_contributions: []
"#
            ),
        );
        self.write(
            &format!("provider/{provider_code}.yaml"),
            &format!(
                r#"provider_code: {provider_code}
display_name: {display_name}
protocol: openai_compatible
model_discovery: static
config_schema: []
"#
            ),
        );
    }

    fn write_slow_worker_runtime(&self, response_label: &str) {
        self.write(
                "bin/fixture_provider",
                &format!(
                    r#"#!/usr/bin/env bash
set -euo pipefail
while IFS= read -r payload; do
  case "${{payload}}" in
    *'"method":"invoke"'*)
      printf '%s\n' '{{"type":"text_delta","delta":"started"}}'
      sleep 0.20
      printf '%s\n' '{{"type":"result","result":{{"final_content":"{response_label}","finish_reason":"stop"}}}}'
      ;;
    *)
      printf '%s\n' '{{"ok":true,"result":{{}}}}'
      ;;
  esac
done
"#
                ),
            );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn write_lifecycle_worker_runtime(&self) {
        self.write(
            "bin/fixture_provider",
            include_str!("../../tests/_fixtures/provider_stdio/lifecycle_worker.sh"),
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = self.path().join("bin/fixture_provider");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }
}

impl Drop for TempProviderPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn publisher_cutover_runner_loads_eligible_legacy_installed_provider_receipt() {
    let package = TempProviderPackage::new();
    let raw = package.remove_publisher_namespace();
    assert!(PackageLoader::load(package.path()).is_err());

    let loaded = PackageLoader::load_legacy_installed(
        package.path(),
        &extension_package_runtime::LegacyInstalledManifestEligibility {
            expected_publisher_namespace: "1flowbase".to_string(),
            expected_versioned_plugin_id: "fixture_provider@0.1.0".to_string(),
            expected_raw_manifest_fingerprint: format!(
                "sha256:{:x}",
                Sha256::digest(raw.as_bytes())
            ),
        },
    )
    .expect("AC-001 runner should consume the explicit legacy installation receipt");

    assert_eq!(loaded.package.manifest.publisher_namespace, "1flowbase");
    assert_eq!(loaded.package.identifier(), "fixture_provider@0.1.0");
}

#[test]
fn normalize_models_accepts_current_provider_descriptor_shape() {
    let models = normalize_models(json!([{
        "model_id": "gpt-4o-mini",
        "display_name": "GPT-4o mini",
        "source": "dynamic",
        "supports_streaming": true,
        "supports_tool_call": false,
        "supports_multimodal": false,
        "context_window": null,
        "max_output_tokens": null,
        "parameter_form": null,
        "provider_metadata": {}
    }]))
    .expect("current provider descriptor shape should stay supported");

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_id, "gpt-4o-mini");
}

#[test]
fn normalize_models_rejects_legacy_provider_descriptor_shape() {
    assert!(
            normalize_models(json!([{
                "code": "gpt-4o-mini",
                "label": "GPT-4o mini",
                "family": "llm",
                "mode": "chat"
            }]))
            .is_err(),
            "legacy code/label model descriptors should be rejected once current contract is the only supported shape"
        );
}

#[test]
fn load_if_needed_skips_reloading_matching_loaded_provider_source() {
    let package = TempProviderPackage::new();
    let mut host = ProviderHost::default();
    let summary = host
        .load_with_source_identity(package.path().to_str().unwrap(), Some("gen-1"))
        .unwrap();
    assert!(host.is_loaded(&summary.plugin_id));

    package.write_provider_package("Mutated Provider");
    host.load_if_needed(
        &summary.plugin_id,
        package.path().to_str().unwrap(),
        Some("gen-1"),
    )
    .unwrap();

    let loaded = host.loaded_packages.get(&summary.plugin_id).unwrap();
    assert_eq!(loaded.package.manifest.display_name, "Fixture Provider");
}

#[test]
fn load_if_needed_reloads_when_provider_source_identity_changes() {
    let package = TempProviderPackage::new();
    let mut host = ProviderHost::default();
    let summary = host
        .load_with_source_identity(package.path().to_str().unwrap(), Some("gen-1"))
        .unwrap();
    assert!(host.is_loaded(&summary.plugin_id));

    package.write_provider_package("Mutated Provider");
    host.load_if_needed(
        &summary.plugin_id,
        package.path().to_str().unwrap(),
        Some("gen-2"),
    )
    .unwrap();

    let loaded = host.loaded_packages.get(&summary.plugin_id).unwrap();
    assert_eq!(loaded.package.manifest.display_name, "Mutated Provider");
}

#[tokio::test]
async fn provider_auth_rejects_an_undeclared_begin_action_before_spawning_the_runtime() {
    let package = TempProviderPackage::new();
    package.declare_device_code_auth();
    let spawn_marker = package.path().join("auth-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .authenticate(
            &plugin_id,
            json!({}),
            ProviderAuthOperation::Begin {
                action: "undeclared_action".to_string(),
            },
        )
        .await
        .expect_err("undeclared provider auth actions must fail before runtime execution");

    assert!(error
        .to_string()
        .contains("provider auth action is not declared by the package"));
    assert!(
        !spawn_marker.exists(),
        "an undeclared auth action must not start the provider process"
    );
}

#[tokio::test]
async fn provider_auth_rejects_an_undeclared_managed_secret_patch_from_the_runtime() {
    let package = TempProviderPackage::new();
    package.declare_device_code_auth();
    package.write_auth_runtime(
        r#"{"status":"authorized","managed_secret_patch":{"unexpected_secret":"leak"}}"#,
    );
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .authenticate(
            &plugin_id,
            json!({}),
            ProviderAuthOperation::Begin {
                action: "device_code".to_string(),
            },
        )
        .await
        .expect_err("undeclared auth secret patches must fail closed");

    assert!(error.to_string().contains("undeclared managed secret key"));
}

#[tokio::test]
async fn provider_validation_requires_manifest_capability_before_runtime_execution() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("validation-spawn-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .validate(&plugin_id, json!({}))
        .await
        .expect_err("validation must be hidden until the package declares it");
    assert!(error
        .to_string()
        .contains("does not declare configuration validation support"));
    assert!(
        !spawn_marker.exists(),
        "an undeclared validation operation must not start the provider runtime"
    );
}

#[tokio::test]
async fn provider_model_listing_requires_manifest_capability_before_runtime_execution() {
    let package = TempProviderPackage::new();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .list_models(&plugin_id, json!({}))
        .await
        .expect_err("model listing must be hidden until the package declares it");
    assert!(error
        .to_string()
        .contains("does not declare model listing support"));
}

#[tokio::test]
async fn provider_account_operations_require_manifest_capabilities_before_runtime_execution() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("account-operation-spawn-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let usage_error = host
        .get_usage_windows(&plugin_id, json!({}))
        .await
        .expect_err("usage must be hidden until the package declares it");
    assert!(usage_error
        .to_string()
        .contains("does not declare usage windows support"));

    let reset_error = host
        .reset_credit(&plugin_id, json!({}), ProviderResetCreditOperation::Count)
        .await
        .expect_err("reset credit must be hidden until the package declares it");
    assert!(reset_error
        .to_string()
        .contains("does not declare reset credits support"));
    assert!(
        !spawn_marker.exists(),
        "an undeclared account operation must not start the provider runtime"
    );
}

#[tokio::test]
async fn provider_account_operations_project_usage_and_single_logical_attempt_key() {
    let package = TempProviderPackage::new();
    package.declare_usage_and_reset_credit_capabilities();
    package.write_usage_and_reset_credit_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let usage = host
        .get_usage_windows(&plugin_id, json!({ "api_key": "secret" }))
        .await
        .expect("declared usage operation should project normalized windows");
    assert_eq!(usage.usage.windows.len(), 2);
    assert_eq!(usage.usage.windows[0].limit_window_seconds, 18_000);
    assert_eq!(usage.usage.windows[1].used_percent, 61.0);

    let count = host
        .reset_credit(&plugin_id, json!({}), ProviderResetCreditOperation::Count)
        .await
        .expect("declared reset count should project the available count");
    assert_eq!(
        count.result,
        ProviderResetCreditResult::Count { available_count: 2 }
    );

    let consume = host
        .reset_credit(
            &plugin_id,
            json!({}),
            ProviderResetCreditOperation::Consume {
                idempotency_key: "attempt-123".to_string(),
            },
        )
        .await
        .expect("consume should forward exactly the logical attempt key");
    assert_eq!(consume.result, ProviderResetCreditResult::Consumed);
}

fn invocation_input(model: &str) -> ProviderInvocationInput {
    ProviderInvocationInput {
        provider_instance_id: "provider-1".to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: model.to_string(),
        provider_config: json!({}),
        ..ProviderInvocationInput::default()
    }
}

fn count_tokens_input(model: &str) -> ProviderCountTokensInput {
    ProviderCountTokensInput::from_invocation(ProviderInvocationInput {
        contract_version: Default::default(),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "anthropic_messages".to_string(),
        model: model.to_string(),
        provider_config: json!({}),
        messages: vec![
            extension_package_runtime::provider_contract::ProviderMessage {
                role: extension_package_runtime::provider_contract::ProviderMessageRole::User,
                content: "count this canonical prompt".to_string(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            },
        ],
        ..ProviderInvocationInput::default()
    })
}

fn compact_input(model: &str, profile: ProviderCompactProfile) -> ProviderInvocationInput {
    ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(profile),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_responses".to_string(),
        model: model.to_string(),
        provider_config: json!({}),
        ..ProviderInvocationInput::default()
    }
}

async fn wait_for_active_streams(host: &ProviderHost, count: usize) {
    for _ in 0..20 {
        if host.active_stream_snapshot().await.streams.len() == count {
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("expected {count} active provider stream(s)");
}

#[test]
fn ac_002_current_provider_package_serializes_typed_input_without_projection() {
    let package = TempProviderPackage::new();
    let mut host = ProviderHost::default();
    let summary = host.load(package.path().to_str().unwrap()).unwrap();
    let mut input = invocation_input("fixture-model");
    input.system = vec![NativePromptBlock::text("current typed system")];
    input
        .model_parameters
        .insert("max_output_tokens".to_string(), json!(512));

    let loaded = host
        .loaded_package(&summary.plugin_id)
        .expect("current provider package should be loaded");
    let prepared = current_provider_wire_input(loaded, &input)
        .expect("current provider package should receive direct typed input");
    let wire_input = prepared.wire_value;

    assert_eq!(
        wire_input["contract_version"],
        json!("1flowbase.provider/v2")
    );
    assert_eq!(wire_input["system"][0]["type"], json!("text"));
    assert_eq!(
        wire_input["model_parameters"]["max_output_tokens"],
        json!(512)
    );
    assert!(wire_input["model_parameters"].get("max_tokens").is_none());
    assert!(
        wire_input.get("operation").is_none() && wire_input.get("profile").is_none(),
        "default Generate must retain the existing provider wire shape"
    );
}

#[tokio::test]
async fn wp_r1_generate_attaches_translation_receipt_to_provider_metadata() {
    let package = TempProviderPackage::new();
    package.write_generate_translation_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let mut input = invocation_input("fixture-model");
    input.system = vec![NativePromptBlock::Text {
        text: "system text".to_string(),
        cache_control: Some(NativePromptCacheControl {
            cache_type: NativePromptCacheControlType::Ephemeral,
            ttl: None,
        }),
    }];
    input.request_context = NativeModelRequestContext {
        end_user_reference: Some("external-user".to_string()),
    };
    input.client_protocol_envelope = Some(ProtocolContextEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        query: BTreeMap::from([("preview".to_string(), vec!["one".to_string()])]),
        ..ProtocolContextEnvelope::default()
    });
    input
        .synchronize_required_capabilities()
        .expect("fixture input must use valid canonical message blocks");
    input
        .required_capabilities
        .insert(ProviderInvocationCapability::ProtocolContext);

    let output = host
        .invoke_stream(&plugin_id, input)
        .await
        .expect("optional Generate context should translate before provider spawn");

    assert_eq!(output.result.final_content.as_deref(), Some("translated"));
    assert_eq!(output.result.provider_metadata["provider"], "fixture");
    assert_eq!(
        output.result.provider_metadata[PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY]
            ["decisions"],
        json!([
            "omitted_system_prompt_cache_control",
            "omitted_end_user_reference",
            "omitted_protocol_context_profile_mismatch"
        ])
    );
}

#[tokio::test]
async fn ac_002_legacy_provider_abi_is_rejected_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let summary = host.load(package.path().to_str().unwrap()).unwrap();
    host.loaded_packages
        .get_mut(&summary.plugin_id)
        .expect("loaded fixture should be mutable for the legacy ABI negative")
        .package
        .manifest
        .contract_version = "1flowbase.provider/v1".to_string();

    let error = host
        .invoke_stream(&summary.plugin_id, invocation_input("fixture-model"))
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("unsupported provider package contract"));
    assert!(!spawn_marker.exists(), "legacy ABI must fail before spawn");
    assert!(host.active_stream_snapshot().await.streams.is_empty());
    assert!(lock_provider_worker_registry(&host.provider_workers)
        .expect("provider worker registry should be available")
        .workers
        .is_empty());
}

#[tokio::test]
async fn d1_p03_count_tokens_missing_capability_estimates_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("count-tokens-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let output = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("missing optional capability must use the generic estimator");

    assert!(output.result.input_tokens > 0);
    assert_eq!(
        output.result.method,
        ProviderCountTokensMethod::GenericEstimate
    );
    assert_eq!(
        output.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::CapabilityUnavailable)
    );
    assert!(
        !spawn_marker.exists(),
        "missing CountTokens capability must fail before provider spawn"
    );
}

#[tokio::test]
async fn d1_p03_count_tokens_missing_plugin_returns_a_typed_generic_estimate() {
    let host = ProviderHost::default();

    let output = host
        .count_tokens("missing-plugin", count_tokens_input("fixture-model"))
        .await
        .expect("a missing CountTokens plugin must not escape the host boundary");

    assert!(output.result.input_tokens > 0);
    assert_eq!(
        output.result.method,
        ProviderCountTokensMethod::GenericEstimate
    );
    assert_eq!(
        output.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::PluginUnavailable)
    );
}

#[tokio::test]
async fn k2_compact_missing_capability_fails_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("compact-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .compact(
            &plugin_id,
            compact_input("fixture-model", ProviderCompactProfile::ResponsesCompact),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ProviderCompactError::Unsupported {
            profile: ProviderCompactProfile::ResponsesCompact,
            capabilities,
        } if capabilities == vec!["compact.responses_compact"]
    ));
    assert!(
        !spawn_marker.exists(),
        "missing Compact capability must fail before provider spawn"
    );
}

#[tokio::test]
async fn k2_compact_unclaimed_profile_capability_fails_before_spawn_side_effect() {
    let package = TempProviderPackage::new();
    package.declare_compact_capability("compact.responses_compact");
    let spawn_marker = package
        .path()
        .join("compact-unclaimed-spawn-side-effect-marker");
    package.write_spawn_side_effect_runtime(&spawn_marker);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let error = host
        .compact(
            &plugin_id,
            compact_input(
                "fixture-model",
                ProviderCompactProfile::ResponsesCompactionV2,
            ),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ProviderCompactError::Unsupported {
            profile: ProviderCompactProfile::ResponsesCompactionV2,
            capabilities,
        } if capabilities == vec!["compact.responses_compaction_v2"]
    ));
    assert!(
        !spawn_marker.exists(),
        "a Compact profile must not claim another profile's manifest row"
    );
}

#[tokio::test]
async fn d1_p03_count_tokens_preserves_upstream_success_and_estimates_provider_failures() {
    let package = TempProviderPackage::new();
    package.declare_count_tokens_capability();
    package.write_count_tokens_response_runtime(
            r#"{"ok":false,"error":{"kind":"provider_upstream_error","message":"upstream CountTokens failed"}}"#,
        );
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let upstream = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("upstream CountTokens failures must use the generic estimator");
    assert!(upstream.result.input_tokens > 0);
    assert_eq!(
        upstream.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::ProviderRuntimeFailure)
    );

    package.write_count_tokens_response_runtime(
        r#"{"ok":true,"result":{"operation":"count_tokens","input_tokens":37}}"#,
    );
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let counted = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("tagged CountTokens result should project");
    assert_eq!(counted.result.input_tokens, 37);
    assert_eq!(
        counted.result.method,
        ProviderCountTokensMethod::UpstreamApi
    );

    package.write_count_tokens_response_runtime(
        r#"{"ok":true,"result":{"operation":"count_tokens","input_tokens":"not-a-number"}}"#,
    );
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let malformed = host
        .count_tokens(&plugin_id, count_tokens_input("fixture-model"))
        .await
        .expect("malformed CountTokens results must use the generic estimator");
    assert!(malformed.result.input_tokens > 0);
    assert_eq!(
        malformed.result.fallback_reason,
        Some(ProviderCountTokensFallbackReason::MalformedProviderResult)
    );
}

#[tokio::test]
async fn active_invocation_lease_serializes_same_provider_pool() {
    let package = TempProviderPackage::new();
    package.write_slow_invoke_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let host = Arc::new(host);

    let first_host = Arc::clone(&host);
    let first_plugin_id = plugin_id.clone();
    let first = tokio::spawn(async move {
        first_host
            .invoke_stream(&first_plugin_id, invocation_input("fixture-model"))
            .await
            .unwrap()
    });
    wait_for_active_streams(&host, 1).await;

    let second_host = Arc::clone(&host);
    let second_plugin_id = plugin_id.clone();
    let second = tokio::spawn(async move {
        second_host
            .invoke_stream(&second_plugin_id, invocation_input("fixture-model"))
            .await
            .unwrap()
    });
    sleep(Duration::from_millis(20)).await;

    assert_eq!(host.active_stream_snapshot().await.streams.len(), 1);
    first.await.unwrap();
    second.await.unwrap();
    assert!(host.active_stream_snapshot().await.streams.is_empty());
}

#[tokio::test]
async fn invoke_stream_uses_default_provider_invocation_budget() {
    let package = TempProviderPackage::new();
    package.write_provider_package_with_runtime_timeout("Fixture Provider", 1);
    package.write_slow_invoke_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    let output = host
        .invoke_stream(&plugin_id, invocation_input("fixture-model"))
        .await
        .expect("provider invocation should not inherit the short runtime command timeout");

    assert_eq!(output.result.final_content.as_deref(), Some("done"));
}

#[tokio::test]
async fn active_invocation_lease_allows_different_provider_pools() {
    let package = TempProviderPackage::new();
    package.write_slow_invoke_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let host = Arc::new(host);

    let first_host = Arc::clone(&host);
    let first_plugin_id = plugin_id.clone();
    let first = tokio::spawn(async move {
        first_host
            .invoke_stream(&first_plugin_id, invocation_input("fixture-model-a"))
            .await
            .unwrap()
    });
    let second_host = Arc::clone(&host);
    let second_plugin_id = plugin_id.clone();
    let second = tokio::spawn(async move {
        second_host
            .invoke_stream(&second_plugin_id, invocation_input("fixture-model-b"))
            .await
            .unwrap()
    });

    wait_for_active_streams(&host, 2).await;
    first.await.unwrap();
    second.await.unwrap();
}

#[tokio::test]
async fn stateful_worker_registry_does_not_serialize_different_plugins() {
    let first_package = TempProviderPackage::new();
    first_package.write_stateful_provider_package(
        "fixture_provider_a",
        "fixture_provider_a",
        "Fixture Provider A",
    );
    first_package.write_slow_worker_runtime("first done");
    let second_package = TempProviderPackage::new();
    second_package.write_stateful_provider_package(
        "fixture_provider_b",
        "fixture_provider_b",
        "Fixture Provider B",
    );
    second_package.write_slow_worker_runtime("second done");
    let mut host = ProviderHost::default();
    let first_plugin_id = host
        .load(first_package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let second_plugin_id = host
        .load(second_package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let host = Arc::new(host);

    let first_host = Arc::clone(&host);
    let first = tokio::spawn(async move {
        first_host
            .invoke_stream(&first_plugin_id, invocation_input("fixture-model-a"))
            .await
            .unwrap()
    });
    wait_for_active_streams(&host, 1).await;

    let second_host = Arc::clone(&host);
    let second = tokio::spawn(async move {
        second_host
            .invoke_stream(&second_plugin_id, invocation_input("fixture-model-b"))
            .await
            .unwrap()
    });

    tokio::time::timeout(Duration::from_millis(320), async {
        let first = first.await.unwrap();
        let second = second.await.unwrap();
        assert_eq!(first.result.final_content.as_deref(), Some("first done"));
        assert_eq!(second.result.final_content.as_deref(), Some("second done"));
    })
    .await
    .expect("different stateful provider workers should not be serialized by the registry lock");
}

#[tokio::test]
async fn failed_stateful_worker_is_replaced_on_next_handle_acquisition() {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package(
        "fixture_provider",
        "fixture_provider",
        "Fixture Provider",
    );
    package.write_lifecycle_worker_runtime();
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;

    assert!(host
        .validate(&plugin_id, json!({ "mode": "crash" }))
        .await
        .is_err());
    let failed = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    let failed_receipt = host
        .provider_worker_cleanup_receipt(&plugin_id)
        .unwrap()
        .unwrap();
    assert_eq!(failed.state, ProviderWorkerLifecycleState::Failed);
    assert_eq!(failed.generation, 1);
    assert_eq!(failed_receipt.prior_pid, failed.pid);

    let output = host
        .validate(&plugin_id, json!({ "mode": "normal" }))
        .await
        .expect("the request after a crash should activate a replacement generation");
    let replacement = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    let retained_receipt = host
        .provider_worker_cleanup_receipt(&plugin_id)
        .unwrap()
        .unwrap();

    assert_eq!(replacement.state, ProviderWorkerLifecycleState::Active);
    assert_eq!(replacement.generation, 2);
    assert_ne!(replacement.pid, failed.pid);
    assert_eq!(output.output["pid"], json!(replacement.pid.unwrap()));
    assert_eq!(retained_receipt, failed_receipt);
}

#[test]
fn every_stateful_runtime_dispatch_uses_the_supervisor_admission_gate() {
    let source = include_str!("../provider_host.rs");

    assert_eq!(
        source
            .matches("PluginExecutionMode::StatefulProviderWorker")
            .count(),
        2,
        "non-streaming and streaming are the only stateful dispatch boundaries"
    );
    assert!(source.contains("worker.call(&request).await"));
    assert!(source.contains("worker\n                    .call_streaming_with_limits"));
    assert!(!source.contains("let mut worker = worker.lock().await"));
    assert!(source.contains("call_executable("));
    assert!(source.contains("call_executable_streaming("));
}
