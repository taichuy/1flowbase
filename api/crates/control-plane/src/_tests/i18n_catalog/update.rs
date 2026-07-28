use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{bail, Result};
use async_trait::async_trait;
use domain::{
    CatalogDigest, CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogSeedFile,
    CatalogTranslation, CatalogVersion, OfficialCatalogMessage, VerifiedCatalogRelease,
    WorkspaceCatalogRevision, WorkspaceCatalogState,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    i18n_catalog::{
        OfficialI18nCatalogUpdateCommand, OfficialI18nCatalogUpdateOutcome,
        OfficialI18nCatalogUpdateService, VerifiedOfficialCatalogSeed,
    },
    ports::{
        DeleteCatalogTranslationInput, DeleteCustomCatalogMessageInput, I18nCatalogRepository,
        OfficialI18nCatalogReleaseDescriptor, OfficialI18nCatalogSourcePort,
        StoredI18nCatalogReleaseDescriptor, UpsertCatalogTranslationInput,
    },
};

fn digest(character: char) -> CatalogDigest {
    CatalogDigest::new(format!("sha256:{}", character.to_string().repeat(64))).unwrap()
}

fn seed(version: &str, semantic: CatalogDigest) -> VerifiedOfficialCatalogSeed {
    let module = CatalogModuleId::new("@taichuy/platform/common").unwrap();
    let target = CatalogLocale::new("zh_Hans").unwrap();
    let mut translations = BTreeMap::new();
    translations.insert(target.clone(), "保存 {name}".to_owned());
    VerifiedOfficialCatalogSeed::new(
        Uuid::now_v7(),
        CatalogVersion::new(version).unwrap(),
        vec![CatalogLocale::source(), target.clone()],
        vec![module.clone()],
        vec![
            CatalogSeedFile::new(module.clone(), target, "common/zh_Hans.json", digest('f'))
                .unwrap(),
        ],
        OffsetDateTime::UNIX_EPOCH,
        semantic,
        vec![OfficialCatalogMessage::new(
            CatalogMessageIdentity::new(module, "Save {name}").unwrap(),
            translations,
        )
        .unwrap()],
    )
    .unwrap()
}

struct FakeSource {
    descriptor: OfficialI18nCatalogReleaseDescriptor,
    seed: VerifiedOfficialCatalogSeed,
    checks: AtomicUsize,
    fetches: AtomicUsize,
    repository_transaction_open: Arc<AtomicBool>,
    fail_check: AtomicBool,
}

#[async_trait]
impl OfficialI18nCatalogSourcePort for FakeSource {
    async fn check_latest_release(&self) -> Result<OfficialI18nCatalogReleaseDescriptor> {
        assert!(!self.repository_transaction_open.load(Ordering::SeqCst));
        self.checks.fetch_add(1, Ordering::SeqCst);
        if self.fail_check.load(Ordering::SeqCst) {
            bail!("controlled source failure");
        }
        Ok(self.descriptor.clone())
    }

    async fn fetch_verified_release(
        &self,
        _: &OfficialI18nCatalogReleaseDescriptor,
    ) -> Result<VerifiedOfficialCatalogSeed> {
        assert!(!self.repository_transaction_open.load(Ordering::SeqCst));
        self.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(self.seed.clone())
    }
}

struct FakeRepositoryData {
    state: WorkspaceCatalogState,
    releases: HashMap<Uuid, StoredI18nCatalogReleaseDescriptor>,
    fail_stage: bool,
    fail_activation: bool,
    override_count: usize,
    custom_count: usize,
}

#[derive(Clone)]
struct FakeRepository {
    data: Arc<Mutex<FakeRepositoryData>>,
    transaction_open: Arc<AtomicBool>,
}

impl FakeRepository {
    fn transaction<T>(
        &self,
        action: impl FnOnce(&mut FakeRepositoryData) -> Result<T>,
    ) -> Result<T> {
        self.transaction_open.store(true, Ordering::SeqCst);
        let result = action(&mut self.data.lock().unwrap());
        self.transaction_open.store(false, Ordering::SeqCst);
        result
    }
}

#[async_trait]
impl I18nCatalogRepository for FakeRepository {
    async fn import_verified_release(&self, release: &VerifiedCatalogRelease) -> Result<()> {
        self.transaction(|data| {
            if data.fail_stage {
                bail!("controlled staging failure");
            }
            data.releases.insert(
                release.id(),
                StoredI18nCatalogReleaseDescriptor {
                    catalog_version: release.catalog_version().clone(),
                    semantic_sha256: release.semantic_sha256().clone(),
                    source_locale: CatalogLocale::source(),
                    locales: release.locales().to_vec(),
                    modules: release.modules().to_vec(),
                },
            );
            Ok(())
        })
    }

    async fn bootstrap_workspace_catalog_state(&self, _: Uuid) -> Result<WorkspaceCatalogState> {
        Ok(self.data.lock().unwrap().state.clone())
    }

    async fn activate_verified_release(
        &self,
        workspace_id: Uuid,
        release_id: Uuid,
        expected_revision: WorkspaceCatalogRevision,
    ) -> Result<WorkspaceCatalogState> {
        self.transaction(|data| {
            if data.fail_activation || data.state.revision() != expected_revision {
                bail!("controlled activation failure");
            }
            let state = WorkspaceCatalogState::restored(
                workspace_id,
                Some(release_id),
                WorkspaceCatalogRevision::new(expected_revision.value() + 1).unwrap(),
            );
            data.state = state.clone();
            Ok(state)
        })
    }

    async fn get_workspace_catalog_state(&self, _: Uuid) -> Result<Option<WorkspaceCatalogState>> {
        Ok(Some(self.data.lock().unwrap().state.clone()))
    }

    async fn get_i18n_catalog_release_descriptor(
        &self,
        _: Uuid,
        release_id: Uuid,
    ) -> Result<Option<StoredI18nCatalogReleaseDescriptor>> {
        Ok(self.data.lock().unwrap().releases.get(&release_id).cloned())
    }

    async fn list_active_official_messages(
        &self,
        _: Uuid,
    ) -> Result<Vec<domain::ActiveOfficialCatalogMessage>> {
        Ok(vec![])
    }
    async fn list_catalog_overrides(&self, _: Uuid) -> Result<Vec<CatalogTranslation>> {
        Ok(vec![])
    }
    async fn list_custom_catalog_translations(&self, _: Uuid) -> Result<Vec<CatalogTranslation>> {
        Ok(vec![])
    }
    async fn upsert_catalog_override(
        &self,
        _: &UpsertCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        bail!("not used")
    }
    async fn delete_catalog_override(
        &self,
        _: &DeleteCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        bail!("not used")
    }
    async fn upsert_custom_catalog_translation(
        &self,
        _: &UpsertCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        bail!("not used")
    }
    async fn delete_custom_catalog_message(
        &self,
        _: &DeleteCustomCatalogMessageInput,
    ) -> Result<WorkspaceCatalogState> {
        bail!("not used")
    }
    async fn mark_superseded_release_obsolete_against_active(
        &self,
        _: Uuid,
        _: Uuid,
    ) -> Result<Vec<domain::ObsoleteCatalogMessage>> {
        Ok(vec![])
    }
    async fn list_obsolete_catalog_messages(
        &self,
        _: Uuid,
    ) -> Result<Vec<domain::ObsoleteCatalogMessage>> {
        Ok(vec![])
    }
}

fn fixture(
    latest_version: &str,
    latest_semantic: CatalogDigest,
) -> (FakeRepository, Arc<FakeSource>, Uuid, Uuid) {
    let workspace_id = Uuid::now_v7();
    let active_release_id = Uuid::now_v7();
    let transaction_open = Arc::new(AtomicBool::new(false));
    let repository = FakeRepository {
        data: Arc::new(Mutex::new(FakeRepositoryData {
            state: WorkspaceCatalogState::restored(
                workspace_id,
                Some(active_release_id),
                WorkspaceCatalogRevision::new(4).unwrap(),
            ),
            releases: HashMap::from([(
                active_release_id,
                StoredI18nCatalogReleaseDescriptor {
                    catalog_version: CatalogVersion::new("1.0.0").unwrap(),
                    semantic_sha256: digest('a'),
                    source_locale: CatalogLocale::source(),
                    locales: vec![
                        CatalogLocale::source(),
                        CatalogLocale::new("zh_Hans").unwrap(),
                    ],
                    modules: vec![CatalogModuleId::new("@taichuy/platform/common").unwrap()],
                },
            )]),
            fail_stage: false,
            fail_activation: false,
            override_count: 2,
            custom_count: 3,
        })),
        transaction_open: transaction_open.clone(),
    };
    let source = Arc::new(FakeSource {
        descriptor: OfficialI18nCatalogReleaseDescriptor {
            catalog_version: CatalogVersion::new(latest_version).unwrap(),
            semantic_sha256: latest_semantic.clone(),
            seed_sha256: digest('e'),
        },
        seed: seed(latest_version, latest_semantic),
        checks: AtomicUsize::new(0),
        fetches: AtomicUsize::new(0),
        repository_transaction_open: transaction_open,
        fail_check: AtomicBool::new(false),
    });
    (repository, source, workspace_id, active_release_id)
}

#[tokio::test]
async fn ac_005_update_check_reports_current_and_newer_without_fetching() {
    for (latest, expected_available) in [("1.0.0", false), ("1.1.0", true)] {
        let (repository, source, workspace_id, _) = fixture(
            latest,
            if expected_available {
                digest('b')
            } else {
                digest('a')
            },
        );
        let service = OfficialI18nCatalogUpdateService::new(repository, source.clone());
        let status = service.check_update(workspace_id).await.unwrap();
        assert_eq!(
            matches!(
                status,
                crate::i18n_catalog::OfficialI18nCatalogUpdateStatus::UpdateAvailable { .. }
            ),
            expected_available,
        );
        assert_eq!(source.fetches.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn ac_005_update_check_maps_source_failure_to_upstream_unavailable() {
    let (repository, source, workspace_id, _) = fixture("1.1.0", digest('b'));
    source.fail_check.store(true, Ordering::SeqCst);
    let error = OfficialI18nCatalogUpdateService::new(repository, source)
        .check_update(workspace_id)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(&crate::errors::ControlPlaneError::UpstreamUnavailable(
            "official_i18n_catalog_source"
        ))
    );
}

#[tokio::test]
async fn ac_005_current_release_checks_once_without_downloading_or_writing() {
    let (repository, source, workspace_id, active_release_id) = fixture("1.0.0", digest('a'));
    let service = OfficialI18nCatalogUpdateService::new(repository.clone(), source.clone());
    let outcome = service
        .check_and_activate(OfficialI18nCatalogUpdateCommand {
            workspace_id,
            expected_revision: WorkspaceCatalogRevision::new(4).unwrap(),
        })
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        OfficialI18nCatalogUpdateOutcome::Current { .. }
    ));
    assert_eq!(
        (
            source.checks.load(Ordering::SeqCst),
            source.fetches.load(Ordering::SeqCst)
        ),
        (1, 0)
    );
    assert_eq!(
        repository.data.lock().unwrap().state.active_release_id(),
        Some(active_release_id)
    );
}

#[tokio::test]
async fn ac_005_same_version_content_drift_is_rejected_before_fetch() {
    let (repository, source, workspace_id, active_release_id) = fixture("1.0.0", digest('b'));
    let service = OfficialI18nCatalogUpdateService::new(repository.clone(), source.clone());
    assert!(service
        .check_and_activate(OfficialI18nCatalogUpdateCommand {
            workspace_id,
            expected_revision: WorkspaceCatalogRevision::new(4).unwrap(),
        })
        .await
        .is_err());
    assert_eq!(source.fetches.load(Ordering::SeqCst), 0);
    assert_eq!(
        repository.data.lock().unwrap().state.active_release_id(),
        Some(active_release_id)
    );
}

#[tokio::test]
async fn ac_005_verified_new_release_activates_and_preserves_user_layers() {
    let (repository, source, workspace_id, active_release_id) = fixture("1.1.0", digest('b'));
    let service = OfficialI18nCatalogUpdateService::new(repository.clone(), source.clone());
    let outcome = service
        .check_and_activate(OfficialI18nCatalogUpdateCommand {
            workspace_id,
            expected_revision: WorkspaceCatalogRevision::new(4).unwrap(),
        })
        .await
        .unwrap();
    let data = repository.data.lock().unwrap();
    assert!(matches!(
        outcome,
        OfficialI18nCatalogUpdateOutcome::Activated { .. }
    ));
    assert_ne!(data.state.active_release_id(), Some(active_release_id));
    assert_eq!((data.override_count, data.custom_count), (2, 3));
    assert_eq!(
        (
            source.checks.load(Ordering::SeqCst),
            source.fetches.load(Ordering::SeqCst)
        ),
        (1, 1)
    );
}

#[tokio::test]
async fn ac_005_stage_or_activation_failure_keeps_old_active_release() {
    for fail_activation in [false, true] {
        let (repository, source, workspace_id, active_release_id) = fixture("1.1.0", digest('b'));
        {
            let mut data = repository.data.lock().unwrap();
            data.fail_stage = !fail_activation;
            data.fail_activation = fail_activation;
        }
        let service = OfficialI18nCatalogUpdateService::new(repository.clone(), source);
        assert!(service
            .check_and_activate(OfficialI18nCatalogUpdateCommand {
                workspace_id,
                expected_revision: WorkspaceCatalogRevision::new(4).unwrap(),
            })
            .await
            .is_err());
        assert_eq!(
            repository.data.lock().unwrap().state.active_release_id(),
            Some(active_release_id)
        );
    }
}

#[tokio::test]
async fn ac_005_stale_revision_rejects_before_any_network_call() {
    let (repository, source, workspace_id, active_release_id) = fixture("1.1.0", digest('b'));
    let service = OfficialI18nCatalogUpdateService::new(repository.clone(), source.clone());
    assert!(service
        .check_and_activate(OfficialI18nCatalogUpdateCommand {
            workspace_id,
            expected_revision: WorkspaceCatalogRevision::new(3).unwrap(),
        })
        .await
        .is_err());
    assert_eq!(
        (
            source.checks.load(Ordering::SeqCst),
            source.fetches.load(Ordering::SeqCst)
        ),
        (0, 0)
    );
    assert_eq!(
        repository.data.lock().unwrap().state.active_release_id(),
        Some(active_release_id)
    );
}
