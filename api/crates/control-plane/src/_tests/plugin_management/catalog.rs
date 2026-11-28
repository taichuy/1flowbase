use std::{fs, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    i18n::RequestedLocales,
    plugin_management::{
        OfficialPluginCatalogFilter, OfficialPluginInstallStatus, PluginCatalogFilter,
        PluginManagementService, RefreshCurrentNodePluginArtifactCommand,
        RefreshPluginPackageCatalogProjectionCommand,
    },
    ports::{
        CreatePluginAssignmentInput, DownloadedOfficialPluginPackage,
        OfficialPluginCatalogSnapshot, OfficialPluginCatalogSource, OfficialPluginSourceEntry,
        OfficialPluginSourcePort, PluginRepository, UpsertPluginArtifactInstanceInput,
    },
};
use domain::PluginDesiredState;

use super::support::{
    actor_with_permissions, requested_locales, sample_artifact, sample_i18n_summary,
    seed_test_installation, MemoryOfficialPluginSource, MemoryPluginManagementRepository,
    MemoryProviderRuntime,
};

#[path = "catalog/aggregation.rs"]
mod aggregation;

#[tokio::test]
async fn ac_001_ac_003_family_fetches_current_catalog_and_reports_openai_update() {
    #[derive(Clone)]
    struct NewerOfficialSource;

    #[async_trait]
    impl OfficialPluginSourcePort for NewerOfficialSource {
        async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
            Ok(OfficialPluginCatalogSnapshot {
                source: OfficialPluginCatalogSource {
                    source_kind: "official_registry".into(),
                    source_label: "官方源".into(),
                    registry_url: "https://example.com/official-registry.json".into(),
                },
                freshness: crate::ports::OfficialPluginCatalogFreshness::Fresh,
                entries: vec![OfficialPluginSourceEntry {
                    plugin_id: "1flowbase.openai_compatible".into(),
                    plugin_type: "model_provider".into(),
                    provider_code: "openai_compatible".into(),
                    namespace: "plugin.openai_compatible".into(),
                    protocol: "openai_compatible".into(),
                    latest_version: "0.2.26".into(),
                    minimum_host_version: "0.1.0".into(),
                    icon: None,
                    selected_artifact: sample_artifact("linux", "amd64", Some("musl")),
                    i18n_summary: sample_i18n_summary(),
                    release_tag: "openai-v0.2.26".into(),
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
            unreachable!("download is not used in this read-only test");
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
    let install_root = std::env::temp_dir().join(format!("plugin-family-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(NewerOfficialSource),
        &install_root,
    );

    let installation_v1 = seed_test_installation(
        &repository,
        &install_root,
        "openai_compatible",
        "0.2.25",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installation_v1,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: "openai_compatible".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    let families = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert_eq!(families.entries.len(), 1);
    assert_eq!(families.entries[0].provider_code, "openai_compatible");
    assert_eq!(families.entries[0].current_version, "0.2.25");
    assert_eq!(
        families.entries[0].latest_version.as_deref(),
        Some("0.2.26")
    );
    assert!(families.entries[0].has_update);
}

#[tokio::test]
async fn ac_001_network_egress_catalog_projects_installed_versions() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::settings_feature("system.network-center").unwrap(),
            "network_egress_plugins.families.view",
        )
        .await;
    let install_root =
        std::env::temp_dir().join(format!("network-egress-catalog-{}", Uuid::now_v7()));
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "clash-proxy",
        "0.2.3",
        PluginDesiredState::Disabled,
    )
    .await;
    repository
        .mark_installation_as_network_egress_provider(installation_id)
        .await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    )
    .for_network_egress_provider_console_operation("network_egress_plugins.families.view");

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter {
                plugin_type: Some("network_egress_provider".to_string()),
            },
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].plugin_type, "network_egress_provider");
    assert_eq!(catalog.entries[0].installation.id, installation_id);
    assert_eq!(catalog.entries[0].installation.plugin_version, "0.2.3");
}

#[tokio::test]
async fn publisher_cutover_family_reports_latest_unknown_without_verified_catalog() {
    #[derive(Clone)]
    struct UnavailableOfficialSource;

    #[async_trait]
    impl OfficialPluginSourcePort for UnavailableOfficialSource {
        async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
            anyhow::bail!("official plugin registry returned an error status")
        }

        async fn download_plugin(
            &self,
            _entry: &OfficialPluginSourceEntry,
        ) -> Result<DownloadedOfficialPluginPackage> {
            unreachable!("download is not used in this read-only test");
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
        std::env::temp_dir().join(format!("plugin-family-offline-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(UnavailableOfficialSource),
        &install_root,
    );

    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "openai_compatible",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: "openai_compatible".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    let families = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(families.entries.len(), 1);
    assert_eq!(families.entries[0].provider_code, "openai_compatible");
    assert_eq!(families.entries[0].current_version, "0.1.0");
    assert_eq!(families.entries[0].latest_version, None);
    assert!(!families.entries[0].has_update);
}

#[tokio::test]
async fn plugin_management_service_list_catalog_does_not_refresh_artifact_snapshot() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-catalog-readonly-{}", Uuid::now_v7()));
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
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    let artifact_update_count = repository.artifact_snapshot_update_count().await;

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].provider_label_key, "provider.label");
    assert_eq!(catalog.entries[0].model_discovery_mode, "hybrid");
    assert!(catalog.entries[0].assigned_to_current_workspace);
    assert!(catalog.i18n_catalog["plugin.fixture_provider"].contains_key("en_US"));
    assert_eq!(
        repository.artifact_snapshot_update_count().await,
        artifact_update_count
    );
}

#[tokio::test]
async fn plugin_management_service_list_catalog_returns_missing_projection_without_package_read() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let install_root = std::env::temp_dir().join(format!(
        "plugin-catalog-missing-projection-{}",
        Uuid::now_v7()
    ));
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
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository.remove_catalog_projection(installation_id).await;
    let artifact_update_count = repository.artifact_snapshot_update_count().await;
    let artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            installation_id,
        )
        .await
        .unwrap()
        .expect("artifact should exist");
    fs::remove_dir_all(artifact.local_path.unwrap()).unwrap();

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].catalog_refresh_status, "missing");
    assert_eq!(catalog.entries[0].model_discovery_mode, "unknown");
    assert!(catalog.entries[0].help_url.is_none());
    assert_eq!(
        repository.artifact_snapshot_update_count().await,
        artifact_update_count
    );
}

#[tokio::test]
async fn plugin_management_service_refresh_catalog_projection_uses_current_node_artifact() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let root_a = std::env::temp_dir().join(format!("plugin-projection-node-a-{}", Uuid::now_v7()));
    let root_b = std::env::temp_dir().join(format!("plugin-projection-node-b-{}", Uuid::now_v7()));
    let service_b = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root_b,
    )
    .with_node_id("node-b");
    let installation_id = seed_test_installation(
        &repository,
        &root_a,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;

    let error = service_b
        .refresh_catalog_projection(RefreshPluginPackageCatalogProjectionCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PluginUnavailable)
    ));
}

#[tokio::test]
async fn plugin_management_service_list_families_reads_projection_instead_of_local_package() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-family-projection-{}", Uuid::now_v7()));
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
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    service
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
        })
        .await
        .unwrap();
    let artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            installation_id,
        )
        .await
        .unwrap()
        .expect("artifact should exist");
    fs::remove_dir_all(artifact.local_path.unwrap()).unwrap();

    let families = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(families.entries.len(), 1);
    assert!(families.i18n_catalog["plugin.fixture_provider"].contains_key("en_US"));
    assert_eq!(
        families.entries[0].default_base_url.as_deref(),
        Some("https://api.example.com")
    );
}

#[tokio::test]
async fn plugin_management_service_reconcile_all_installations_backfills_missing_catalog_projection(
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let install_root = std::env::temp_dir().join(format!(
        "plugin-catalog-backfill-projection-{}",
        Uuid::now_v7()
    ));
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
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository.remove_catalog_projection(installation_id).await;

    service.reconcile_all_installations().await.unwrap();

    let projection = repository
        .get_plugin_package_catalog_projection(installation_id)
        .await
        .unwrap()
        .expect("catalog projection should be backfilled");
    assert_eq!(projection.projection_status.as_str(), "ok");

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].catalog_refresh_status, "ok");
    assert_eq!(catalog.entries[0].model_discovery_mode, "hybrid");
    assert_eq!(
        catalog.entries[0].default_base_url.as_deref(),
        Some("https://api.example.com")
    );
}

#[tokio::test]
async fn plugin_management_service_reconcile_all_installations_loads_assigned_provider_runtime() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let install_root = std::env::temp_dir().join(format!(
        "plugin-reconcile-assigned-runtime-{}",
        Uuid::now_v7()
    ));
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repository.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let assigned_installation_id = seed_test_installation(
        &repository,
        &install_root,
        "fixture_provider",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let unassigned_installation_id = seed_test_installation(
        &repository,
        &install_root,
        "historical_provider",
        "0.0.9",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: assigned_installation_id,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    service.reconcile_all_installations().await.unwrap();

    assert_eq!(
        runtime.loaded_installations().await,
        vec![assigned_installation_id]
    );
    let assigned_artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            assigned_installation_id,
        )
        .await
        .unwrap()
        .expect("assigned artifact should exist");
    assert_eq!(assigned_artifact.runtime_status.as_str(), "active");
    assert_eq!(assigned_artifact.availability_status.as_str(), "available");
    let unassigned_artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            unassigned_installation_id,
        )
        .await
        .unwrap()
        .expect("historical artifact should exist");
    assert_eq!(unassigned_artifact.runtime_status.as_str(), "inactive");
    assert_eq!(
        unassigned_artifact.availability_status.as_str(),
        "install_incomplete"
    );
}

#[tokio::test]
async fn publisher_cutover_family_uses_normalized_latest_official_entry_per_provider() {
    #[derive(Clone)]
    struct DuplicateOfficialSource;

    #[async_trait]
    impl OfficialPluginSourcePort for DuplicateOfficialSource {
        async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
            Ok(OfficialPluginCatalogSnapshot {
                source: OfficialPluginCatalogSource {
                    source_kind: "official_registry".into(),
                    source_label: "官方源".into(),
                    registry_url: "https://example.com/official-registry.json".into(),
                },
                freshness: crate::ports::OfficialPluginCatalogFreshness::Fresh,
                entries: vec![
                    OfficialPluginSourceEntry {
                        plugin_id: "1flowbase.openai_compatible".into(),
                        plugin_type: "model_provider".into(),
                        provider_code: "openai_compatible".into(),
                        namespace: "plugin.openai_compatible".into(),
                        protocol: "openai_compatible".into(),
                        latest_version: "0.2.0".into(),
                        minimum_host_version: "0.1.0".into(),
                        icon: None,
                        selected_artifact: sample_artifact("linux", "amd64", Some("musl")),
                        i18n_summary: sample_i18n_summary(),
                        release_tag: "openai_compatible-v0.2.0".into(),
                        trust_mode: "allow_unsigned".into(),
                        help_url: Some("https://example.com/help".into()),
                        model_discovery_mode: "hybrid".into(),
                    },
                    OfficialPluginSourceEntry {
                        plugin_id: "1flowse.openai_compatible".into(),
                        plugin_type: "model_provider".into(),
                        provider_code: "openai_compatible".into(),
                        namespace: "plugin.openai_compatible".into(),
                        protocol: "openai_compatible".into(),
                        latest_version: "0.1.0".into(),
                        minimum_host_version: "0.1.0".into(),
                        icon: None,
                        selected_artifact: sample_artifact("linux", "amd64", Some("musl")),
                        i18n_summary: sample_i18n_summary(),
                        release_tag: "openai_compatible-v0.1.0".into(),
                        trust_mode: "allow_unsigned".into(),
                        help_url: Some("https://example.com/help".into()),
                        model_discovery_mode: "hybrid".into(),
                    },
                ],
            })
        }

        async fn cached_official_catalog(&self) -> Option<OfficialPluginCatalogSnapshot> {
            self.list_official_catalog().await.ok()
        }

        async fn download_plugin(
            &self,
            _entry: &OfficialPluginSourceEntry,
        ) -> Result<DownloadedOfficialPluginPackage> {
            unreachable!("download is not used in this read-only test");
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
    let install_root = std::env::temp_dir().join(format!("plugin-family-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(DuplicateOfficialSource),
        &install_root,
    );

    let installation_v1 = seed_test_installation(
        &repository,
        &install_root,
        "openai_compatible",
        "0.1.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installation_v1,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: "openai_compatible".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();

    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].plugin_id, "1flowbase.openai_compatible");
    assert_eq!(catalog.entries[0].latest_version, "0.2.0");

    let families = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    assert_eq!(families.entries.len(), 1);
    assert_eq!(families.entries[0].current_version, "0.1.0");
    assert_eq!(families.entries[0].latest_version.as_deref(), Some("0.2.0"));
    assert!(families.entries[0].has_update);
}

#[tokio::test]
async fn plugin_management_service_uses_persisted_missing_artifact_snapshot_for_catalog_fallback() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-missing-catalog-{}", Uuid::now_v7()));
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
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let node_id = format!("local:{}", install_root.display());
    let artifact = repository
        .get_artifact_instance(&node_id, installation_id)
        .await
        .unwrap()
        .unwrap();
    let install_path = artifact.local_path.clone().unwrap();
    fs::remove_dir_all(&install_path).unwrap();
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id,
            installation_id,
            local_version: artifact.local_version,
            local_checksum: artifact.local_checksum,
            local_path: Some(install_path),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Missing,
            runtime_status: artifact.runtime_status,
            availability_status: domain::PluginAvailabilityStatus::ArtifactMissing,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: artifact.is_current,
        })
        .await
        .unwrap();
    let maintenance_update_count = repository.artifact_snapshot_update_count().await;

    let catalog = service
        .list_catalog(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();
    let artifact = repository
        .get_artifact_instance(
            &format!("local:{}", install_root.display()),
            installation_id,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(
        catalog.entries[0].local_artifact.artifact_status,
        domain::PluginArtifactInstanceStatus::Missing
    );
    assert_eq!(
        catalog.entries[0].local_artifact.availability_status,
        domain::PluginAvailabilityStatus::ArtifactMissing
    );
    assert_eq!(catalog.entries[0].model_discovery_mode, "hybrid");
    assert_eq!(
        artifact.availability_status,
        domain::PluginAvailabilityStatus::ArtifactMissing
    );
    assert_eq!(
        repository.artifact_snapshot_update_count().await,
        maintenance_update_count
    );

    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    let families = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert!(families.entries.is_empty());
}

#[tokio::test]
async fn ac_1785_missing_artifact_is_uninstalled_in_the_official_catalog() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all", "plugin_config.configure.all"],
    ));
    let install_root =
        std::env::temp_dir().join(format!("plugin-official-uninstalled-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &install_root,
    );
    let installation_id = seed_test_installation(
        &repository,
        &install_root,
        "openai_compatible",
        "0.2.1",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id,
            provider_code: "openai_compatible".into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
    let node_id = format!("local:{}", install_root.display());
    let artifact = repository
        .get_artifact_instance(&node_id, installation_id)
        .await
        .unwrap()
        .unwrap();
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id,
            installation_id,
            local_version: artifact.local_version,
            local_checksum: artifact.local_checksum,
            local_path: None,
            package_path: None,
            manifest_fingerprint: artifact.manifest_fingerprint,
            artifact_status: domain::PluginArtifactInstanceStatus::Missing,
            runtime_status: domain::PluginRuntimeStatus::Inactive,
            availability_status: domain::PluginAvailabilityStatus::ArtifactMissing,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: Some("artifact_missing".into()),
            is_current: false,
        })
        .await
        .unwrap();

    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(
        catalog.entries[0].install_status,
        OfficialPluginInstallStatus::Uninstalled
    );
}

#[tokio::test]
async fn list_official_catalog_filters_by_plugin_type_and_returns_localized_items() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("plugin-list-{}", Uuid::now_v7())),
    );

    let view = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter {
                plugin_type: Some("model_provider".into()),
                ..OfficialPluginCatalogFilter::default()
            },
            RequestedLocales::new("zh_Hans", "en_US"),
        )
        .await
        .unwrap();
    let entry = &view.entries[0];

    let reference = OfficialPluginSourceEntry {
        plugin_id: "1flowbase.openai_compatible".into(),
        plugin_type: "model_provider".into(),
        provider_code: "openai_compatible".into(),
        namespace: "plugin.openai_compatible".into(),
        protocol: "openai_compatible".into(),
        latest_version: "0.2.1".into(),
        minimum_host_version: "0.1.0".into(),
        icon: None,
        selected_artifact: sample_artifact("linux", "amd64", Some("musl")),
        i18n_summary: sample_i18n_summary(),
        release_tag: "openai_compatible-v0.2.1".into(),
        trust_mode: "signature_required".into(),
        help_url: Some("https://example.test/help".into()),
        model_discovery_mode: "hybrid".into(),
    };

    assert_eq!(view.entries.len(), 1);
    assert_eq!(entry.plugin_type, reference.plugin_type);
    assert_eq!(entry.display_name, "OpenAI-Compatible API Provider");
    assert_eq!(view.page.limit, 20);
    assert!(view.page.next_cursor.is_none());
}

#[tokio::test]
async fn ac_1281_plugin_catalog_policy_only_allows_without_legacy_grant() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryPluginManagementRepository::new(actor_with_permissions(workspace_id, &[]));
    repository
        .set_console_operation(
            domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
            "plugins.families.view",
        )
        .await;
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("plugin-policy-only-{}", Uuid::now_v7())),
    )
    .for_plugin_console_operation(
        domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
        "plugins.families.view",
    );

    service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .expect("the exact plugin operation must authorize the catalog owner");
}

#[tokio::test]
async fn ac_1281_plugin_catalog_legacy_only_does_not_authorize() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.view.all"],
    ));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        std::env::temp_dir().join(format!("plugin-legacy-only-{}", Uuid::now_v7())),
    )
    .for_plugin_console_operation(
        domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
        "plugins.families.view",
    );

    assert!(service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .is_err());
}

#[tokio::test]
async fn boot_retires_superseded_versions_resets_ghosts_and_skips_unchanged_history() {
    use domain::{PluginAvailabilityStatus, PluginRuntimeStatus};

    let repo = MemoryPluginManagementRepository::new(actor_with_permissions(Uuid::now_v7(), &[]));
    let root = std::env::temp_dir().join(format!("plugin-boot-history-{}", Uuid::now_v7()));
    let node = format!("local:{}", root.display());
    let runtime = MemoryProviderRuntime::default();
    let service = PluginManagementService::new(
        repo.clone(),
        runtime.clone(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root,
    );
    let mut ids = Vec::new();
    for (family, version, state) in [
        (
            "fixture_provider",
            "0.1.0",
            PluginDesiredState::ActiveRequested,
        ),
        (
            "fixture_provider",
            "0.2.0",
            PluginDesiredState::ActiveRequested,
        ),
        ("fixture_provider", "0.0.9", PluginDesiredState::Disabled),
        (
            "fresh_provider",
            "1.0.0",
            PluginDesiredState::ActiveRequested,
        ),
        (
            "builtin_provider",
            "1.0.0",
            PluginDesiredState::ActiveRequested,
        ),
        ("host_provider", "1.0.0", PluginDesiredState::Disabled),
    ] {
        let id = seed_test_installation(&repo, &root, family, version, state).await;
        // Seed a pre-fence persisted runtime snapshot. In particular, Disabled + Active
        // must remain representable in old data so startup reconciliation can repair it.
        let artifact = repo
            .get_artifact_instance(&node, id)
            .await
            .unwrap()
            .unwrap();
        repo.upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: artifact.node_id,
            installation_id: id,
            local_version: artifact.local_version,
            local_checksum: artifact.local_checksum,
            local_path: artifact.local_path,
            package_path: artifact.package_path,
            manifest_fingerprint: artifact.manifest_fingerprint,
            artifact_status: artifact.artifact_status,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: crate::plugin_lifecycle::derive_availability_status(
                state,
                domain::PluginArtifactStatus::Ready,
                PluginRuntimeStatus::Active,
            ),
            checked_at: artifact.checked_at,
            last_error: artifact.last_error,
            is_current: artifact.is_current,
        })
        .await
        .unwrap();
        ids.push(id);
    }
    repo.mark_installation_builtin(ids[4]).await;
    repo.set_installation_category(ids[5], domain::ExtensionCategory::HostExtensions)
        .await;
    repo.set_installation_contract_version(
        ids[5],
        crate::host_extension::HOST_EXTENSION_CONTRACT_VERSION,
    )
    .await;
    repo.create_assignment(&CreatePluginAssignmentInput {
        installation_id: ids[1],
        workspace_id: repo.actor.current_workspace_id,
        provider_code: "fixture_provider".into(),
        actor_user_id: repo.actor.user_id,
    })
    .await
    .unwrap();
    let original_projection = repo
        .get_plugin_package_catalog_projection(ids[2])
        .await
        .unwrap()
        .unwrap();
    // Prove the healthy historical projection doesn't require a package load, even if provider
    // payload is unreadable. The manifest/marker still support the normal artifact scan.
    fs::remove_file(root.join("installed/fixture_provider/0.0.9/provider/fixture_provider.yaml"))
        .unwrap();
    service.reconcile_all_installations().await.unwrap();
    assert_eq!(runtime.loaded_installations().await, vec![ids[1]]);
    let mut artifacts = Vec::new();
    for (index, (desired, status, availability)) in [
        (
            PluginDesiredState::Disabled,
            PluginRuntimeStatus::Inactive,
            PluginAvailabilityStatus::Disabled,
        ),
        (
            PluginDesiredState::ActiveRequested,
            PluginRuntimeStatus::Active,
            PluginAvailabilityStatus::Available,
        ),
        (
            PluginDesiredState::Disabled,
            PluginRuntimeStatus::Inactive,
            PluginAvailabilityStatus::Disabled,
        ),
        (
            PluginDesiredState::ActiveRequested,
            PluginRuntimeStatus::Inactive,
            PluginAvailabilityStatus::InstallIncomplete,
        ),
        (
            PluginDesiredState::ActiveRequested,
            PluginRuntimeStatus::Active,
            PluginAvailabilityStatus::Available,
        ),
        (
            PluginDesiredState::Disabled,
            PluginRuntimeStatus::Active,
            PluginAvailabilityStatus::Disabled,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let id = ids[index];
        assert_eq!(
            repo.get_installation(id)
                .await
                .unwrap()
                .unwrap()
                .desired_state,
            desired
        );
        let artifact = repo
            .get_artifact_instance(&node, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (artifact.runtime_status, artifact.availability_status),
            (status, availability)
        );
        artifacts.push(artifact);
    }
    service.reconcile_all_installations().await.unwrap();
    for index in [0, 2, 3, 4, 5] {
        assert_eq!(
            repo.get_artifact_instance(&node, ids[index])
                .await
                .unwrap()
                .unwrap(),
            artifacts[index]
        );
    }
    assert_eq!(
        repo.get_plugin_package_catalog_projection(ids[2])
            .await
            .unwrap()
            .unwrap(),
        original_projection
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn boot_preserves_first_install_when_only_disabled_sibling_is_assigned() {
    let repo = MemoryPluginManagementRepository::new(actor_with_permissions(Uuid::now_v7(), &[]));
    let root = std::env::temp_dir().join(format!("plugin-disabled-sibling-{}", Uuid::now_v7()));
    let code = "fixture_provider";
    let old = seed_test_installation(
        &repo,
        &root,
        code,
        "1.0.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    let fresh = seed_test_installation(
        &repo,
        &root,
        code,
        "2.0.0",
        PluginDesiredState::ActiveRequested,
    )
    .await;
    repo.create_assignment(&CreatePluginAssignmentInput {
        installation_id: old,
        workspace_id: repo.actor.current_workspace_id,
        provider_code: code.into(),
        actor_user_id: repo.actor.user_id,
    })
    .await
    .unwrap();
    repo.update_desired_state(&crate::ports::UpdatePluginDesiredStateInput {
        installation_id: old,
        desired_state: PluginDesiredState::Disabled,
        actor_user_id: repo.actor.user_id,
    })
    .await
    .unwrap();
    let service = PluginManagementService::new(
        repo.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(MemoryOfficialPluginSource::default()),
        &root,
    );
    service.reconcile_all_installations().await.unwrap();
    assert_eq!(
        repo.get_installation(fresh)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::ActiveRequested
    );
    assert_eq!(
        repo.list_assignments(repo.actor.current_workspace_id)
            .await
            .unwrap()[0]
            .installation_id,
        old
    );
    fs::remove_dir_all(root).unwrap();
}
