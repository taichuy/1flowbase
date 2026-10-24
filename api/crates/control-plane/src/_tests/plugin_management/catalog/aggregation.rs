use std::path::Path;

use domain::{ExtensionCategory, PluginArtifactInstanceStatus, PluginAvailabilityStatus};
use time::OffsetDateTime;

use super::*;

struct CatalogSource(Vec<OfficialPluginSourceEntry>);

impl CatalogSource {
    fn for_providers<'a>(providers: impl IntoIterator<Item = &'a str>) -> Self {
        Self(
            providers
                .into_iter()
                .map(|provider_code| OfficialPluginSourceEntry {
                    plugin_id: format!("1flowbase.{provider_code}"),
                    plugin_type: "model_provider".into(),
                    provider_code: provider_code.into(),
                    namespace: format!("plugin.{provider_code}"),
                    protocol: "openai_compatible".into(),
                    latest_version: "1.0.0".into(),
                    minimum_host_version: "0.1.0".into(),
                    icon: None,
                    selected_artifact: sample_artifact("linux", "amd64", Some("musl")),
                    i18n_summary: sample_i18n_summary(),
                    release_tag: format!("{provider_code}-v1.0.0"),
                    trust_mode: "allow_unsigned".into(),
                    help_url: None,
                    model_discovery_mode: "hybrid".into(),
                })
                .collect(),
        )
    }
}

#[async_trait]
impl OfficialPluginSourcePort for CatalogSource {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        Ok(OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_kind: "official_registry".into(),
                source_label: "Catalog fixture".into(),
                registry_url: "https://example.test/catalog.json".into(),
            },
            freshness: crate::ports::OfficialPluginCatalogFreshness::Fresh,
            entries: self.0.clone(),
        })
    }

    async fn download_plugin(
        &self,
        _entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        unreachable!("catalog reads must not download packages");
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        Vec::new()
    }
}

async fn set_artifact_status(
    repository: &MemoryPluginManagementRepository,
    install_root: &Path,
    installation_id: Uuid,
    status: Option<PluginArtifactInstanceStatus>,
) {
    let node_id = format!("local:{}", install_root.display());
    let Some(status) = status else {
        repository
            .remove_artifact_snapshot(&node_id, installation_id)
            .await;
        return;
    };
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
            local_path: artifact.local_path,
            package_path: artifact.package_path,
            manifest_fingerprint: artifact.manifest_fingerprint,
            artifact_status: status,
            runtime_status: artifact.runtime_status,
            availability_status: if status == PluginArtifactInstanceStatus::Missing {
                PluginAvailabilityStatus::ArtifactMissing
            } else {
                artifact.availability_status
            },
            checked_at: artifact.checked_at,
            last_error: artifact.last_error,
            is_current: artifact.is_current,
        })
        .await
        .unwrap();
}

async fn assign(
    repository: &MemoryPluginManagementRepository,
    provider_code: &str,
    installation_id: Uuid,
) {
    repository
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id: repository.actor.current_workspace_id,
            provider_code: provider_code.into(),
            actor_user_id: repository.actor.user_id,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn official_catalog_aggregates_versions_and_preserves_category_status_boundaries() {
    use ExtensionCategory::*;
    use OfficialPluginInstallStatus::*;
    use PluginArtifactInstanceStatus::*;

    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        Uuid::now_v7(),
        &["plugin_config.view.all"],
    ));
    let install_root = std::env::temp_dir().join(format!("catalog-aggregation-{}", Uuid::now_v7()));
    let mut cases = vec![
        (
            "mixed_assigned".to_string(),
            vec![
                (RuntimeExtensions, Some(Missing), false),
                (RuntimeExtensions, Some(Ready), true),
                (RuntimeExtensions, Some(Ready), false),
            ],
            Assigned,
        ),
        (
            "mixed_installed".to_string(),
            vec![
                (RuntimeExtensions, Some(Missing), true),
                (RuntimeExtensions, Some(Ready), false),
                (RuntimeExtensions, None, false),
            ],
            Installed,
        ),
        (
            "unavailable_versions".to_string(),
            vec![
                (RuntimeExtensions, Some(Outdated), false),
                (RuntimeExtensions, Some(Mismatched), false),
                (RuntimeExtensions, Some(Corrupted), false),
                (RuntimeExtensions, Some(LoadFailed), true),
            ],
            NotInstalled,
        ),
        ("never_installed".to_string(), Vec::new(), NotInstalled),
    ];
    for (category, missing_status) in [
        (RuntimeExtensions, Uninstalled),
        (CapabilityPlugins, Uninstalled),
        (HostExtensions, NotInstalled),
        (AgentFlow, NotInstalled),
        (ApplicationsDemo, NotInstalled),
        (I18n, NotInstalled),
        (Mcp, NotInstalled),
    ] {
        for (suffix, snapshot, assigned, expected) in [
            ("ready", Some(Ready), false, Installed),
            ("assigned", Some(Ready), true, Assigned),
            ("missing", Some(Missing), true, missing_status),
            ("absent", None, true, missing_status),
        ] {
            cases.push((
                format!("{}_{suffix}", category.as_str()),
                vec![(category, snapshot, assigned)],
                expected,
            ));
        }
    }
    for (provider_code, versions, _) in &cases {
        for (index, &(category, status, assigned)) in versions.iter().enumerate() {
            let installation_id = seed_test_installation(
                &repository,
                &install_root,
                provider_code,
                &format!("0.1.{index}"),
                PluginDesiredState::Disabled,
            )
            .await;
            repository
                .set_installation_category(installation_id, category)
                .await;
            set_artifact_status(&repository, &install_root, installation_id, status).await;
            if assigned {
                assign(&repository, provider_code, installation_id).await;
            }
        }
    }
    let source = CatalogSource::for_providers(cases.iter().map(|(code, _, _)| code.as_str()));
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(source),
        &install_root,
    );
    let snapshot_updates = repository.artifact_snapshot_update_count().await;
    let catalog = service
        .list_official_catalog(
            repository.actor.user_id,
            OfficialPluginCatalogFilter {
                limit: cases.len(),
                ..OfficialPluginCatalogFilter::default()
            },
            requested_locales(),
        )
        .await
        .unwrap();

    cases.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(catalog.entries.len(), cases.len());
    for (entry, (provider_code, _, expected)) in catalog.entries.iter().zip(&cases) {
        assert_eq!(&entry.provider_code, provider_code);
        assert_eq!(entry.install_status, *expected, "provider {provider_code}");
    }
    assert_eq!(
        repository.artifact_snapshot_update_count().await,
        snapshot_updates
    );
    fs::remove_dir_all(install_root).unwrap();
}

#[tokio::test]
async fn families_preserve_provider_and_version_order_with_missing_snapshots() {
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        Uuid::now_v7(),
        &["plugin_config.view.all"],
    ));
    let install_root = std::env::temp_dir().join(format!("family-aggregation-{}", Uuid::now_v7()));
    let mut alpha_ids = Vec::new();
    let created_at = OffsetDateTime::UNIX_EPOCH;
    for (provider_code, version, age_seconds, status, assigned) in [
        (
            "zeta",
            "0.1.0",
            0,
            Some(PluginArtifactInstanceStatus::Ready),
            true,
        ),
        (
            "alpha",
            "0.1.0",
            0,
            Some(PluginArtifactInstanceStatus::Ready),
            true,
        ),
        (
            "alpha",
            "0.2.0",
            2,
            Some(PluginArtifactInstanceStatus::Ready),
            false,
        ),
        (
            "alpha",
            "0.3.0",
            1,
            Some(PluginArtifactInstanceStatus::Missing),
            false,
        ),
        ("alpha", "0.4.0", 1, None, false),
        ("missing_current", "0.1.0", 0, None, true),
    ] {
        let installation_id = seed_test_installation(
            &repository,
            &install_root,
            provider_code,
            version,
            PluginDesiredState::Disabled,
        )
        .await;
        repository
            .set_installation_created_at(
                installation_id,
                created_at + time::Duration::seconds(age_seconds),
            )
            .await;
        set_artifact_status(&repository, &install_root, installation_id, status).await;
        if assigned {
            assign(&repository, provider_code, installation_id).await;
        }
        if provider_code == "alpha" {
            alpha_ids.push(installation_id);
        }
    }
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(CatalogSource::for_providers([
            "zeta",
            "alpha",
            "missing_current",
        ])),
        &install_root,
    );
    let snapshot_updates = repository.artifact_snapshot_update_count().await;
    let families = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap();

    assert_eq!(
        families
            .entries
            .iter()
            .map(|family| family.provider_code.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
    let alpha = &families.entries[0];
    assert_eq!(alpha.current_installation_id, alpha_ids[0]);
    assert_eq!(alpha.current_version, "0.1.0");
    assert_eq!(
        alpha.installation_status,
        OfficialPluginInstallStatus::Assigned
    );
    assert_eq!(alpha.latest_version.as_deref(), Some("1.0.0"));
    assert!(alpha.has_update);
    assert_eq!(
        alpha.default_base_url.as_deref(),
        Some("https://api.example.com")
    );
    assert!(families.i18n_catalog["plugin.alpha"].contains_key("en_US"));
    assert!(families.i18n_catalog["plugin.zeta"].contains_key("en_US"));
    assert_eq!(
        alpha
            .installed_versions
            .iter()
            .map(|version| version.installation_id)
            .collect::<Vec<_>>(),
        vec![
            alpha_ids[1],
            alpha_ids[2].max(alpha_ids[3]),
            alpha_ids[2].min(alpha_ids[3]),
            alpha_ids[0]
        ]
    );
    for version in &alpha.installed_versions {
        assert_eq!(version.is_current, version.installation_id == alpha_ids[0]);
        assert_eq!(version.source_kind, "official_registry");
        assert_eq!(version.desired_state, "disabled");
        if [alpha_ids[2], alpha_ids[3]].contains(&version.installation_id) {
            assert_eq!(
                version.local_artifact.artifact_status,
                PluginArtifactInstanceStatus::Missing
            );
            assert_eq!(version.availability_status, "artifact_missing");
        } else {
            assert_eq!(
                version.local_artifact.artifact_status,
                PluginArtifactInstanceStatus::Ready
            );
            assert_eq!(version.availability_status, "disabled");
        }
        if version.installation_id == alpha_ids[3] {
            assert_eq!(
                version.local_artifact.last_error.as_deref(),
                Some("artifact_snapshot_missing")
            );
        }
    }
    assert_eq!(families.entries[1].installed_versions.len(), 1);
    assert_eq!(
        repository.artifact_snapshot_update_count().await,
        snapshot_updates
    );

    let filtered = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter {
                plugin_type: Some("network_egress_provider".into()),
            },
            requested_locales(),
        )
        .await
        .unwrap();
    assert!(filtered.entries.is_empty());

    assign(&repository, "dangling", Uuid::now_v7()).await;
    let error = service
        .list_families(
            repository.actor.user_id,
            PluginCatalogFilter::default(),
            requested_locales(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("plugin_installation"))
    ));
    fs::remove_dir_all(install_root).unwrap();
}
