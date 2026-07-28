use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    i18n_catalog::{CatalogResolutionOrigin, CatalogResolver},
    ports::{CatalogResolutionCandidate, CatalogResolutionRepository},
};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogModuleId};

#[derive(Clone)]
struct CandidateRepository {
    candidate: Arc<Mutex<CatalogResolutionCandidate>>,
    reads: Arc<AtomicUsize>,
}

impl CandidateRepository {
    fn new(root_override: Option<&str>, active_official: Option<&str>) -> Self {
        Self {
            candidate: Arc::new(Mutex::new(CatalogResolutionCandidate {
                root_override: root_override.map(str::to_owned),
                active_official: active_official.map(str::to_owned),
            })),
            reads: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CatalogResolutionRepository for CandidateRepository {
    async fn find_catalog_resolution_candidate(
        &self,
        _workspace_id: Uuid,
        _identity: &CatalogMessageIdentity,
        _locale: &CatalogLocale,
    ) -> anyhow::Result<CatalogResolutionCandidate> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.candidate.lock().unwrap().clone())
    }
}

fn identity(msgid: &str) -> CatalogMessageIdentity {
    CatalogMessageIdentity::new(
        CatalogModuleId::new("@taichuy/platform/common").unwrap(),
        msgid,
    )
    .unwrap()
}

#[tokio::test]
async fn ac_004_exact_root_override_precedes_active_official() {
    let root_workspace_id = Uuid::now_v7();
    let repository = CandidateRepository::new(Some("根覆盖"), Some("官方"));
    let resolver = CatalogResolver::new(repository, root_workspace_id);

    let resolved = resolver
        .resolve(
            root_workspace_id,
            &identity("Save"),
            &CatalogLocale::new("zh_Hans").unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resolved.value, "根覆盖");
    assert_eq!(resolved.origin, CatalogResolutionOrigin::RootOverride);
}

#[tokio::test]
async fn ac_004_active_official_precedes_english_identity() {
    let root_workspace_id = Uuid::now_v7();
    let resolver = CatalogResolver::new(
        CandidateRepository::new(None, Some("官方")),
        root_workspace_id,
    );

    let resolved = resolver
        .resolve(
            root_workspace_id,
            &identity("Save"),
            &CatalogLocale::new("zh_Hans").unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resolved.value, "官方");
    assert_eq!(resolved.origin, CatalogResolutionOrigin::ActiveOfficial);
}

#[tokio::test]
async fn ac_004_custom_identity_and_missing_or_other_locale_fall_back_to_english() {
    let root_workspace_id = Uuid::now_v7();
    let resolver = CatalogResolver::new(CandidateRepository::new(None, None), root_workspace_id);

    for locale in ["zh_Hans", "fr_FR"] {
        let resolved = resolver
            .resolve(
                root_workspace_id,
                &identity("custom.key"),
                &CatalogLocale::new(locale).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resolved.value, "custom.key");
        assert_eq!(resolved.origin, CatalogResolutionOrigin::EnglishIdentity);
    }
}

#[tokio::test]
async fn ac_004_english_identity_and_non_root_scopes_never_hit_storage() {
    let root_workspace_id = Uuid::now_v7();
    let repository = CandidateRepository::new(Some("不可见"), Some("不可见"));
    let resolver = CatalogResolver::new(repository.clone(), root_workspace_id);

    let english = resolver
        .resolve(
            root_workspace_id,
            &identity("Save"),
            &CatalogLocale::source(),
        )
        .await
        .unwrap();
    assert_eq!(english.value, "Save");
    assert_eq!(repository.reads(), 0);

    for forbidden_scope in [
        domain::SYSTEM_SCOPE_ID,
        Uuid::now_v7(),
        Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap(),
    ] {
        assert!(resolver
            .resolve(
                forbidden_scope,
                &identity("Save"),
                &CatalogLocale::new("zh_Hans").unwrap(),
            )
            .await
            .is_err());
    }
    assert_eq!(repository.reads(), 0);
}
