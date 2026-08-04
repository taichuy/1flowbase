use std::{fs, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    i18n::RequestedLocales,
    plugin_management::{
        OfficialPluginCatalogFilter, PluginCatalogFilter, PluginManagementService,
        RefreshCurrentNodePluginArtifactCommand, RefreshPluginPackageCatalogProjectionCommand,
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

#[tokio::test]
async fn publisher_cutover_family_uses_semver_for_older_official_version() {
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
                    latest_version: "0.2.0".into(),
                    minimum_host_version: "0.1.0".into(),
                    icon: None,
                    selected_artifact: sample_artifact("linux", "amd64", Some("musl")),
                    i18n_summary: sample_i18n_summary(),
                    release_tag: "openai_compatible-v0.2.0".into(),
                    trust_mode: "allow_unsigned".into(),
                    help_url: Some("https://example.com/help".into()),
                    model_discovery_mode: "hybrid".into(),
                }],
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
        Arc::new(NewerOfficialSource),
        &install_root,
    );

    let installation_v1 = seed_test_installation(
        &repository,
        &install_root,
        "openai_compatible",
        "0.10.0",
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
    assert_eq!(families.entries[0].current_version, "0.10.0");
    assert_eq!(families.entries[0].latest_version.as_deref(), Some("0.2.0"));
    assert!(!families.entries[0].has_update);
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
async fn ac_001_plugin_management_service_lists_provider_families_without_waiting_for_official_catalog(
) {
    #[derive(Clone)]
    struct BlockingOfficialSource;

    #[async_trait]
    impl OfficialPluginSourcePort for BlockingOfficialSource {
        async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
            std::future::pending().await
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
        std::env::temp_dir().join(format!("plugin-family-nonblocking-{}", Uuid::now_v7()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(BlockingOfficialSource),
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

    let families = tokio::time::timeout(
        Duration::from_millis(250),
        service.list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        ),
    )
    .await
    .expect("AC-001: local provider families must not wait for the official registry")
    .unwrap();

    assert_eq!(families.entries.len(), 1);
    assert_eq!(families.entries[0].provider_code, "openai_compatible");
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
        Some(ControlPlaneError::Conflict("plugin_artifact_missing"))
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
