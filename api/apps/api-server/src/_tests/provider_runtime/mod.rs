use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use control_plane::ports::{DataSourceRuntimePort, ProviderRuntimePort};
use domain::{
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use plugin_framework::{
    error::PluginFrameworkError,
    extension_bus::{
        compile_extension_graph, ExtensionBusVersion, ModuleActivationDeclaration,
        ModuleDescriptor, ModuleId, ModuleKind, ModuleVersion,
    },
    provider_contract::{
        ProviderCompactError, ProviderCompactProfile, ProviderCompactResult,
        ProviderCountTokensInput, ProviderInvocationInput, ProviderRuntimeErrorKind,
        ProviderWireOperation,
    },
    DataModelOperationHandlerRef, DataModelTemplateIdentity, DataSourceConfigInput,
    DataSourceExecuteModelOperationInput, DataSourceModelOperationActorContext,
    DataSourceModelOperationScopeContext,
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

fn write_stateful_slot_provider_package(package: &TempProviderPackage, spawn_marker: &Path) {
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
touch '{}'
while IFS= read -r payload; do
  printf '{{"ok":true,"result":{{"provider":"fixture"}}}}\n'
done
"#,
            spawn_marker.display()
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

fn strict_fixture_installation(
    package: &TempProviderPackage,
) -> domain::LocalPluginInstallationRecord {
    let mut installation = fixture_installation(package);
    installation.artifact.runtime_status = PluginRuntimeStatus::Inactive;
    installation.artifact.is_current = true;
    installation
}

fn model_provider_extension_graph() -> Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>
{
    Arc::new(
        crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            Vec::new(),
        )
        .unwrap()
        .compile_graph()
        .unwrap(),
    )
}

fn graph_without_model_provider_point(
) -> Arc<plugin_framework::extension_bus::EffectiveExtensionGraph> {
    Arc::new(
        compile_extension_graph(vec![ModuleDescriptor {
            bus_version: ExtensionBusVersion::V1,
            module_id: ModuleId::new("fixture.boot-core").unwrap(),
            module_version: ModuleVersion::new("1").unwrap(),
            module_kind: ModuleKind::BootCore,
            activation: ModuleActivationDeclaration::Active,
            dependencies: Default::default(),
            granted_permissions: Default::default(),
            extension_points: Vec::new(),
            contributions: Vec::new(),
        }])
        .unwrap(),
    )
}

fn checked_data_source_fixture_installation() -> domain::LocalPluginInstallationRecord {
    let now = OffsetDateTime::now_utc();
    let installation = PluginInstallationRecord {
        id: Uuid::now_v7(),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "test".to_string(),
        provider_code: "data_source_http_fixture".to_string(),
        plugin_id: "data_source_http_fixture@0.1.0".to_string(),
        plugin_version: "0.1.0".to_string(),
        contract_version: "1flowbase.data_source/v1".to_string(),
        protocol: "stdio_json".to_string(),
        display_name: "Data Source HTTP Fixture".to_string(),
        source_kind: "filesystem_dropin".to_string(),
        trust_level: "unverified".to_string(),
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
            local_path: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../plugins/templates/data_source_http_fixture")
                    .display()
                    .to_string(),
            ),
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

fn checked_data_source_model_operation_input(
    payload: serde_json::Value,
) -> DataSourceExecuteModelOperationInput {
    DataSourceExecuteModelOperationInput {
        connection: DataSourceConfigInput::default(),
        handler_ref: DataModelOperationHandlerRef {
            provider: "data_source_http_fixture".to_string(),
            code: "archive_contact".to_string(),
            version: "v1".to_string(),
        },
        resource_key: "contacts".to_string(),
        template_identity: DataModelTemplateIdentity {
            provider: "data_source_http_fixture".to_string(),
            code: "contact_archive".to_string(),
            version: "v1".to_string(),
        },
        operation_code: "archive_contact".to_string(),
        actor: DataSourceModelOperationActorContext {
            actor_id: "user-1".to_string(),
        },
        scope: DataSourceModelOperationScopeContext {
            scope_id: "workspace-1".to_string(),
        },
        payload,
        path: json!({ "id": "contact-1" }),
        query: json!({ "notify": false }),
    }
}

#[tokio::test]
async fn strict_runtime_uses_one_boot_graph_for_model_provider_and_empty_input_pipeline() {
    let graph = model_provider_extension_graph();
    let services = ApiRuntimeServices::new(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
        Arc::clone(&graph),
    )
    .unwrap();
    let snapshot = crate::extension_bus::ExtensionBootSnapshot::new(Arc::clone(&graph));
    let resolver_graph = services.model_provider_extension_graph().unwrap();

    assert!(Arc::ptr_eq(resolver_graph, snapshot.graph_arc()));
    assert_eq!(
        resolver_graph.fingerprint().as_str(),
        snapshot.fingerprint()
    );
    let pipeline_output = ProviderRuntimePort::pipeline_provider_input(
        &ApiProviderRuntime::new(Arc::new(services.clone())),
        ProviderInvocationInput::default(),
    )
    .await
    .unwrap();
    assert_eq!(pipeline_output.input, ProviderInvocationInput::default());
    assert_eq!(
        pipeline_output.receipt.unwrap().graph_fingerprint,
        snapshot.fingerprint()
    );

    let legacy = ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
        Arc::new(RwLock::new(ProviderHost::default())),
        Arc::new(RwLock::new(CapabilityHost::default())),
        Arc::new(RwLock::new(DataSourceHost::default())),
    );
    assert!(legacy.model_provider_extension_graph().is_none());
}

#[tokio::test]
async fn strict_model_provider_binding_loads_the_stateful_lifecycle_host() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let mut installation = strict_fixture_installation(&package);
    installation.artifact.availability_status = PluginAvailabilityStatus::InstallIncomplete;
    let graph = model_provider_extension_graph();
    let provider_host = Arc::new(RwLock::new(ProviderHost::default()));
    let services = Arc::new(
        ApiRuntimeServices::new(
            Arc::clone(&provider_host),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
            Arc::clone(&graph),
        )
        .unwrap(),
    );
    let runtime = ApiProviderRuntime::new(services);

    let output = ProviderRuntimePort::validate_provider(&runtime, &installation, json!({}))
        .await
        .expect("strict binding should dispatch through the stateful provider host");
    let snapshot = provider_host
        .read()
        .await
        .provider_worker_snapshot(&installation.plugin_id)
        .unwrap()
        .unwrap();

    assert_eq!(output["provider"], "fixture");
    assert!(spawn_marker.exists());
    assert_eq!(snapshot.generation, 1);
    assert_eq!(
        snapshot.state,
        plugin_runner::stdio_runtime::ProviderWorkerLifecycleState::Active
    );
    let binding = crate::provider_runtime::ModelProviderSlotResolver::new(graph)
        .resolve(&installation)
        .unwrap();
    assert_eq!(binding.installation_id, installation.id);
    assert_eq!(binding.plugin_id, installation.plugin_id);
    assert_eq!(binding.provider_code, installation.provider_code);
    assert!(matches!(
        binding.provenance,
        crate::provider_runtime::ModelProviderBindingProvenance::BootGraph(_)
    ));
}

#[tokio::test]
async fn strict_model_provider_binding_rejects_invalid_dynamic_facts_before_spawn() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let graph = model_provider_extension_graph();
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
            graph,
        )
        .unwrap(),
    ));
    let valid = strict_fixture_installation(&package);
    let mut cases = Vec::new();

    let mut disabled = valid.clone();
    disabled.installation.desired_state = PluginDesiredState::Disabled;
    cases.push(disabled.clone());
    let mut pending = valid.clone();
    pending.installation.desired_state = PluginDesiredState::PendingRestart;
    cases.push(pending);
    let mut artifact_missing = valid.clone();
    artifact_missing.artifact.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    cases.push(artifact_missing);
    let mut artifact_mismatched = valid.clone();
    artifact_mismatched.artifact.artifact_status = domain::PluginArtifactInstanceStatus::Mismatched;
    cases.push(artifact_mismatched);
    let mut stale_artifact = valid.clone();
    stale_artifact.artifact.is_current = false;
    cases.push(stale_artifact);
    let mut load_failed = valid.clone();
    load_failed.artifact.runtime_status = PluginRuntimeStatus::LoadFailed;
    cases.push(load_failed);
    let mut verification_pending = valid.clone();
    verification_pending.installation.verification_status = PluginVerificationStatus::Pending;
    cases.push(verification_pending);
    let mut unavailable = valid.clone();
    unavailable.artifact.availability_status = PluginAvailabilityStatus::ArtifactMissing;
    cases.push(unavailable);
    let mut wrong_contract = valid.clone();
    wrong_contract.installation.contract_version = "1flowbase.data_source/v1".to_string();
    cases.push(wrong_contract);
    let mut wrong_category = valid.clone();
    wrong_category.installation.category = domain::ExtensionCategory::CapabilityPlugins;
    cases.push(wrong_category);

    for installation in cases {
        assert!(
            ProviderRuntimePort::validate_provider(&runtime, &installation, json!({}))
                .await
                .is_err()
        );
    }
    let canonical_input = ProviderInvocationInput {
        provider_code: disabled.provider_code.clone(),
        ..ProviderInvocationInput::default()
    };
    assert!(ProviderRuntimePort::ensure_loaded(&runtime, &disabled)
        .await
        .is_err());
    assert!(
        ProviderRuntimePort::list_models(&runtime, &disabled, json!({}))
            .await
            .is_err()
    );
    assert!(
        ProviderRuntimePort::get_balance(&runtime, &disabled, json!({}))
            .await
            .is_err()
    );
    assert!(ProviderRuntimePort::count_tokens(
        &runtime,
        &disabled,
        ProviderCountTokensInput::from_invocation(canonical_input.clone())
    )
    .await
    .is_err());
    assert!(
        ProviderRuntimePort::compact(&runtime, &disabled, canonical_input.clone())
            .await
            .is_err()
    );
    assert!(
        ProviderRuntimePort::invoke_stream(&runtime, &disabled, canonical_input.clone())
            .await
            .is_err()
    );
    assert!(ProviderRuntimePort::invoke_stream_with_live_events(
        &runtime,
        &disabled,
        canonical_input,
        None
    )
    .await
    .is_err());
    assert!(!spawn_marker.exists());
}

#[tokio::test]
async fn strict_model_provider_binding_rejects_wrong_provider_code_and_missing_point_before_spawn()
{
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let installation = strict_fixture_installation(&package);
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
            model_provider_extension_graph(),
        )
        .unwrap(),
    ));
    let wrong_provider = ProviderInvocationInput {
        provider_code: "another_provider".to_string(),
        ..ProviderInvocationInput::default()
    };
    assert!(runtime
        .invoke_stream(&installation, wrong_provider)
        .await
        .is_err());

    let missing_point_error = crate::provider_runtime::ModelProviderSlotResolver::new(
        graph_without_model_provider_point(),
    )
    .resolve(&installation)
    .unwrap_err();
    assert!(missing_point_error
        .to_string()
        .contains("model_provider_extension_slot_unavailable"));
    assert!(!spawn_marker.exists());
}

#[test]
fn provider_runtime_consumer_has_no_provider_specific_branch_and_production_wires_strict_graph() {
    let consumer = include_str!("../../provider_runtime/mod.rs");
    let boot = include_str!("../../lib.rs");

    assert!(!consumer.to_ascii_lowercase().contains("openai"));
    assert!(!consumer.to_ascii_lowercase().contains("anthropic"));
    assert_eq!(
        consumer
            .matches("self.resolve_model_provider_binding(installation)?")
            .count(),
        8,
        "every ProviderRuntimePort operation must resolve the typed slot binding"
    );
    assert_eq!(
        consumer.matches("&binding.plugin_id").count(),
        9,
        "ProviderHost load and operation paths must use the binding plugin id"
    );
    assert_eq!(
        consumer.matches("binding.require_provider_code(").count(),
        4,
        "every canonical provider input must fail closed on a crossed provider code"
    );
    assert!(boot.contains("ApiRuntimeServices::new("));
    assert!(boot.contains("Arc::clone(&extension_graph)"));
    assert!(!boot.contains("new_without_model_provider_extension_graph_for_tests"));
}

#[tokio::test]
async fn data_source_runtime_intakes_and_dispatches_checked_external_template() {
    let services = Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    );
    let runtime = ApiProviderRuntime::new(services.clone());
    let installation = checked_data_source_fixture_installation();

    DataSourceRuntimePort::ensure_loaded(&runtime, &installation)
        .await
        .expect("checked data source fixture should load");
    let identity = DataModelTemplateIdentity {
        provider: "data_source_http_fixture".to_string(),
        code: "contact_archive".to_string(),
        version: "v1".to_string(),
    };
    let template = services
        .data_model_template_catalog()
        .resolve(&identity)
        .expect("checked external template should enter the shared catalog");
    assert_eq!(template.descriptor().operations[0].code, "archive_contact");

    let output = DataSourceRuntimePort::execute_model_operation(
        &runtime,
        &installation,
        checked_data_source_model_operation_input(json!({ "reason": "duplicate" })),
    )
    .await
    .expect("checked external operation should dispatch through the host");
    assert_eq!(output["archived"], true);
    assert_eq!(output["contact_id"], "contact-1");

    let malformed_error = DataSourceRuntimePort::execute_model_operation(
        &runtime,
        &installation,
        checked_data_source_model_operation_input(json!({ "malformed_response": true })),
    )
    .await
    .expect_err("malformed external operation output must fail closed");
    assert!(matches!(
        malformed_error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::InvalidInput(
            "data_source_runtime"
        ))
    ));
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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));

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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));

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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::clone(&provider_host),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));
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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));

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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));

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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));

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
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
        ),
    ));

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
