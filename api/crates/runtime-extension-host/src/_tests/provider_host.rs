use super::*;
use extension_package_runtime::provider_contract::{
    NativeModelRequestContext, NativePromptBlock, NativePromptCacheControl,
    NativePromptCacheControlType, ProtocolContextEnvelope, ProviderAuthOperation,
    ProviderCompactProfile, ProviderInvocationCapability, ProviderLogicalSessionState,
    ProviderResetCreditOperation, ProviderResetCreditResult, ProviderTransportSessionAction,
    ProviderTransportSessionCommand, ProviderTransportSessionDirective,
    PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY, PROVIDER_RESET_CREDITS_CAPABILITY,
    PROVIDER_USAGE_WINDOWS_CAPABILITY,
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

#[path = "provider_host/deadline.rs"]
mod deadline;

#[test]
fn invocation_timeout_does_not_derive_from_physical_generation_deadline() {
    let limits = PluginRuntimeLimits {
        invoke_timeout_ms: Some(17_000),
        ..PluginRuntimeLimits::default()
    };
    let mut input = invocation_input("fixture-model");
    input
        .set_transport_session_directive(ProviderTransportSessionDirective {
            worker_incarnation: None,
            logical_session_id: "logical-fixture".into(),
            generation: 7,
            task_id: "invocation-7-1".into(),
            state: ProviderLogicalSessionState::Active,
            physical_deadline_unix_ms: 1,
        })
        .unwrap();

    assert_eq!(
        provider_invocation_limits(&limits, &input).timeout_ms,
        Some(17_000)
    );
}

#[test]
fn native_responses_stream_has_longer_default_without_overriding_package_limit() {
    let ordinary = invocation_input("fixture-model");
    let mut native = ordinary.clone();
    native
        .required_capabilities
        .insert(ProviderInvocationCapability::ResponsesNativePassthrough);
    let defaults = PluginRuntimeLimits::default();
    assert_eq!(
        provider_invocation_limits(&defaults, &ordinary).timeout_ms,
        Some(300_000)
    );
    assert_eq!(
        provider_invocation_limits(&defaults, &native).timeout_ms,
        Some(1_800_000)
    );
    let explicit = PluginRuntimeLimits {
        invoke_timeout_ms: Some(42_000),
        ..defaults
    };
    assert_eq!(
        provider_invocation_limits(&explicit, &native).timeout_ms,
        Some(42_000)
    );
}

#[test]
fn execution_deadline_bounds_both_admission_and_provider_execution() {
    let now = tokio::time::Instant::now();
    let defaults = PluginRuntimeLimits::default();
    let mut native = invocation_input("fixture-model");
    native
        .required_capabilities
        .insert(ProviderInvocationCapability::ResponsesNativePassthrough);
    let configured = provider_invocation_limits(&defaults, &native);
    let deadline = now + Duration::from_secs(30);
    assert_eq!(
        limit_provider_invocation_to_deadline(configured.clone(), Some(deadline), now).timeout_ms,
        Some(30_000)
    );
    assert_eq!(
        limit_provider_invocation_to_deadline(
            configured.clone(),
            Some(deadline),
            now + Duration::from_secs(12)
        )
        .timeout_ms,
        Some(18_000),
        "time spent in admission must reduce the same execution budget"
    );
    assert_eq!(
        limit_provider_invocation_to_deadline(
            configured.clone(),
            Some(deadline),
            now + Duration::from_secs(31)
        )
        .timeout_ms,
        Some(0)
    );
    assert_eq!(
        limit_provider_invocation_to_deadline(configured, None, now).timeout_ms,
        Some(1_800_000),
        "calls without a principal deadline retain their configured budget"
    );
}

#[test]
fn c2_runtime_stage_receipt_is_metadata_only() {
    let mut metadata = json!({});
    attach_runtime_stage_timing(&mut metadata, 3, 7).unwrap();
    let receipt = &metadata[RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY];
    assert_eq!(receipt["schema_version"], 1);
    assert_eq!(receipt["mapping_ms"], 3);
    assert_eq!(receipt["queue_ms"], 7);
    let serialized = serde_json::to_string(&metadata).unwrap();
    for forbidden in [
        "api_key",
        "prompt",
        "tool_output",
        "encrypted_content",
        "previous_response_id",
        "response_id",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn runtime_stage_timing_normalizes_omitted_provider_metadata() {
    let mut metadata = Value::Null;

    attach_runtime_stage_timing(&mut metadata, 5, 11).unwrap();

    assert_eq!(
        metadata[RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY],
        json!({
            "schema_version": 1,
            "mapping_ms": 5,
            "queue_ms": 11,
        })
    );
}

#[test]
fn runtime_stage_timing_rejects_non_object_provider_metadata() {
    for mut metadata in [json!("invalid"), json!(17), json!(["invalid"])] {
        let error = attach_runtime_stage_timing(&mut metadata, 5, 11).unwrap_err();

        assert!(error
            .to_string()
            .contains("provider_metadata must be an object for runtime stage timing"));
    }
}

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
                base16ct::HexDisplay(&Sha256::digest(raw.as_bytes()))
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

#[cfg(unix)]
#[path = "provider_host/stream_error_tests.rs"]
mod stream_error_tests;

#[cfg(unix)]
#[path = "provider_host/transport_bindings.rs"]
mod transport_bindings;

#[cfg(unix)]
#[path = "provider_host/session_workers.rs"]
mod session_workers;

mod invocation_dispatch;
#[path = "provider_host/shared_workers.rs"]
mod shared_workers;
