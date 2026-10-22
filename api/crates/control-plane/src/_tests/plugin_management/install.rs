use std::{fs, path::PathBuf, sync::Arc};

use crate::{
    errors::ControlPlaneError,
    plugin_management::{
        AssignPluginCommand, EnablePluginCommand, ExtensionCatalogCategory,
        InstallExtensionNodePluginCommand, InstallOfficialPluginCommand, InstallPluginCommand,
        InstallResolvedOfficialPluginCommand, InstallUploadedPluginCommand,
        OfficialPluginCatalogFilter, PluginCatalogFilter, PluginCompatibilityOverride,
        PluginManagementService, PluginRiskOverride, RefreshCurrentNodePluginArtifactCommand,
        PLUGIN_RISK_SIGNATURE_MISSING,
    },
    ports::{
        CreatePluginTaskInput, FrontendBlockCatalogRepository, JsDependencyRepository,
        NodeContributionRepository, PluginRepository, UpsertPluginArtifactInstanceInput,
    },
};
use domain::{ExtensionSignatureStatus, NodeContributionDependencyStatus, PluginTaskStatus};
use plugin_framework::error::PluginFrameworkError;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::support::{
    actor_with_permissions, build_capability_plugin_package_bytes,
    build_openai_compatible_package_bytes,
    build_openai_compatible_package_bytes_with_runtime_entry,
    build_provider_distribution_package_bytes, build_signed_openai_upload_package,
    create_capability_plugin_fixture, create_frontend_block_fixture,
    create_js_dependency_pack_fixture, create_provider_fixture,
    create_provider_fixture_with_node_contribution, requested_locales, seed_test_installation,
    MemoryOfficialPluginSource, MemoryPluginManagementRepository, MemoryProviderRuntime,
};

#[tokio::test]
async fn extension_center_installs_and_activates_distribution_runtime_through_package_intake() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(workspace_id, &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.extension-center").unwrap(),
            "extension_center.install",
        )
        .await;
    let runtime = MemoryProviderRuntime::default();
    let install_root =
        std::env::temp_dir().join(format!("distribution-node-registration-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    )
    .for_extension_center_console_operation("extension_center.install");

    let result = service
        .install_extension_node_plugin(InstallExtensionNodePluginCommand {
            actor_user_id: repository.actor.user_id,
            category: ExtensionCatalogCategory::RuntimeExtensions,
            file_name: "session-retry-distribution.1flowbasepkg".to_string(),
            package_bytes: build_provider_distribution_package_bytes("1.0.0"),
            source_kind: "official_registry".to_string(),
        })
        .await
        .expect("distribution package should use the common install and activation lifecycle");

    assert_eq!(
        result.installation.metadata_json["plugin_type"],
        "provider_distribution_rule"
    );
    assert_eq!(
        result.installation.contract_version,
        "1flowbase.provider-distribution-rule/v1"
    );
    assert_eq!(
        runtime.loaded_installations().await,
        vec![result.installation.id]
    );
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn extension_center_distribution_update_activates_new_version_and_retires_old_runtime() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(workspace_id, &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.extension-center").unwrap(),
            "extension_center.update",
        )
        .await;
    let runtime = MemoryProviderRuntime::default();
    let install_root = std::env::temp_dir().join(format!("distribution-update-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    )
    .for_extension_center_console_operation("extension_center.update");

    let first = service
        .install_extension_node_plugin(InstallExtensionNodePluginCommand {
            actor_user_id: repository.actor.user_id,
            category: ExtensionCatalogCategory::RuntimeExtensions,
            file_name: "session-retry-distribution-1.0.0.1flowbasepkg".to_string(),
            package_bytes: build_provider_distribution_package_bytes("1.0.0"),
            source_kind: "official_registry".to_string(),
        })
        .await
        .unwrap();
    let second = service
        .install_extension_node_plugin(InstallExtensionNodePluginCommand {
            actor_user_id: repository.actor.user_id,
            category: ExtensionCatalogCategory::RuntimeExtensions,
            file_name: "session-retry-distribution-1.1.0.1flowbasepkg".to_string(),
            package_bytes: build_provider_distribution_package_bytes("1.1.0"),
            source_kind: "official_registry".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        runtime.loaded_installations().await,
        vec![first.installation.id, second.installation.id]
    );
    assert_eq!(
        runtime.unloaded_installations().await,
        vec![first.installation.id]
    );
    let retired = repository
        .get_installation(first.installation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retired.desired_state, domain::PluginDesiredState::Disabled);
    let active = repository
        .get_installation(second.installation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        active.desired_state,
        domain::PluginDesiredState::ActiveRequested
    );
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn official_catalog_type_mismatch_is_rejected_before_installation() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("catalog-type-mismatch-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let package_bytes = build_openai_compatible_package_bytes("0.1.0", false);

    let error = service
        .install_resolved_official_plugin(InstallResolvedOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "runtime-extensions:taichuy/session-retry-distribution".into(),
            plugin_type: "provider_distribution_rule".into(),
            minimum_host_version: "0.1.0".into(),
            source_kind: "official_registry".into(),
            file_name: "mismatched.1flowbasepkg".into(),
            expected_checksum: format!(
                "sha256:{:x}",
                base16ct::HexDisplay(&Sha256::digest(&package_bytes))
            ),
            package_bytes,
            compatibility_override: None,
            risk_override: Some(PluginRiskOverride {
                reason: "test acknowledges unsigned fixture".into(),
                acknowledged_warnings: vec![PLUGIN_RISK_SIGNATURE_MISSING.into()],
            }),
        })
        .await
        .expect_err("catalog type must agree with the package manifest slot");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "extension_catalog_package_mismatch"
        ))
    ));
    assert!(repository.list_installations().await.unwrap().is_empty());
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn uploaded_native_package_for_another_platform_is_rejected_before_install_side_effects() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-platform-mismatch-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let mut arm64_elf = vec![0_u8; 64];
    arm64_elf[..4].copy_from_slice(b"\x7fELF");
    arm64_elf[4] = 2;
    arm64_elf[5] = 1;
    arm64_elf[6] = 1;
    arm64_elf[18] = 0xb7;

    let error = service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "openai-compatible-linux-arm64.1flowbasepkg".into(),
            package_bytes: build_openai_compatible_package_bytes_with_runtime_entry(
                "0.1.0", &arm64_elf,
            ),
        })
        .await
        .expect_err("an arm64 ELF must not install on the current x86_64 Linux host");

    assert!(matches!(
        error.downcast_ref::<PluginFrameworkError>(),
        Some(PluginFrameworkError::PackageRuntimeTargetMismatch { .. })
    ));
    assert!(repository.list_installations().await.unwrap().is_empty());
    assert!(repository.list_tasks().await.unwrap().is_empty());
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn root_1545_ac_3_generic_inventory_node_artifact_dual_registers_runtime_plugin() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(workspace_id, &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.extension-center").unwrap(),
            "extension_center.install.upload",
        )
        .await;
    let install_root =
        std::env::temp_dir().join(format!("extension-node-registration-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    )
    .for_extension_center_console_operation("extension_center.install.upload");

    let result = service
        .install_extension_node_plugin(InstallExtensionNodePluginCommand {
            actor_user_id: repository.actor.user_id,
            category: ExtensionCatalogCategory::CapabilityPlugins,
            file_name: "capability.1flowbasepkg".to_string(),
            package_bytes: build_capability_plugin_package_bytes(),
            source_kind: "uploaded".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        result.installation.metadata_json["plugin_type"],
        "capability_plugin"
    );
    assert_eq!(repository.list_installations().await.unwrap().len(), 1);
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn model_provider_settings_reject_capability_upload_before_install_side_effects() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(workspace_id, &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.model-providers").unwrap(),
            "model_provider_plugins.install.upload",
        )
        .await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("model-provider-settings-{}", Uuid::now_v7())),
    )
    .for_model_provider_console_operation("model_provider_plugins.install.upload");

    let error = service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "fixture-capability.1flowbasepkg".into(),
            package_bytes: build_capability_plugin_package_bytes(),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(
            "model_provider_plugin_required"
        ))
    ));
    assert!(repository.list_installations().await.unwrap().is_empty());
    assert!(repository.list_tasks().await.unwrap().is_empty());
}

#[tokio::test]
async fn model_provider_settings_reject_plugin_task_from_another_workspace() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(workspace_id, &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.model-providers").unwrap(),
            "model_provider_plugins.tasks.view",
        )
        .await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("model-provider-task-scope-{}", Uuid::now_v7())),
    )
    .for_model_provider_console_operation("model_provider_plugins.tasks.view");
    let task_id = Uuid::now_v7();
    repository
        .create_task(&CreatePluginTaskInput {
            task_id,
            installation_id: None,
            workspace_id: Some(Uuid::now_v7()),
            provider_code: "fixture_provider".into(),
            task_kind: domain::PluginTaskKind::Install,
            status: domain::PluginTaskStatus::Succeeded,
            status_message: Some("installed".into()),
            detail_json: serde_json::json!({}),
            actor_user_id: Some(Uuid::now_v7()),
        })
        .await
        .unwrap();

    let error = service
        .get_task(repository.actor.user_id, task_id)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("plugin_task_scope"))
    ));
}

#[tokio::test]
async fn plugin_management_service_blocks_manage_actions_without_configure_permission() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert!(catalog.entries.is_empty());

    let error = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: "/tmp/missing".to_string(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(crate::errors::ControlPlaneError::PermissionDenied(
            "permission_denied"
        ))
    ));
}

#[tokio::test]
async fn plugin_management_service_refreshes_empty_current_node_without_global_path_mutation() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let root_a = std::env::temp_dir().join(format!("plugin-node-a-{}", Uuid::now_v7()));
    let root_b = std::env::temp_dir().join(format!("plugin-node-b-{}", Uuid::now_v7()));
    let service_a = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root_a,
    )
    .with_node_id("node-a");
    let service_b = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root_b,
    )
    .with_node_id("node-b");

    let install = service_a
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "openai_compatible-0.1.0.1flowbasepkg".into(),
            package_bytes: build_openai_compatible_package_bytes("0.1.0", false),
        })
        .await
        .unwrap();
    let node_a_artifact = repository
        .get_artifact_instance("node-a", install.installation.id)
        .await
        .unwrap()
        .unwrap();
    let global_installed_path = node_a_artifact.local_path.clone();

    let node_b_artifact = service_b
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: install.installation.id,
        })
        .await
        .unwrap();

    assert_eq!(node_b_artifact.node_id, "node-b");
    assert_eq!(node_b_artifact.artifact_status.as_str(), "missing");
    assert!(node_b_artifact.local_version.is_none());
    assert!(node_b_artifact.local_path.is_none());
    let node_a_after_refresh = repository
        .get_artifact_instance("node-a", install.installation.id)
        .await
        .unwrap()
        .expect("installation should remain present");
    assert_eq!(node_a_after_refresh.local_path, global_installed_path);
    assert_eq!(node_a_after_refresh.artifact_status.as_str(), "ready");
}

#[tokio::test]
async fn plugin_management_service_marks_current_node_artifact_outdated_for_stale_local_version() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let root_a = std::env::temp_dir().join(format!("plugin-node-new-{}", Uuid::now_v7()));
    let root_b = std::env::temp_dir().join(format!("plugin-node-old-{}", Uuid::now_v7()));
    let service_a = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root_a,
    )
    .with_node_id("node-new");
    let service_b = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root_b,
    )
    .with_node_id("node-old");

    service_b
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "openai_compatible-0.1.0.1flowbasepkg".into(),
            package_bytes: build_openai_compatible_package_bytes("0.1.0", false),
        })
        .await
        .unwrap();
    let expected = service_a
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "openai_compatible-0.2.0.1flowbasepkg".into(),
            package_bytes: build_openai_compatible_package_bytes("0.2.0", false),
        })
        .await
        .unwrap()
        .installation;

    let stale_artifact = service_b
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: expected.id,
        })
        .await
        .unwrap();

    assert_eq!(stale_artifact.node_id, "node-old");
    assert_eq!(stale_artifact.local_version.as_deref(), Some("0.1.0"));
    assert_eq!(stale_artifact.artifact_status.as_str(), "outdated");
    let expected_artifact = repository
        .get_artifact_instance("node-new", expected.id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        stale_artifact.local_path.as_deref(),
        expected_artifact.local_path.as_deref()
    );
}

#[tokio::test]
async fn ac_be_4_plugin_management_service_keeps_local_artifact_ready_when_expected_checksum_is_missing_from_marker(
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-checksum-marker-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        domain::PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .set_expected_checksum(installation_id, Some(format!("sha256:{}", "b".repeat(64))))
        .await;

    let artifact = service
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
        })
        .await
        .unwrap();

    assert_eq!(artifact.artifact_status.as_str(), "ready");
    assert_eq!(artifact.last_error.as_deref(), Some("checksum_missing"));
    assert!(repository
        .audit_events()
        .await
        .iter()
        .any(|event| event == "plugin.local_artifact_warning"));
}

#[tokio::test]
async fn publisher_cutover_artifact_scan_accepts_only_matching_legacy_manifest_receipt() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("publisher-cutover-scan-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        domain::PluginDesiredState::ActiveRequested,
    )
    .await;
    let node_id = format!("local:{}", install_root.display());
    let mut artifact = repository
        .get_artifact_instance(&node_id, installation_id)
        .await
        .unwrap()
        .unwrap();
    let installed_path = PathBuf::from(artifact.local_path.clone().unwrap());
    let manifest_path = installed_path.join("manifest.yaml");
    let raw = fs::read_to_string(&manifest_path).unwrap();
    let legacy_raw = raw
        .lines()
        .filter(|line| !line.starts_with("publisher_namespace:"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&manifest_path, legacy_raw).unwrap();
    let fingerprint = plugin_framework::compute_manifest_fingerprint(&manifest_path)
        .await
        .unwrap();
    fs::write(
        installed_path.join(".1flowbase-artifact.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "plugin_id": "fixture_provider@0.1.0",
            "version": "0.1.0",
            "checksum": null,
            "manifest_fingerprint": fingerprint,
        }))
        .unwrap(),
    )
    .unwrap();
    artifact.manifest_fingerprint = Some(fingerprint.clone());
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: artifact.node_id.clone(),
            installation_id: artifact.installation_id,
            local_version: artifact.local_version.clone(),
            local_checksum: artifact.local_checksum.clone(),
            local_path: artifact.local_path.clone(),
            package_path: artifact.package_path.clone(),
            manifest_fingerprint: artifact.manifest_fingerprint.clone(),
            artifact_status: artifact.artifact_status,
            runtime_status: artifact.runtime_status,
            availability_status: artifact.availability_status,
            checked_at: artifact.checked_at,
            last_error: artifact.last_error.clone(),
            is_current: artifact.is_current,
        })
        .await
        .unwrap();
    repository
        .set_legacy_manifest_compatibility(
            installation_id,
            Some("missing_publisher_namespace_v1".to_string()),
        )
        .await;

    let ready = service
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
        })
        .await
        .unwrap();
    assert_eq!(ready.artifact_status.as_str(), "ready");

    let mut marker: serde_json::Value =
        serde_json::from_slice(&fs::read(installed_path.join(".1flowbase-artifact.json")).unwrap())
            .unwrap();
    marker["manifest_fingerprint"] = serde_json::Value::String("0".repeat(64));
    fs::write(
        installed_path.join(".1flowbase-artifact.json"),
        serde_json::to_vec_pretty(&marker).unwrap(),
    )
    .unwrap();
    let corrupted = service
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
        })
        .await
        .unwrap();
    assert_eq!(corrupted.artifact_status.as_str(), "corrupted");
}

#[tokio::test]
async fn plugin_management_service_installs_uploaded_signed_package_as_verified_official() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let fixture = build_signed_openai_upload_package("0.2.0");
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::with_trusted_public_keys(vec![
            fixture.public_key.clone(),
        ])),
        std::env::temp_dir().join(format!("plugin-uploaded-{}", Uuid::now_v7())),
    );

    let result = service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "openai_compatible-0.2.0.1flowbasepkg".into(),
            package_bytes: fixture.package_bytes.clone(),
        })
        .await
        .unwrap();

    assert_eq!(result.installation.source_kind, "uploaded");
    assert_eq!(result.installation.trust_level, "verified_official");
    assert_eq!(
        result.installation.signature_status,
        ExtensionSignatureStatus::Verified
    );
}

#[tokio::test]
async fn plugin_management_service_rejects_restarting_terminal_task() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("plugin-terminal-task-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("plugin-terminal-task-installed-{nonce}"));
    create_provider_fixture(&package_root);
    repository
        .set_created_task_status_override(domain::PluginTaskStatus::Succeeded)
        .await;

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let error = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(crate::errors::ControlPlaneError::InvalidStateTransition { resource, from, to, .. })
            if *resource == "plugin_task" && from == "succeeded" && to == "running"
    ));
}

#[tokio::test]
async fn plugin_management_service_labels_local_install_with_current_install_kind() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let actor_user_id = repository.actor.user_id;
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("plugin-current-install-kind-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("plugin-current-install-kind-installed-{nonce}"));
    create_provider_fixture(&package_root);

    let service = PluginManagementService::new(
        repository,
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let result = service
        .install_plugin(InstallPluginCommand {
            actor_user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        result.installation.metadata_json["install_kind"].as_str(),
        Some("uploaded_manual_install")
    );
}

#[tokio::test]
async fn plugin_management_service_does_not_route_data_source_package_as_model_provider() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("plugin-data-source-installed-{}", Uuid::now_v7())),
    );
    let package_root = create_data_source_fixture_package("http_source", "0.1.0");

    let result = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .expect("data source package should install");

    assert_eq!(
        result.installation.contract_version,
        "1flowbase.data_source/v1"
    );
    assert_eq!(result.installation.provider_code, "http_source");
}

fn create_data_source_fixture_package(source_code: &str, version: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "plugin-data-source-source-{}-{}",
        source_code,
        Uuid::now_v7()
    ));
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("datasource")).unwrap();
    fs::write(
        root.join("manifest.yaml"),
        format!(
            r#"
manifest_version: 1
plugin_id: {source_code}@{version}
version: {version}
publisher_namespace: acme
vendor: acme
display_name: HTTP Source
description: HTTP source runtime extension
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes: [data_source]
binding_targets: [workspace]
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
  entry: bin/{source_code}
"#
        ),
    )
    .unwrap();
    fs::write(root.join("bin").join(source_code), "#!/bin/sh\n").unwrap();
    fs::write(
        root.join("datasource").join(format!("{source_code}.yaml")),
        format!(
            r#"
source_code: {source_code}
display_name: HTTP Source
auth_modes: [api_key]
capabilities: [preview_read]
supports_sync: false
supports_webhook: false
resource_kinds: [table]
config_schema: []
"#
        ),
    )
    .unwrap();
    root
}

#[path = "install/contribution_catalogs.rs"]
mod contribution_catalogs;
mod official_catalog;
