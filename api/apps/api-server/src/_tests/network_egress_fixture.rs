//! Shared persisted Network Center fixture.
//!
//! This lives outside the catalog test module because the Root #1805 consumer authenticity
//! target must compile the real Route -> Pool -> Provider -> Host path without also compiling
//! the complete catalog and API-server test inventory.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use control_plane::ports::{
    CreateModelProviderInstanceInput, CreateNetworkEgressPoolInput,
    CreateNetworkEgressPoolMemberInput, CreateNetworkEgressProviderInput,
    CreateNetworkEgressRouteInput, ModelProviderRepository, NetworkEgressPoolRepository,
    NetworkEgressRepository, NetworkEgressRouteRepository, PluginRepository,
    ReplaceNetworkEgressProjectionInput, UpsertNetworkEgressProviderSecretInput,
    UpsertPluginArtifactInstanceInput, UpsertPluginInstallationInput,
};
use domain::{
    NetworkEgressConsumerSelector, NetworkEgressHealthStatus, NetworkEgressProviderLifecycle,
    PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
};
use plugin_framework::compute_manifest_fingerprint;
use serde_json::json;

use crate::{
    app_state::ApiState, network_egress_client::NetworkEgressHttpClientResolver,
    provider_runtime::ApiProviderRuntime,
};

pub(super) struct TempNetworkEgressPackage {
    root: PathBuf,
}

impl TempNetworkEgressPackage {
    fn new(proxy_url: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("catalog-network-egress-{nonce}"));
        fs::create_dir_all(root.join("bin")).expect("fixture package directory must be created");
        fs::write(
            root.join("manifest.yaml"),
            r#"manifest_version: 1
plugin_id: fixture_egress@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Egress
description: Fixture Egress
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes:
  - network_egress_provider
binding_targets:
  - workspace
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.network_egress_provider/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture_egress
  limits:
    timeout_ms: 2000
node_contributions: []
"#,
        )
        .expect("fixture manifest must be written");
        fs::write(
            root.join("bin/fixture_egress"),
            format!(
                "#!/usr/bin/env bash\nset -euo pipefail\nif [ \"${{1:-}}\" = \"--network-egress-config-file\" ]; then shift 2; fi\nwhile IFS= read -r request; do\n  case \"${{request}}\" in\n    *'\"operation\":\"sync_egresses\"'*) printf '%s\\n' '{{\"operation\":\"sync_egresses\",\"result\":{{\"egresses\":[{{\"provider_egress_key\":\"fixture-egress\",\"display_name\":\"Fixture Egress\",\"availability\":\"available\"}}]}}}}' ;;\n    *'\"operation\":\"acquire_http_forward_proxy\"'*) printf '%s\\n' '{{\"operation\":\"acquire_http_forward_proxy\",\"result\":{{\"lease_id\":\"fixture-lease\",\"http_proxy_url\":\"{proxy_url}\",\"cleanup_token\":\"host-private\",\"expires_at\":4102444800000}}}}' ;;\n    *'\"operation\":\"release_http_forward_proxy\"'*) printf '%s\\n' '{{\"operation\":\"release_http_forward_proxy\",\"result\":{{\"lease_id\":\"fixture-lease\"}}}}' ;;\n    *) exit 1 ;;\n  esac\ndone\n"
            ),
        )
        .expect("fixture egress worker must be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = root.join("bin/fixture_egress");
            let mut permissions = fs::metadata(&path)
                .expect("fixture egress worker metadata must be readable")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions)
                .expect("fixture egress worker must be executable");
        }
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempNetworkEgressPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Persists the exact Route -> Pool -> Provider projection used by the consumer fixtures.
/// The supplied runtime is also the consumer's runtime, so the test cannot replace Host-owned
/// lease acquisition with an injected proxy client.
pub(super) async fn seed_network_egress_resolver(
    state: &ApiState,
    proxy_url: &str,
    selector: NetworkEgressConsumerSelector,
    runtime: ApiProviderRuntime,
) -> (NetworkEgressHttpClientResolver, TempNetworkEgressPackage) {
    let package = TempNetworkEgressPackage::new(proxy_url);
    let root = state
        .store
        .find_user_for_password_login(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID, "root")
        .await
        .expect("root lookup must succeed")
        .expect("test bootstrap must have a root user");
    let installation_id = uuid::Uuid::now_v7();
    let manifest_fingerprint = compute_manifest_fingerprint(&package.path().join("manifest.yaml"))
        .await
        .expect("fixture manifest must have a fingerprint");
    <storage_durable::MainDurableStore as PluginRepository>::upsert_installation(
        &state.store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".into(),
            provider_code: "fixture_egress".into(),
            plugin_id: "fixture_egress@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT.into(),
            protocol: "stdio_json_worker".into(),
            display_name: "Fixture Egress".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: root.id,
        },
    )
    .await
    .expect("fixture installation must persist");
    if let NetworkEgressConsumerSelector::ModelProviderInstance { instance_id } = &selector {
        let model_installation_id = uuid::Uuid::now_v7();
        <storage_durable::MainDurableStore as PluginRepository>::upsert_installation(
            &state.store,
            &UpsertPluginInstallationInput {
                installation_id: model_installation_id,
                category: domain::ExtensionCategory::RuntimeExtensions,
                organization: "test".into(),
                provider_code: "fixture_provider".into(),
                plugin_id: "fixture_provider@0.1.0".into(),
                plugin_version: "0.1.0".into(),
                contract_version: "1flowbase.provider/v2".into(),
                protocol: "openai_compatible".into(),
                display_name: "Fixture Provider".into(),
                source_kind: "uploaded".into(),
                trust_level: "unverified".into(),
                verification_status: PluginVerificationStatus::Valid,
                desired_state: PluginDesiredState::ActiveRequested,
                expected_checksum: None,
                signature_status: domain::ExtensionSignatureStatus::Missing,
                signature_algorithm: None,
                signing_key_id: None,
                metadata_json: json!({}),
                is_system_reserved: false,
                actor_user_id: root.id,
            },
        )
        .await
        .expect("fixture model installation must persist");
        <storage_durable::MainDurableStore as ModelProviderRepository>::create_instance(
            &state.store,
            &CreateModelProviderInstanceInput {
                instance_id: *instance_id,
                workspace_id: state.bootstrap_workspace_id,
                installation_id: model_installation_id,
                provider_code: "fixture_provider".into(),
                protocol: "openai_compatible".into(),
                display_name: "Fixture Model Provider".into(),
                status: domain::ModelProviderInstanceStatus::Ready,
                config_json: json!({"base_url": "https://fixture.invalid"}),
                configured_models: vec![domain::ModelProviderConfiguredModel {
                    model_id: "fixture_chat".into(),
                    enabled: true,
                    context_window_override_tokens: None,
                    supports_multimodal: None,
                    pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.into(),
                    pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.into(),
                }],
                enabled_model_ids: vec!["fixture_chat".into()],
                included_in_main: Some(false),
                created_by: root.id,
            },
        )
        .await
        .expect("fixture model consumer must persist before its egress route");
    }
    <storage_durable::MainDurableStore as PluginRepository>::upsert_artifact_instance(
        &state.store,
        &UpsertPluginArtifactInstanceInput {
            node_id: state.api_node_id.clone(),
            installation_id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            local_path: Some(package.path().display().to_string()),
            package_path: None,
            manifest_fingerprint: Some(manifest_fingerprint),
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: false,
        },
    )
    .await
    .expect("fixture artifact must persist");
    let provider_id = uuid::Uuid::now_v7();
    <storage_durable::MainDurableStore as NetworkEgressRepository>::create_network_egress_provider(
        &state.store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            installation_id,
            provider_code: "fixture_egress".into(),
            display_name: "Fixture Egress".into(),
            secret_ref: "secret://fixture-egress".into(),
            lifecycle: NetworkEgressProviderLifecycle::Active,
            actor_user_id: root.id,
        },
    )
    .await
    .expect("fixture provider must persist");
    <storage_durable::MainDurableStore as NetworkEgressRepository>::upsert_network_egress_provider_secret(
        &state.store,
        &UpsertNetworkEgressProviderSecretInput {
            provider_id,
            secret_ref: "secret://fixture-egress".into(),
            plaintext_secret_json: json!({"token": "egress-provider-secret"}),
            master_key: state.provider_secret_master_key.clone(),
            secret_version: 1,
        },
    )
    .await
    .expect("fixture provider secret must persist");
    <storage_durable::MainDurableStore as NetworkEgressRepository>::replace_network_egress_projection(
        &state.store,
        &ReplaceNetworkEgressProjectionInput {
            provider_id,
            health_status: NetworkEgressHealthStatus::Healthy,
            last_sync_error: None,
            synchronized_at: time::OffsetDateTime::now_utc(),
            egresses: vec![domain::NetworkEgressProjectionRecord {
                provider_id,
                provider_egress_key: "fixture-egress".into(),
                display_name: "Fixture Egress".into(),
                region: None,
                tags: Vec::new(),
                availability: "available".into(),
                synced_at: time::OffsetDateTime::now_utc(),
            }],
            actor_user_id: root.id,
        },
    )
    .await
    .expect("fixture projection must persist");
    let pool_id = uuid::Uuid::now_v7();
    <storage_durable::MainDurableStore as NetworkEgressPoolRepository>::create_network_egress_pool(
        &state.store,
        &CreateNetworkEgressPoolInput {
            pool_id,
            display_name: "Fixture Pool".into(),
            actor_user_id: root.id,
        },
    )
    .await
    .expect("fixture pool must persist");
    <storage_durable::MainDurableStore as NetworkEgressPoolRepository>::create_network_egress_pool_member(
        &state.store,
        &CreateNetworkEgressPoolMemberInput {
            member_id: uuid::Uuid::now_v7(),
            pool_id,
            provider_id,
            provider_egress_key: "fixture-egress".into(),
            enabled: true,
            sequence: 0,
            actor_user_id: root.id,
        },
    )
    .await
    .expect("fixture pool member must persist");
    <storage_durable::MainDurableStore as NetworkEgressRouteRepository>::create_network_egress_route(
        &state.store,
        &CreateNetworkEgressRouteInput {
            route_id: uuid::Uuid::now_v7(),
            workspace_id: state.bootstrap_workspace_id,
            selector,
            pool_id,
            enabled: true,
            actor_user_id: root.id,
        },
    )
    .await
    .expect("fixture route must persist");
    (
        NetworkEgressHttpClientResolver::new(
            state.store.clone(),
            runtime,
            state.provider_secret_master_key.clone(),
            state.api_node_id.clone(),
        ),
        package,
    )
}
