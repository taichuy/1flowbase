use std::{fs, path::Path};

use crate::{
    errors::ControlPlaneError,
    host_infrastructure_config::{
        HostInfrastructureConfigService, SaveHostInfrastructureProviderConfigCommand,
    },
    ports::{PluginRepository, UpsertPluginInstallationInput},
};
use domain::{
    HostInfrastructureConfigStatus, PluginArtifactStatus, PluginAvailabilityStatus,
    PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::super::plugin_management::support::{
    actor_with_permissions, MemoryPluginManagementRepository,
};

fn write_host_extension_fixture(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: redis-infra-host@0.1.0
version: 0.1.0
vendor: 1flowbase tests
display_name: Redis Infra Host
description: Redis host infrastructure fixture
source_kind: uploaded
trust_level: unverified
consumption_kind: host_extension
execution_mode: in_process
slot_codes: [host_bootstrap]
binding_targets: []
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: host_managed
  storage: host_managed
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: host-extension.yaml
"#,
    )
    .unwrap();
    fs::write(
        root.join("host-extension.yaml"),
        r#"schema_version: 1flowbase.host-extension/v1
extension_id: redis-infra-host
version: 0.1.0
bootstrap_phase: pre_state
native:
  abi_version: 1flowbase.host.native/v1
  library: builtin://redis-infra-host
  entry_symbol: redis_infra_host
owned_resources: []
extends_resources: []
infrastructure_providers:
  - contract: storage-ephemeral
    provider_code: redis
    display_name: Redis
    description: Redis backed host infrastructure.
    config_ref: secret://system/redis-infra-host/config
    config_schema:
      - key: host
        label: Host
        type: string
        required: true
      - key: port
        label: Port
        type: number
        required: true
  - contract: cache-store
    provider_code: redis
    display_name: Redis
    description: Redis backed host infrastructure.
    config_ref: secret://system/redis-infra-host/config
    config_schema:
      - key: host
        label: Host
        type: string
        required: true
      - key: port
        label: Port
        type: number
        required: true
routes: []
workers: []
migrations: []
"#,
    )
    .unwrap();
}

async fn seed_host_extension_installation(
    repository: &MemoryPluginManagementRepository,
    installed_path: &Path,
    desired_state: PluginDesiredState,
    runtime_status: PluginRuntimeStatus,
) -> Uuid {
    let now = OffsetDateTime::now_utc();
    repository
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "redis-infra-host".into(),
            plugin_id: "redis-infra-host@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.host_extension/v1".into(),
            protocol: "native_host".into(),
            display_name: "Redis Infra Host".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status,
            availability_status: if matches!(desired_state, PluginDesiredState::Disabled) {
                PluginAvailabilityStatus::Disabled
            } else {
                PluginAvailabilityStatus::PendingRestart
            },
            package_path: None,
            installed_path: installed_path.display().to_string(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({ "seeded_at": now.unix_timestamp() }),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn list_providers_aggregates_contracts_for_inactive_disabled_extension() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["settings_feature.access.system.host-infrastructure"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("host-infra-config-list-{}", Uuid::now_v7()));
    write_host_extension_fixture(&install_root);
    let installation_id = seed_host_extension_installation(
        &repository,
        &install_root,
        PluginDesiredState::Disabled,
        PluginRuntimeStatus::Inactive,
    )
    .await;

    let service = HostInfrastructureConfigService::new(repository.clone());
    let result = service
        .list_providers(repository.actor.clone())
        .await
        .unwrap();

    assert_eq!(result.providers.len(), 1);
    let provider = &result.providers[0];
    assert_eq!(provider.installation_id, installation_id);
    assert_eq!(provider.provider_code, "redis");
    assert_eq!(provider.display_name, "Redis");
    assert_eq!(provider.desired_state, "disabled");
    assert_eq!(provider.runtime_status, "inactive");
    assert_eq!(
        provider.contracts,
        vec!["storage-ephemeral".to_string(), "cache-store".to_string()]
    );
    assert_eq!(provider.config_schema.len(), 2);
    assert!(!provider.restart_required);

    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn save_provider_config_sets_pending_restart_without_runtime_activation() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["settings_feature.access.system.host-infrastructure"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("host-infra-config-save-{}", Uuid::now_v7()));
    write_host_extension_fixture(&install_root);
    let installation_id = seed_host_extension_installation(
        &repository,
        &install_root,
        PluginDesiredState::Disabled,
        PluginRuntimeStatus::Inactive,
    )
    .await;

    let service = HostInfrastructureConfigService::new(repository.clone());
    let result = service
        .save_provider_config(SaveHostInfrastructureProviderConfigCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            provider_code: "redis".into(),
            enabled_contracts: vec!["storage-ephemeral".into()],
            config_json: json!({ "host": "localhost", "port": 6379 }),
        })
        .await
        .unwrap();

    assert!(result.restart_required);
    assert_eq!(result.installation_desired_state, "pending_restart");
    assert_eq!(result.provider_config_status, "pending_restart");

    let installation = repository
        .get_installation(installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        installation.desired_state,
        PluginDesiredState::PendingRestart
    );
    assert_eq!(installation.runtime_status, PluginRuntimeStatus::Inactive);

    let saved = repository
        .host_infrastructure_config(installation_id, "redis")
        .await
        .unwrap();
    assert_eq!(saved.status, HostInfrastructureConfigStatus::PendingRestart);
    assert_eq!(saved.enabled_contracts, vec!["storage-ephemeral"]);
    assert_eq!(
        saved.config_json,
        json!({ "host": "localhost", "port": 6379 })
    );

    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn save_provider_config_rejects_undeclared_contracts() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["settings_feature.access.system.host-infrastructure"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("host-infra-config-invalid-{}", Uuid::now_v7()));
    write_host_extension_fixture(&install_root);
    let installation_id = seed_host_extension_installation(
        &repository,
        &install_root,
        PluginDesiredState::Disabled,
        PluginRuntimeStatus::Inactive,
    )
    .await;

    let service = HostInfrastructureConfigService::new(repository.clone());
    let error = service
        .save_provider_config(SaveHostInfrastructureProviderConfigCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            provider_code: "redis".into(),
            enabled_contracts: vec!["event-bus".into()],
            config_json: json!({ "host": "localhost", "port": 6379 }),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidInput(
            "enabled_contract_not_declared"
        ))
    ));
    assert!(repository
        .host_infrastructure_config(installation_id, "redis")
        .await
        .is_none());

    let _ = fs::remove_dir_all(install_root);
}
