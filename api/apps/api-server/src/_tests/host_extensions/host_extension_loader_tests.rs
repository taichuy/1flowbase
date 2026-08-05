use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use control_plane::{
    host_extension::HOST_EXTENSION_CONTRACT_VERSION,
    ports::{AuthRepository, PluginRepository, UpsertPluginInstallationInput},
};
use domain::{
    PluginArtifactInstanceStatus, PluginAvailabilityStatus, PluginDesiredState,
    PluginRuntimeStatus, PluginVerificationStatus,
};
use plugin_framework::compute_manifest_fingerprint;
use serde_json::json;
use uuid::Uuid;

use crate::{app_state::ApiState, host_extension_loader::load_host_extensions_at_startup};

use super::super::support::{test_api_state_with_database_url, write_test_executable};

fn create_host_extension_installation_fixture(root: &Path, version: &str, source_kind: &str) {
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        format!(
            r#"manifest_version: 1
plugin_id: fixture_host_extension@{version}
version: {version}
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Host Extension
description: Fixture startup-only host extension
icon: icon.svg
source_kind: {source_kind}
trust_level: checksum_only
consumption_kind: host_extension
execution_mode: in_process
slot_codes:
  - host_bootstrap
binding_targets: []
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: host-extension.yaml
  limits: {{}}
"#
        ),
    )
    .unwrap();
    fs::write(
        root.join("host-extension.yaml"),
        format!(
            r#"schema_version: 1flowbase.host-extension/v1
extension_id: fixture_host_extension
version: {version}
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/fixture_host_extension
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
routes: []
workers: []
migrations: []
"#
        ),
    )
    .unwrap();
    write_test_executable(
        &root.join("lib/fixture_host_extension"),
        "#!/bin/sh\nexit 0\n",
    );
}

async fn write_host_extension_artifact_marker(root: &Path, version: &str) {
    let manifest_fingerprint = compute_manifest_fingerprint(&root.join("manifest.yaml"))
        .await
        .unwrap();
    fs::write(
        root.join(".1flowbase-artifact.json"),
        serde_json::to_vec_pretty(&json!({
            "plugin_id": format!("fixture_host_extension@{version}"),
            "version": version,
            "checksum": null,
            "manifest_fingerprint": manifest_fingerprint,
        }))
        .unwrap(),
    )
    .unwrap();
}

async fn create_current_node_host_extension_fixture(
    state: &ApiState,
    version: &str,
    source_kind: &str,
) -> PathBuf {
    let root = PathBuf::from(&state.provider_install_root)
        .join("installed")
        .join("fixture_host_extension")
        .join(version);
    create_host_extension_installation_fixture(&root, version, source_kind);
    write_host_extension_artifact_marker(&root, version).await;
    root
}

async fn seed_pending_restart_host_extension(
    state: &ApiState,
    installed_root: &Path,
    version: &str,
) -> Uuid {
    let actor = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root",
    )
    .await
    .unwrap()
    .unwrap();
    let manifest_fingerprint = compute_manifest_fingerprint(&installed_root.join("manifest.yaml"))
        .await
        .unwrap();

    let installation = PluginRepository::upsert_installation(
        &state.store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::HostExtensions,
            organization: "test".into(),
            provider_code: "fixture_host_extension".into(),
            plugin_id: format!("fixture_host_extension@{version}"),
            plugin_version: version.into(),
            contract_version: HOST_EXTENSION_CONTRACT_VERSION.into(),
            protocol: "native_host".into(),
            display_name: "Fixture Host Extension".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::PendingRestart,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    PluginRepository::upsert_artifact_instance(
        &state.store,
        &control_plane::ports::UpsertPluginArtifactInstanceInput {
            node_id: state.api_node_id.clone(),
            installation_id: installation.id,
            local_version: Some(version.into()),
            local_checksum: None,
            local_path: Some(installed_root.display().to_string()),
            package_path: None,
            manifest_fingerprint: Some(manifest_fingerprint),
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::PendingRestart,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: false,
        },
    )
    .await
    .unwrap();
    installation.id
}

#[tokio::test]
async fn startup_loader_scans_dropins_and_pending_restart_rows_before_serving() {
    let (base_state, _database_url) = test_api_state_with_database_url().await;
    let dropin_root =
        std::env::temp_dir().join(format!("host-extension-dropins-{}", Uuid::now_v7()));
    let pending_root =
        create_current_node_host_extension_fixture(&base_state, "0.1.0", "uploaded").await;
    create_host_extension_installation_fixture(
        &dropin_root.join("fixture_dropin"),
        "0.1.0",
        "filesystem_dropin",
    );
    let installation_id =
        seed_pending_restart_host_extension(&base_state, &pending_root, "0.1.0").await;
    let state = Arc::new(ApiState {
        host_extension_dropin_root: dropin_root.display().to_string(),
        ..(*base_state).clone()
    });

    let summary = load_host_extensions_at_startup(&state).await.unwrap();
    let installation = PluginRepository::get_installation(&state.store, installation_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(summary.detected_dropin_count, 1);
    assert_eq!(summary.pending_restart_count, 1);
    assert_eq!(summary.loaded_count, 1);
    assert_eq!(summary.failed_count, 0);
    assert_eq!(summary.skipped_count, 0);
    assert_eq!(
        installation.desired_state,
        PluginDesiredState::ActiveRequested
    );
    let artifact =
        PluginRepository::get_artifact_instance(&state.store, &state.api_node_id, installation_id)
            .await
            .unwrap()
            .expect("current node artifact should be recorded");
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Active);
    assert_eq!(
        artifact.artifact_status,
        PluginArtifactInstanceStatus::Ready
    );

    let _ = fs::remove_dir_all(dropin_root);
    let _ = fs::remove_dir_all(pending_root);
}

#[tokio::test]
async fn startup_loader_does_not_use_another_node_host_extension_path_when_current_artifact_is_missing(
) {
    let (base_state, _database_url) = test_api_state_with_database_url().await;
    let other_node_root =
        std::env::temp_dir().join(format!("host-extension-other-node-{}", Uuid::now_v7()));
    create_host_extension_installation_fixture(&other_node_root, "0.1.0", "uploaded");
    let installation_id =
        seed_pending_restart_host_extension(&base_state, &other_node_root, "0.1.0").await;
    sqlx::query(
        "update extension_artifact_instances set node_id = 'other-node' where installation_id = $1",
    )
    .bind(installation_id)
    .execute(base_state.store.pool())
    .await
    .unwrap();

    let summary = load_host_extensions_at_startup(&base_state).await.unwrap();
    let installation = PluginRepository::get_installation(&base_state.store, installation_id)
        .await
        .unwrap()
        .unwrap();
    let artifact = PluginRepository::get_artifact_instance(
        &base_state.store,
        &base_state.api_node_id,
        installation_id,
    )
    .await
    .unwrap();

    assert_eq!(summary.pending_restart_count, 1);
    assert_eq!(summary.loaded_count, 0);
    assert_eq!(summary.failed_count, 0);
    assert_eq!(summary.skipped_count, 1);
    assert_eq!(
        installation.desired_state,
        PluginDesiredState::PendingRestart
    );
    assert!(artifact.is_none());

    let _ = fs::remove_dir_all(other_node_root);
}

#[tokio::test]
async fn installed_host_extension_without_host_extension_yaml_becomes_load_failed() {
    let (base_state, _database_url) = test_api_state_with_database_url().await;
    let pending_root =
        create_current_node_host_extension_fixture(&base_state, "0.1.0", "uploaded").await;
    fs::remove_file(pending_root.join("host-extension.yaml")).unwrap();
    let installation_id =
        seed_pending_restart_host_extension(&base_state, &pending_root, "0.1.0").await;

    let summary = load_host_extensions_at_startup(&base_state).await.unwrap();
    let installation = PluginRepository::get_installation(&base_state.store, installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.pending_restart_count, 1);
    assert_eq!(summary.loaded_count, 0);
    assert_eq!(summary.failed_count, 1);
    assert_eq!(
        installation.desired_state,
        PluginDesiredState::PendingRestart
    );
    let artifact = PluginRepository::get_artifact_instance(
        &base_state.store,
        &base_state.api_node_id,
        installation_id,
    )
    .await
    .unwrap()
    .expect("current node artifact should be recorded");
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::LoadFailed);
    assert_eq!(
        artifact.artifact_status,
        PluginArtifactInstanceStatus::LoadFailed
    );
    assert!(artifact
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("host-extension.yaml"));

    let _ = fs::remove_dir_all(pending_root);
}

#[tokio::test]
async fn invalid_host_extension_yaml_becomes_load_failed_with_last_error() {
    let (base_state, _database_url) = test_api_state_with_database_url().await;
    let pending_root =
        create_current_node_host_extension_fixture(&base_state, "0.1.0", "uploaded").await;
    fs::write(
        pending_root.join("host-extension.yaml"),
        r#"schema_version: wrong/v1
extension_id: fixture_host_extension
version: 0.1.0
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: lib/fixture_host_extension
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
routes: []
workers: []
migrations: []
"#,
    )
    .unwrap();
    let installation_id =
        seed_pending_restart_host_extension(&base_state, &pending_root, "0.1.0").await;

    let summary = load_host_extensions_at_startup(&base_state).await.unwrap();
    assert_eq!(summary.pending_restart_count, 1);
    assert_eq!(summary.loaded_count, 0);
    assert_eq!(summary.failed_count, 1);
    let artifact = PluginRepository::get_artifact_instance(
        &base_state.store,
        &base_state.api_node_id,
        installation_id,
    )
    .await
    .unwrap()
    .expect("current node artifact should be recorded");
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::LoadFailed);
    assert!(artifact
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("schema_version"));

    let _ = fs::remove_dir_all(pending_root);
}

#[tokio::test]
async fn entry_file_existence_alone_is_insufficient() {
    let (base_state, _database_url) = test_api_state_with_database_url().await;
    let pending_root =
        create_current_node_host_extension_fixture(&base_state, "0.1.0", "uploaded").await;
    fs::remove_file(pending_root.join("lib/fixture_host_extension")).unwrap();
    let installation_id =
        seed_pending_restart_host_extension(&base_state, &pending_root, "0.1.0").await;

    let summary = load_host_extensions_at_startup(&base_state).await.unwrap();
    assert_eq!(summary.pending_restart_count, 1);
    assert_eq!(summary.loaded_count, 0);
    assert_eq!(summary.failed_count, 1);
    let artifact = PluginRepository::get_artifact_instance(
        &base_state.store,
        &base_state.api_node_id,
        installation_id,
    )
    .await
    .unwrap()
    .expect("current node artifact should be recorded");
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::LoadFailed);
    assert!(artifact
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("native library"));

    let _ = fs::remove_dir_all(pending_root);
}
