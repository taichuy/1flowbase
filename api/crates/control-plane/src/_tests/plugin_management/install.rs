use std::{fs, path::PathBuf, sync::Arc};

use crate::{
    plugin_management::{
        AssignPluginCommand, EnablePluginCommand, InstallOfficialPluginCommand,
        InstallPluginCommand, InstallUploadedPluginCommand, OfficialPluginCatalogFilter,
        PluginCatalogFilter, PluginCompatibilityOverride, PluginManagementService,
        RefreshCurrentNodePluginArtifactCommand,
    },
    ports::{
        FrontendBlockCatalogRepository, JsDependencyRepository, NodeContributionRepository,
        PluginRepository, UpdatePluginArtifactSnapshotInput,
    },
};
use domain::{NodeContributionDependencyStatus, PluginTaskStatus};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::support::{
    actor_with_permissions, build_openai_compatible_package_bytes,
    build_signed_openai_upload_package, create_capability_plugin_fixture,
    create_frontend_block_fixture, create_js_dependency_pack_fixture, create_provider_fixture,
    create_provider_fixture_with_node_contribution, requested_locales, seed_test_installation,
    MemoryOfficialPluginSource, MemoryPluginManagementRepository, MemoryProviderRuntime,
};

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
    let global_installed_path = install.installation.installed_path.clone();

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
    assert!(node_b_artifact.installed_path.is_none());
    let global_after_refresh = repository
        .get_installation(install.installation.id)
        .await
        .unwrap()
        .expect("installation should remain present");
    assert_eq!(global_after_refresh.installed_path, global_installed_path);
    assert_eq!(global_after_refresh.artifact_status.as_str(), "ready");
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
    assert_ne!(
        stale_artifact.installed_path.as_deref(),
        Some(expected.installed_path.as_str())
    );
}

#[tokio::test]
async fn plugin_management_service_marks_current_node_artifact_mismatched_when_expected_checksum_is_missing_from_marker(
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
    let installation = repository
        .get_installation(installation_id)
        .await
        .unwrap()
        .expect("installation should exist");
    repository
        .update_artifact_snapshot(&UpdatePluginArtifactSnapshotInput {
            installation_id,
            artifact_status: installation.artifact_status,
            availability_status: installation.availability_status,
            package_path: installation.package_path,
            installed_path: installation.installed_path,
            checksum: Some(format!("sha256:{}", "b".repeat(64))),
            manifest_fingerprint: installation.manifest_fingerprint,
        })
        .await
        .unwrap();

    let artifact = service
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
        })
        .await
        .unwrap();

    assert_eq!(artifact.artifact_status.as_str(), "mismatched");
    assert_eq!(artifact.last_error.as_deref(), Some("checksum_missing"));
}

#[tokio::test]
async fn plugin_management_service_lists_official_catalog_and_installs_latest_release_asset() {
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
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.source_kind, "official_registry");
    assert_eq!(catalog.source_label, "官方源");
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].plugin_id, "1flowbase.openai_compatible");

    let expected_package_bytes = build_openai_compatible_package_bytes("0.1.0", false);

    let install = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".to_string(),
            compatibility_override: None,
        })
        .await
        .unwrap();

    assert_eq!(install.installation.provider_code, "openai_compatible");
    assert_eq!(install.installation.source_kind, "official_registry");
    assert_eq!(
        install.installation.checksum.as_deref(),
        Some(format!("sha256:{:x}", Sha256::digest(&expected_package_bytes)).as_str())
    );
    assert_eq!(
        install.installation.signature_status.as_deref(),
        Some("unsigned")
    );
    assert_eq!(install.installation.trust_level, "unverified");
    assert_eq!(install.task.status, PluginTaskStatus::Succeeded);
}

#[tokio::test]
async fn plugin_management_service_marks_official_catalog_entry_below_minimum_host_version() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::with_minimum_host_version(
            "999.0.0",
        )),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    let entry = &catalog.entries[0];
    assert_eq!(entry.minimum_host_version, "999.0.0");
    assert_eq!(entry.current_host_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(entry.compatibility_status, "below_minimum_host_version");
    assert_eq!(
        entry.compatibility_warning_reason.as_deref(),
        Some("below_minimum_host_version")
    );
}

#[tokio::test]
async fn plugin_management_service_requires_explicit_override_for_below_minimum_official_install() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::with_minimum_host_version(
            "999.0.0",
        )),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let blocked = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".to_string(),
            compatibility_override: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        blocked.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(crate::errors::ControlPlaneError::Conflict(
            "plugin_host_version_below_minimum"
        ))
    ));

    let install = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".to_string(),
            compatibility_override: Some(PluginCompatibilityOverride {
                reason: "below_minimum_host_version".to_string(),
                acknowledged_current_host_version: env!("CARGO_PKG_VERSION").to_string(),
                acknowledged_minimum_host_version: "999.0.0".to_string(),
            }),
        })
        .await
        .unwrap();

    assert_eq!(
        install.installation.metadata_json["compatibility_override"]["reason"],
        "below_minimum_host_version"
    );
    assert_eq!(
        install.installation.metadata_json["compatibility_override"]
            ["acknowledged_minimum_host_version"],
        "999.0.0"
    );
    let install_task = repository
        .list_tasks()
        .await
        .unwrap()
        .into_iter()
        .find(|task| task.task_kind == domain::PluginTaskKind::Install)
        .expect("install task should be recorded");
    assert_eq!(
        install_task.detail_json["compatibility_override"]["reason"],
        "below_minimum_host_version"
    );
}

#[tokio::test]
async fn plugin_management_service_rejects_unsigned_signature_required_official_package() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::unsigned_required()),
        std::env::temp_dir().join(format!("plugin-installed-{}", Uuid::now_v7())),
    );

    let error = service
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: repository.actor.user_id,
            plugin_id: "1flowbase.openai_compatible".into(),
            compatibility_override: None,
        })
        .await
        .expect_err("unsigned official package must fail");

    assert!(error
        .to_string()
        .contains("requires a valid official signature"));
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
        result.installation.signature_status.as_deref(),
        Some("verified")
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
async fn plugin_management_service_syncs_manifest_node_contributions_on_install() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root =
        std::env::temp_dir().join(format!("plugin-node-contribution-source-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("plugin-node-contribution-installed-{nonce}"));
    create_provider_fixture_with_node_contribution(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let entries = NodeContributionRepository::list_node_contributions(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].contribution_code, "openai_prompt");
    assert_eq!(
        entries[0].dependency_status,
        NodeContributionDependencyStatus::MissingPlugin
    );
}

#[tokio::test]
async fn plugin_management_service_installs_capability_plugin_node_contributions() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("capability-plugin-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("capability-plugin-installed-{nonce}"));
    create_capability_plugin_fixture(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let entries = NodeContributionRepository::list_node_contributions(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(installation.contract_version, "1flowbase.capability/v1");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].plugin_unique_identifier, "fixture_capability");
    assert_eq!(entries[0].package_id, "fixture_capability@0.1.0");
    assert_eq!(entries[0].schema_version, "1flowbase.node-contribution/v2");
    assert!(entries[0].contribution_checksum.starts_with("sha256:"));
    assert!(entries[0].compiled_contribution_hash.starts_with("sha256:"));
    assert_eq!(
        entries[0].output_schema_snapshot["outputs"][0]["key"],
        "answer"
    );
    assert_eq!(
        entries[0].dependency_status,
        NodeContributionDependencyStatus::MissingPlugin
    );
}

#[tokio::test]
async fn plugin_management_service_syncs_js_dependency_pack_and_catalog_requires_assignment() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("js-dependency-pack-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("js-dependency-pack-installed-{nonce}"));
    create_js_dependency_pack_fixture(&package_root, "zod", "zod");

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;

    let hidden_entries =
        JsDependencyRepository::list_workspace_js_dependencies(&repository, workspace_id)
            .await
            .unwrap();
    assert!(hidden_entries.is_empty());

    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();

    let entries = JsDependencyRepository::list_workspace_js_dependencies(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].provider_code, "fixture_js_dependency_pack");
    assert_eq!(entries[0].alias, "zod");
    assert_eq!(entries[0].package, "zod");
    assert_eq!(entries[0].version, "1.2.3");
    assert_eq!(entries[0].target, "backend_code");
    assert_eq!(entries[0].artifact_path, "artifacts/zod.backend.mjs");
    assert_eq!(entries[0].integrity, "sha256-zod");
    assert_eq!(entries[0].permissions.network, "outbound_only");
    assert_eq!(entries[0].permissions.filesystem, "deny");
    assert_eq!(entries[0].permissions.env, "deny");
}

#[tokio::test]
async fn plugin_management_service_syncs_frontend_block_catalog_and_requires_assignment() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("frontend-block-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("frontend-block-installed-{nonce}"));
    create_frontend_block_fixture(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;

    let hidden_entries =
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&repository, workspace_id)
            .await
            .unwrap();
    assert!(hidden_entries.is_empty());

    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();

    let entries =
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&repository, workspace_id)
            .await
            .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].installation_id, installation.id);
    assert_eq!(entries[0].provider_code, "fixture_frontend_blocks");
    assert_eq!(entries[0].contribution_code, "hero_banner");
    assert_eq!(entries[0].runtime, "iframe");
    assert_eq!(entries[0].entry, "blocks/hero/index.html");
    assert_eq!(entries[0].code_template_version.as_deref(), Some("1.0.0"));
    assert!(entries[0]
        .code_template
        .as_deref()
        .is_some_and(|template| template.contains("defineBlock")));
    assert_eq!(entries[0].code_modules[0].source, "@1flowbase/block-sdk");
    assert_eq!(
        entries[0].context_contract.primitives,
        vec!["text", "image"]
    );
    assert_eq!(entries[0].permissions.storage, "none");
    assert_eq!(
        entries[0].ui_capabilities,
        vec!["responsive", "configurable"]
    );
}

#[tokio::test]
async fn plugin_install_catalog_failure_rolls_back_new_installation_and_artifact() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("frontend-block-failure-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("frontend-block-failure-install-{nonce}"));
    create_frontend_block_fixture(&package_root);
    repository.fail_next_installation_catalog_commit().await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
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

    assert!(error.to_string().contains("constraint violation"));
    assert!(PluginRepository::list_installations(&repository)
        .await
        .unwrap()
        .is_empty());
    assert!(!install_root
        .join("installed/fixture_frontend_blocks/0.1.0")
        .exists());
    assert!(PluginRepository::list_tasks(&repository)
        .await
        .unwrap()
        .iter()
        .any(|task| task.status == PluginTaskStatus::Failed));
}

#[tokio::test]
async fn plugin_reinstall_catalog_failure_preserves_previous_installation_catalog_and_artifact() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("frontend-block-reinstall-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("frontend-block-reinstall-install-{nonce}"));
    create_frontend_block_fixture(&package_root);
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let original = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let installed_entry = PathBuf::from(&original.installed_path).join("blocks/hero/index.html");
    let original_artifact = fs::read_to_string(&installed_entry).unwrap();

    let manifest_path = package_root.join("manifest.yaml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    fs::write(
        &manifest_path,
        manifest
            .replacen("version: 0.1.0", "version: 0.2.0", 1)
            .replace("title: Hero Banner", "title: Changed Banner"),
    )
    .unwrap();
    fs::write(
        package_root.join("blocks/hero/index.html"),
        "<div>changed</div>",
    )
    .unwrap();
    repository.fail_next_installation_catalog_commit().await;

    service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap_err();

    let preserved = PluginRepository::get_installation(&repository, original.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preserved.plugin_version, original.plugin_version);
    assert_eq!(preserved.artifact_status, original.artifact_status);
    assert_eq!(
        fs::read_to_string(installed_entry).unwrap(),
        original_artifact
    );
    assert!(!install_root
        .join("installed/fixture_frontend_blocks/0.2.0")
        .exists());
    assert!(PluginRepository::list_tasks(&repository)
        .await
        .unwrap()
        .iter()
        .any(|task| task.status == PluginTaskStatus::Failed));
    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: original.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: original.id,
        })
        .await
        .unwrap();
    let entries =
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&repository, workspace_id)
            .await
            .unwrap();
    assert_eq!(entries[0].title, "Hero Banner");
}

#[tokio::test]
async fn plugin_management_service_replaces_js_dependency_registry_on_reinstall() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root =
        std::env::temp_dir().join(format!("js-dependency-pack-replace-source-{nonce}"));
    let install_root =
        std::env::temp_dir().join(format!("js-dependency-pack-replace-installed-{nonce}"));
    create_js_dependency_pack_fixture(&package_root, "zod", "zod");

    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();
    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: installation.id,
        })
        .await
        .unwrap();

    fs::remove_dir_all(&package_root).unwrap();
    create_js_dependency_pack_fixture(&package_root, "valibot", "valibot");

    let replaced_installation = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap()
        .installation;
    let entries = JsDependencyRepository::list_workspace_js_dependencies(&repository, workspace_id)
        .await
        .unwrap();

    assert_eq!(replaced_installation.id, installation.id);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].alias, "valibot");
    assert_eq!(entries[0].package, "valibot");
    assert_eq!(entries[0].artifact_path, "artifacts/valibot.backend.mjs");
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
