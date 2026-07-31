use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    host_extension::is_model_provider_installation,
    plugin_management::{
        AssignPluginCommand, DeletePluginFamilyCommand, EnablePluginCommand, InstallPluginCommand,
        InstallUploadedPluginCommand, PluginCatalogFilter, PluginCompatibilityOverride,
        PluginManagementService, SwitchPluginVersionCommand, UpgradeLatestPluginFamilyCommand,
    },
    ports::{
        CreatePluginAssignmentInput, DownloadedOfficialPluginPackage, ModelProviderRepository,
        OfficialPluginCatalogSnapshot, OfficialPluginCatalogSource, OfficialPluginSourceEntry,
        OfficialPluginSourcePort, PluginRepository, UpsertPluginInstallationInput,
    },
};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginTaskKind, PluginTaskStatus, PluginVerificationStatus,
};
use plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;

use super::support::{
    actor_with_permissions, build_openai_compatible_package_bytes, create_provider_fixture,
    requested_locales, seed_test_installation, MemoryOfficialPluginSource,
    MemoryPluginManagementRepository, MemoryProviderRuntime,
};

#[tokio::test]
async fn plugin_management_service_switches_to_a_local_version_without_redownloading() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let install_root = std::env::temp_dir().join(format!("plugin-switch-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let target_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.2.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current_installation,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    let task = service
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
            target_installation_id: target_installation,
        })
        .await
        .unwrap();

    let assignments = repository.list_assignments(workspace_id).await.unwrap();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].installation_id, target_installation);
    assert_eq!(task.task_kind, PluginTaskKind::SwitchVersion);
    assert_eq!(task.status, PluginTaskStatus::Succeeded);
}

#[tokio::test]
async fn plugin_management_service_switches_version_and_invalidates_provider_caches() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let install_root =
        std::env::temp_dir().join(format!("plugin-switch-migrate-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let target_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.2.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current_installation,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    repository
        .seed_instance_with_ready_cache(
            current_installation,
            "fixture_provider",
            "Fixture Provider Prod",
        )
        .await;
    repository
        .seed_instance_with_ready_cache(
            current_installation,
            "fixture_provider",
            "Fixture Provider Staging",
        )
        .await;

    let task = service
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
            target_installation_id: target_installation,
        })
        .await
        .unwrap();

    assert_eq!(task.task_kind, PluginTaskKind::SwitchVersion);
    assert_eq!(task.detail_json["migrated_instance_count"], 2);
    assert_eq!(
        repository
            .assignment_installation_id("fixture_provider")
            .await,
        target_installation
    );
    assert_eq!(
        repository.cache_refresh_statuses().await,
        vec!["idle", "idle"]
    );
}

#[tokio::test]
async fn plugin_management_service_upgrades_to_latest_without_redownloading_when_already_installed_locally(
) {
    #[derive(Clone)]
    struct LatestAlreadyInstalledSource {
        download_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl OfficialPluginSourcePort for LatestAlreadyInstalledSource {
        async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
            Ok(OfficialPluginCatalogSnapshot {
                source: OfficialPluginCatalogSource {
                    source_kind: "official_registry".into(),
                    source_label: "官方源".into(),
                    registry_url: "https://example.com/official-registry.json".into(),
                },
                freshness: crate::ports::OfficialPluginCatalogFreshness::Fresh,
                entries: vec![OfficialPluginSourceEntry {
                    plugin_id: "1flowbase.fixture_provider".into(),
                    plugin_type: "model_provider".into(),
                    provider_code: "fixture_provider".into(),
                    namespace: "plugin.fixture_provider".into(),
                    protocol: "openai_compatible".into(),
                    latest_version: "0.2.0".into(),
                    minimum_host_version: "0.1.0".into(),
                    icon: None,
                    selected_artifact: super::support::sample_artifact(
                        "linux",
                        "amd64",
                        Some("musl"),
                    ),
                    i18n_summary: super::support::sample_i18n_summary(),
                    release_tag: "fixture_provider-v0.2.0".into(),
                    trust_mode: "allow_unsigned".into(),
                    help_url: Some("https://example.com/help".into()),
                    model_discovery_mode: "hybrid".into(),
                }],
            })
        }

        async fn download_plugin(
            &self,
            _entry: &OfficialPluginSourceEntry,
        ) -> Result<DownloadedOfficialPluginPackage> {
            self.download_calls.fetch_add(1, Ordering::SeqCst);
            anyhow::bail!("download should not be called when latest is already installed")
        }

        fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
            Vec::new()
        }
    }

    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let download_calls = Arc::new(AtomicUsize::new(0));
    let install_root =
        std::env::temp_dir().join(format!("plugin-upgrade-latest-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(LatestAlreadyInstalledSource {
            download_calls: download_calls.clone(),
        }),
        &install_root,
    );

    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let target_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.2.0",
        PluginDesiredState::Disabled,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current_installation,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    let task = service
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
            compatibility_override: None,
        })
        .await
        .unwrap();

    let assignments = repository.list_assignments(workspace_id).await.unwrap();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].installation_id, target_installation);
    assert_eq!(task.task_kind, PluginTaskKind::SwitchVersion);
    assert_eq!(task.status, PluginTaskStatus::Succeeded);
    assert_eq!(download_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn ac_001_ac_002_upgrade_latest_installs_and_loads_existing_global_target_on_current_node() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let root_with_latest =
        std::env::temp_dir().join(format!("plugin-upgrade-global-latest-{}", Uuid::now_v7()));
    let current_node_root =
        std::env::temp_dir().join(format!("plugin-upgrade-current-node-{}", Uuid::now_v7()));
    let runtime = MemoryProviderRuntime::default();
    let global_service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root_with_latest,
    )
    .with_node_id("node-with-latest");
    let current_node_service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &current_node_root,
    )
    .with_node_id("current-node");

    let target = global_service
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: repository.actor.user_id,
            file_name: "openai_compatible-0.1.0.1flowbasepkg".into(),
            package_bytes: build_openai_compatible_package_bytes("0.1.0", false),
        })
        .await
        .unwrap()
        .installation;
    let current = seed_test_installation(
        &repository,
        &current_node_root,
        "openai_compatible",
        "0.0.9",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current,
            workspace_id,
            provider_code: "openai_compatible".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    let task = current_node_service
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "openai_compatible".into(),
            compatibility_override: None,
        })
        .await
        .unwrap();

    assert_eq!(task.status, PluginTaskStatus::Succeeded);
    assert_eq!(
        repository
            .assignment_installation_id("openai_compatible")
            .await,
        target.id
    );
    assert_eq!(runtime.loaded_installations().await, vec![target.id]);
    let artifact = repository
        .get_artifact_instance("current-node", target.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(artifact.local_version.as_deref(), Some("0.1.0"));
    assert_eq!(artifact.artifact_status.as_str(), "ready");
    assert_eq!(artifact.runtime_status.as_str(), "active");
    assert!(current_node_root
        .join("installed/openai_compatible/0.1.0")
        .is_dir());
}

#[tokio::test]
async fn plugin_management_service_requires_override_when_upgrading_to_below_minimum_latest() {
    #[derive(Clone)]
    struct IncompatibleLatestAlreadyInstalledSource;

    #[async_trait]
    impl OfficialPluginSourcePort for IncompatibleLatestAlreadyInstalledSource {
        async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
            Ok(OfficialPluginCatalogSnapshot {
                source: OfficialPluginCatalogSource {
                    source_kind: "official_registry".into(),
                    source_label: "官方源".into(),
                    registry_url: "https://example.com/official-registry.json".into(),
                },
                freshness: crate::ports::OfficialPluginCatalogFreshness::Fresh,
                entries: vec![OfficialPluginSourceEntry {
                    plugin_id: "1flowbase.fixture_provider".into(),
                    plugin_type: "model_provider".into(),
                    provider_code: "fixture_provider".into(),
                    namespace: "plugin.fixture_provider".into(),
                    protocol: "openai_compatible".into(),
                    latest_version: "0.2.0".into(),
                    minimum_host_version: "999.0.0".into(),
                    icon: None,
                    selected_artifact: super::support::sample_artifact(
                        "linux",
                        "amd64",
                        Some("musl"),
                    ),
                    i18n_summary: super::support::sample_i18n_summary(),
                    release_tag: "fixture_provider-v0.2.0".into(),
                    trust_mode: "allow_unsigned".into(),
                    help_url: Some("https://example.com/help".into()),
                    model_discovery_mode: "hybrid".into(),
                }],
            })
        }

        async fn download_plugin(
            &self,
            _entry: &OfficialPluginSourceEntry,
        ) -> Result<DownloadedOfficialPluginPackage> {
            anyhow::bail!("download should not be called when latest is already installed")
        }

        fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
            Vec::new()
        }
    }

    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-upgrade-incompatible-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(IncompatibleLatestAlreadyInstalledSource),
        &install_root,
    );

    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let target_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.2.0",
        PluginDesiredState::Disabled,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current_installation,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    let blocked = service
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
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

    let task = service
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
            compatibility_override: Some(PluginCompatibilityOverride {
                reason: "below_minimum_host_version".into(),
                acknowledged_current_host_version: env!("CARGO_PKG_VERSION").into(),
                acknowledged_minimum_host_version: "999.0.0".into(),
            }),
        })
        .await
        .unwrap();

    assert_eq!(task.task_kind, PluginTaskKind::SwitchVersion);
    assert_eq!(
        task.detail_json["compatibility_override"]["reason"],
        "below_minimum_host_version"
    );
    let assignments = repository.list_assignments(workspace_id).await.unwrap();
    assert_eq!(assignments[0].installation_id, target_installation);
}

#[tokio::test]
async fn plugin_management_service_deletes_provider_family_with_instances_and_artifacts() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-delete-family-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let current_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let old_installation = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.0.9",
        PluginDesiredState::Disabled,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current_installation,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    repository
        .seed_instance_with_ready_cache(
            current_installation,
            "fixture_provider",
            "Fixture Provider Prod",
        )
        .await;
    repository
        .seed_instance_with_ready_cache(
            old_installation,
            "fixture_provider",
            "Fixture Provider Staging",
        )
        .await;

    let current_path = PathBuf::from(
        &repository
            .get_installation(current_installation)
            .await
            .unwrap()
            .unwrap()
            .installed_path,
    );
    let old_path = PathBuf::from(
        &repository
            .get_installation(old_installation)
            .await
            .unwrap()
            .unwrap()
            .installed_path,
    );

    let task = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "fixture_provider".into(),
        })
        .await
        .unwrap();

    assert_eq!(task.task_kind, PluginTaskKind::Uninstall);
    assert_eq!(task.status, PluginTaskStatus::Succeeded);
    assert_eq!(task.detail_json["deleted_instance_count"], 2);
    assert_eq!(task.detail_json["deleted_installation_count"], 2);
    assert_eq!(repository.list_installations().await.unwrap().len(), 0);
    assert_eq!(
        repository
            .list_assignments(workspace_id)
            .await
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        repository.list_instances(workspace_id).await.unwrap().len(),
        0
    );
    assert!(!current_path.exists());
    assert!(!old_path.exists());
    assert_eq!(
        repository.audit_events().await,
        vec!["plugin.family_deleted"]
    );
}

#[tokio::test]
async fn plugin_management_service_rejects_deleting_a_system_builtin_family() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-delete-builtin-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "builtin_frontstage",
        "1.0.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository.mark_installation_builtin(installation_id).await;

    let error = service
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: repository.actor.user_id,
            provider_code: "builtin_frontstage".into(),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(crate::errors::ControlPlaneError::Conflict(
            "builtin_plugin_immutable"
        ))
    ));
    assert!(repository
        .get_installation(installation_id)
        .await
        .unwrap()
        .is_some());
    assert!(repository.list_tasks().await.unwrap().is_empty());
}

#[tokio::test]
async fn plugin_management_service_installs_enables_assigns_and_lists_tasks() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let runtime = MemoryProviderRuntime::default();
    let nonce = Uuid::now_v7().to_string();
    let package_root = std::env::temp_dir().join(format!("plugin-source-{nonce}"));
    let install_root = std::env::temp_dir().join(format!("plugin-installed-{nonce}"));
    create_provider_fixture(&package_root);

    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let install = service
        .install_plugin(InstallPluginCommand {
            actor_user_id: repository.actor.user_id,
            package_root: package_root.display().to_string(),
        })
        .await
        .unwrap();
    assert_eq!(install.task.status, PluginTaskStatus::Succeeded);
    assert!(matches!(
        install.installation.desired_state,
        PluginDesiredState::Disabled
    ));
    assert!(PathBuf::from(&install.installation.installed_path).is_dir());
    assert!(!std::path::Path::new(&install.installation.installed_path)
        .join("demo")
        .exists());
    assert!(!std::path::Path::new(&install.installation.installed_path)
        .join("scripts")
        .exists());

    let enable = service
        .enable_plugin(EnablePluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: install.installation.id,
        })
        .await
        .unwrap();
    assert_eq!(enable.status, PluginTaskStatus::Succeeded);

    let assign = service
        .assign_plugin(AssignPluginCommand {
            actor_user_id: repository.actor.user_id,
            installation_id: install.installation.id,
        })
        .await
        .unwrap();
    assert_eq!(assign.status, PluginTaskStatus::Succeeded);

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert!(catalog.entries[0].assigned_to_current_workspace);
    assert_eq!(catalog.entries[0].model_discovery_mode, "hybrid");

    let tasks = service.list_tasks(repository.actor.user_id).await.unwrap();
    assert_eq!(tasks.len(), 3);
    let fetched = service
        .get_task(repository.actor.user_id, install.task.id)
        .await
        .unwrap();
    assert_eq!(fetched.task_kind, PluginTaskKind::Install);
    assert_eq!(
        runtime.loaded_installations().await,
        vec![install.installation.id]
    );
    assert_eq!(
        repository.audit_events().await,
        vec!["plugin.installed", "plugin.enabled", "plugin.assigned"]
    );
}

#[tokio::test]
async fn assign_plugin_allows_data_source_runtime_installation() {
    let (service, repository, actor_user_id, installation_id, workspace_id) =
        seed_data_source_runtime_installation().await;

    service
        .enable_plugin(EnablePluginCommand {
            actor_user_id,
            installation_id,
        })
        .await
        .expect("data source runtime should enable");

    service
        .assign_plugin(AssignPluginCommand {
            actor_user_id,
            installation_id,
        })
        .await
        .expect("data source runtime should assign");

    let assignments = repository.list_assignments(workspace_id).await.unwrap();
    assert!(assignments
        .iter()
        .any(|item| item.installation_id == installation_id));
}

#[tokio::test]
async fn ac_002_model_provider_recognition_accepts_only_current_contract() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-provider-recognition-{}", Uuid::now_v7()));
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let current = repository
        .get_installation(installation_id)
        .await
        .unwrap()
        .expect("current provider installation should exist");
    assert_eq!(current.contract_version, CURRENT_PROVIDER_CONTRACT);
    assert!(is_model_provider_installation(&current));

    let mut legacy = current;
    legacy.contract_version = "1flowbase.provider/v1".to_string();
    assert!(!is_model_provider_installation(&legacy));
}

#[tokio::test]
async fn ac_002_workspace_assignment_rejects_legacy_provider_contract() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let actor_user_id = repository.actor.user_id;
    let install_root =
        std::env::temp_dir().join(format!("plugin-legacy-assignment-{}", Uuid::now_v7()));
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .set_installation_contract_version(installation_id, "1flowbase.provider/v1")
        .await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );

    let error = service
        .assign_plugin(AssignPluginCommand {
            actor_user_id,
            installation_id,
        })
        .await
        .expect_err("legacy provider contract must not be workspace assignable");

    assert!(error
        .to_string()
        .contains("plugin_assignment_not_supported"));
    assert!(repository
        .list_assignments(workspace_id)
        .await
        .unwrap()
        .is_empty());
}

async fn seed_data_source_runtime_installation() -> (
    PluginManagementService<MemoryPluginManagementRepository, MemoryProviderRuntime>,
    MemoryPluginManagementRepository,
    Uuid,
    Uuid,
    Uuid,
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let actor_user_id = repository.actor.user_id;
    let runtime = MemoryProviderRuntime::default();
    let install_root =
        std::env::temp_dir().join(format!("plugin-data-source-assignment-{}", Uuid::now_v7()));
    let installed_path = install_root.join("installed/http_source/0.1.0");
    fs::create_dir_all(&installed_path).unwrap();
    fs::create_dir_all(installed_path.join("bin")).unwrap();
    fs::create_dir_all(installed_path.join("datasource")).unwrap();
    fs::write(
        installed_path.join("manifest.yaml"),
        r#"
manifest_version: 1
plugin_id: http_source@0.1.0
version: 0.1.0
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
  entry: bin/http_source
"#,
    )
    .unwrap();
    fs::write(installed_path.join("bin/http_source"), "#!/bin/sh\n").unwrap();
    fs::write(
        installed_path.join("datasource/http_source.yaml"),
        r#"
source_code: http_source
display_name: HTTP Source
auth_modes: [api_key]
capabilities: [preview_read]
supports_sync: false
supports_webhook: false
resource_kinds: [table]
config_schema: []
"#,
    )
    .unwrap();
    let manifest_fingerprint =
        plugin_framework::compute_manifest_fingerprint(&installed_path.join("manifest.yaml"))
            .await
            .unwrap();
    fs::write(
        installed_path.join(".1flowbase-artifact.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "plugin_id": "http_source@0.1.0",
            "version": "0.1.0",
            "checksum": null,
            "manifest_fingerprint": manifest_fingerprint,
        }))
        .unwrap(),
    )
    .unwrap();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime,
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = repository
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "http_source".into(),
            plugin_id: "http_source@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.data_source/v1".into(),
            protocol: "data_source".into(),
            display_name: "HTTP Source".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::Disabled,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Disabled,
            package_path: None,
            installed_path: installed_path.display().to_string(),
            checksum: None,
            manifest_fingerprint: Some(manifest_fingerprint),
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: serde_json::json!({}),
            actor_user_id,
        })
        .await
        .unwrap()
        .id;

    (
        service,
        repository,
        actor_user_id,
        installation_id,
        workspace_id,
    )
}
