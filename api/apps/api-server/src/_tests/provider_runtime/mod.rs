use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use control_plane::ports::ProviderRuntimePort;
use domain::{
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use plugin_framework::{
    error::PluginFrameworkError,
    provider_contract::{
        ProviderCompactError, ProviderCompactProfile, ProviderCompactResult,
        ProviderInvocationInput, ProviderRuntimeErrorKind, ProviderWireOperation,
    },
};
use plugin_runner::{
    capability_host::CapabilityHost, data_source_host::DataSourceHost, provider_host::ProviderHost,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeServices};

struct TempProviderPackage {
    root: PathBuf,
}

impl TempProviderPackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("api-provider-runtime-test-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        Self { root }
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
}

impl Drop for TempProviderPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_failing_provider_package(package: &TempProviderPackage) {
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Fixture Provider
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
contract_version: 1flowbase.provider/v2
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
    timeout_ms: 30000
node_contributions: []
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
"#,
    );
    package.write(
        "i18n/en_US.json",
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    );
    package.write(
        "bin/fixture_provider",
        r#"#!/usr/bin/env bash
printf '%s' 'invalid api_key' >&2
exit 1
"#,
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn write_balance_provider_package(package: &TempProviderPackage) {
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Fixture Provider
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
contract_version: 1flowbase.provider/v2
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
    timeout_ms: 30000
node_contributions: []
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema:
  - key: api_key
    type: secret
    required: true
"#,
    );
    package.write(
        "i18n/en_US.json",
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    );
    package.write(
            "bin/fixture_provider",
            r#"#!/usr/bin/env bash
payload="$(cat)"
case "${payload}" in
  *'"method":"balance"'*)
    printf '%s' '{"ok":true,"result":{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"110.00","granted_balance":"10.00","topped_up_balance":"100.00"}],"provider_metadata":{"provider":"deepseek"}}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"kind":"provider_invalid_response","message":"unknown method","provider_summary":null}}'
    exit 1
    ;;
esac
"#,
        );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn write_slow_invocation_provider_package(package: &TempProviderPackage) {
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Fixture Provider
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
contract_version: 1flowbase.provider/v2
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
    timeout_ms: 30000
node_contributions: []
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema:
  - key: api_key
    type: secret
    required: true
"#,
    );
    package.write(
        "i18n/en_US.json",
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    );
    package.write(
            "bin/fixture_provider",
            r#"#!/usr/bin/env bash
payload="$(cat)"
case "${payload}" in
  *'"method":"invoke"'*)
    printf '%s\n' '{"type":"text_delta","delta":"slow"}'
    sleep 1
    printf '%s\n' '{"type":"result","result":{"final_content":"slow","finish_reason":"stop"}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"kind":"provider_invalid_response","message":"unknown method","provider_summary":null}}'
    exit 1
    ;;
esac
"#,
        );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn write_compact_provider_package(package: &TempProviderPackage, response: &str) {
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Fixture Provider
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
contract_version: 1flowbase.provider/v2
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
  capabilities:
    - compact.responses_compact
    - compact.responses_compaction_v2
  limits:
    timeout_ms: 30000
node_contributions: []
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema: []
"#,
    );
    package.write(
        "i18n/en_US.json",
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    );
    package.write(
        "bin/fixture_provider",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${{payload}}" in
  *'"method":"invoke"'*'"operation":"compact"'*'"profile":"responses_compaction_v2"'*)
    printf '%s' '{response}'
    ;;
  *)
    printf '%s' '{{"ok":false,"error":{{"kind":"provider_invalid_response","message":"expected typed Compact invoke"}}}}'
    exit 1
    ;;
esac
"#,
        ),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

async fn wait_for_provider_active_streams(provider_host: &Arc<RwLock<ProviderHost>>, count: usize) {
    for _ in 0..20 {
        let snapshot = {
            let host = provider_host.read().await;
            host.active_stream_snapshot().await
        };
        if snapshot.streams.len() == count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("expected {count} active provider stream(s)");
}

fn fixture_installation(package: &TempProviderPackage) -> domain::LocalPluginInstallationRecord {
    let now = OffsetDateTime::now_utc();
    let installation = PluginInstallationRecord {
        id: Uuid::now_v7(),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "test".to_string(),
        provider_code: "fixture_provider".to_string(),
        plugin_id: "fixture_provider@0.1.0".to_string(),
        plugin_version: "0.1.0".to_string(),
        contract_version: "1flowbase.provider/v2".to_string(),
        protocol: "openai_compatible".to_string(),
        display_name: "Fixture Provider".to_string(),
        source_kind: "uploaded".to_string(),
        trust_level: "checksum_only".to_string(),
        verification_status: PluginVerificationStatus::Valid,
        desired_state: PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({}),
        is_system_reserved: false,
        created_by: Uuid::now_v7(),
        updated_by: None,
        created_at: now,
        updated_at: now,
    };
    domain::LocalPluginInstallationRecord {
        artifact: domain::PluginArtifactInstanceRecord {
            node_id: "test-node".to_string(),
            installation_id: installation.id,
            local_version: Some("0.1.0".to_string()),
            local_checksum: None,
            local_path: Some(package.path().display().to_string()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            checked_at: now,
            last_error: None,
            is_current: false,
        },
        installation,
    }
}

fn compact_fixture_installation(
    package: &TempProviderPackage,
) -> domain::LocalPluginInstallationRecord {
    let mut installation = fixture_installation(package);
    installation.installation.contract_version = "1flowbase.provider/v2".to_string();
    installation.installation.protocol = "openai_responses".to_string();
    installation
}

fn compact_invocation_input(profile: ProviderCompactProfile) -> ProviderInvocationInput {
    ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(profile),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_responses".to_string(),
        model: "fixture-compact-model".to_string(),
        provider_config: json!({
            "api_key": "secret"
        }),
        ..ProviderInvocationInput::default()
    }
}

#[tokio::test]
async fn provider_runtime_get_balance_ensures_loaded_and_calls_host() {
    let package = TempProviderPackage::new();
    write_balance_provider_package(&package);
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));

    let balance = runtime
        .get_balance(
            &fixture_installation(&package),
            json!({
                "api_key": "secret"
            }),
        )
        .await
        .expect("balance should be returned through api runtime adapter");

    assert!(balance.is_available);
    assert_eq!(balance.balance_infos[0].currency, "CNY");
    assert_eq!(balance.balance_infos[0].total_balance, "110.00");
    assert_eq!(balance.provider_metadata["provider"], "deepseek");
}

#[tokio::test]
async fn publisher_cutover_provider_runtime_consumes_legacy_manifest_compatibility() {
    let package = TempProviderPackage::new();
    write_balance_provider_package(&package);
    let manifest_path = package.path().join("manifest.yaml");
    let strict_raw = fs::read_to_string(&manifest_path).unwrap();
    let legacy_raw = strict_raw.replace("publisher_namespace: 1flowbase\n", "");
    fs::write(&manifest_path, &legacy_raw).unwrap();
    let mut installation = fixture_installation(&package);
    installation.installation.organization = "1flowbase".to_string();
    installation.installation.legacy_manifest_compatibility =
        Some("missing_publisher_namespace_v1".to_string());
    installation.artifact.manifest_fingerprint = Some(format!(
        "sha256:{:x}",
        Sha256::digest(legacy_raw.as_bytes())
    ));
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));

    let balance = runtime
        .get_balance(&installation, json!({ "api_key": "secret" }))
        .await
        .expect("AC-001 publisher cutover installation should reach the runner");

    assert!(balance.is_available);
}

#[tokio::test]
async fn provider_runtime_drops_host_lock_before_invoking_provider() {
    let package = TempProviderPackage::new();
    write_slow_invocation_provider_package(&package);
    let provider_host = Arc::new(RwLock::new(ProviderHost::default()));
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::clone(&provider_host),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));
    let installation = fixture_installation(&package);

    ProviderRuntimePort::ensure_loaded(&runtime, &installation)
        .await
        .expect("provider should load before invocation");
    let invoke_runtime = runtime.clone();
    let invoke_installation = installation.clone();
    let invocation = tokio::spawn(async move {
        invoke_runtime
            .invoke_stream(
                &invoke_installation,
                ProviderInvocationInput {
                    provider_instance_id: "provider-1".to_string(),
                    provider_code: "fixture_provider".to_string(),
                    protocol: "openai_compatible".to_string(),
                    model: "fixture_chat".to_string(),
                    provider_config: json!({
                        "api_key": "secret"
                    }),
                    ..ProviderInvocationInput::default()
                },
            )
            .await
            .unwrap()
    });
    wait_for_provider_active_streams(&provider_host, 1).await;

    let write_guard = tokio::time::timeout(Duration::from_millis(200), provider_host.write())
        .await
        .expect("provider host write lock should not wait for an external invocation");
    drop(write_guard);
    let output = invocation.await.unwrap();
    assert_eq!(output.result.final_content.as_deref(), Some("slow"));
}

#[tokio::test]
async fn provider_runtime_preserves_contract_error_for_llm_invocation() {
    let package = TempProviderPackage::new();
    write_failing_provider_package(&package);
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));

    let error = runtime
        .invoke_stream(
            &fixture_installation(&package),
            ProviderInvocationInput {
                provider_instance_id: "provider-1".to_string(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "fixture_chat".to_string(),
                provider_config: json!({
                    "base_url": "https://api.example.test",
                    "api_key": "bad-key"
                }),
                ..ProviderInvocationInput::default()
            },
        )
        .await
        .expect_err("runtime contract errors should propagate to orchestration");

    let framework_error = error
        .downcast_ref::<PluginFrameworkError>()
        .expect("provider runtime error should keep framework error type");
    match framework_error {
        PluginFrameworkError::RuntimeContract { error } => {
            assert_eq!(error.kind, ProviderRuntimeErrorKind::AuthFailed);
            assert_eq!(error.message, "invalid api_key");
        }
        other => panic!("expected runtime contract error, got {other:?}"),
    }
}

#[tokio::test]
async fn provider_runtime_preserves_invalid_provider_contract_display_message() {
    let package = TempProviderPackage::new();
    write_failing_provider_package(&package);
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));

    let error = runtime
        .invoke_stream(
            &fixture_installation(&package),
            ProviderInvocationInput {
                operation: ProviderWireOperation::Compact,
                provider_instance_id: "provider-1".to_string(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "fixture_chat".to_string(),
                provider_config: json!({
                    "base_url": "https://api.example.test",
                    "api_key": "bad-key"
                }),
                ..ProviderInvocationInput::default()
            },
        )
        .await
        .expect_err("invalid provider contracts must retain their framework diagnostic");

    let framework_error = error
        .downcast_ref::<PluginFrameworkError>()
        .expect("invalid provider contracts should keep the framework error type");
    assert_eq!(
        framework_error.to_string(),
        "invalid provider contract: provider stream invocation must declare operation=generate"
    );
}

#[tokio::test]
async fn provider_runtime_compact_preserves_typed_v2_opaque_result() {
    let package = TempProviderPackage::new();
    write_compact_provider_package(
        &package,
        r#"{"ok":true,"result":{"result_type":"completed_opaque_compaction_item","operation":"compact","profile":"responses_compaction_v2","response_id":"response-frozen","compaction_item":{"type":"compaction","encrypted_content":"opaque-v2"},"encrypted_content":"opaque-v2"}}"#,
    );
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));

    let compacted = runtime
        .compact(
            &compact_fixture_installation(&package),
            compact_invocation_input(ProviderCompactProfile::ResponsesCompactionV2),
        )
        .await
        .expect("the API provider runtime should return the typed Compact result");

    assert_eq!(
        compacted,
        ProviderCompactResult::CompletedOpaqueCompactionItem {
            operation: ProviderWireOperation::Compact,
            profile: ProviderCompactProfile::ResponsesCompactionV2,
            response_id: Some("response-frozen".to_string()),
            compaction_item: json!({
                "type": "compaction",
                "encrypted_content": "opaque-v2"
            }),
            encrypted_content: "opaque-v2".to_string(),
        }
    );
}

#[tokio::test]
async fn provider_runtime_compact_preserves_typed_provider_failure() {
    let package = TempProviderPackage::new();
    write_compact_provider_package(
        &package,
        r#"{"ok":false,"error":{"kind":"provider_upstream_error","message":"upstream Compact failed","provider_summary":"opaque upstream"}}"#,
    );
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    )));

    let error = runtime
        .compact(
            &compact_fixture_installation(&package),
            compact_invocation_input(ProviderCompactProfile::ResponsesCompactionV2),
        )
        .await
        .expect_err("typed Compact provider failures must not become fabricated success values");

    let compact_error = error
        .downcast_ref::<ProviderCompactError>()
        .expect("the API runtime must preserve the typed Compact error");
    assert!(matches!(
        compact_error,
        ProviderCompactError::Runtime { error }
            if error.kind == ProviderRuntimeErrorKind::ProviderUpstreamError
                && error.message == "upstream Compact failed"
                && error.provider_summary.as_deref() == Some("opaque upstream")
    ));
}
