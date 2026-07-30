use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    i18n_catalog::{CatalogResolutionOrigin, CatalogResolver},
    ports::{CatalogResolutionCandidate, CatalogResolutionRepository},
};
use domain::{CatalogLocale, CatalogMessageIdentity};

#[derive(Clone)]
struct CandidateRepository {
    candidates: Arc<Vec<CatalogResolutionCandidate>>,
    reads: Arc<AtomicUsize>,
    fail_at: Option<usize>,
}

impl CandidateRepository {
    fn new(candidates: Vec<CatalogResolutionCandidate>) -> Self {
        Self {
            candidates: Arc::new(candidates),
            reads: Arc::new(AtomicUsize::new(0)),
            fail_at: None,
        }
    }

    fn failing(fail_at: usize) -> Self {
        Self {
            fail_at: Some(fail_at),
            ..Self::new(vec![])
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
        let read = self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail_at == Some(read) {
            anyhow::bail!("controlled repository failure");
        }
        Ok(self.candidates.get(read).cloned().unwrap_or_else(empty))
    }
}

fn candidate(
    root_override: Option<&str>,
    active_official: Option<&str>,
) -> CatalogResolutionCandidate {
    CatalogResolutionCandidate {
        root_override: root_override.map(str::to_owned),
        active_official: active_official.map(str::to_owned),
    }
}

fn empty() -> CatalogResolutionCandidate {
    candidate(None, None)
}

fn identity(key: &str) -> CatalogMessageIdentity {
    CatalogMessageIdentity::new(key).unwrap()
}

#[tokio::test]
async fn requested_override_and_official_precede_english_fallbacks() {
    let root_workspace_id = Uuid::now_v7();
    for (requested, expected_value, expected_origin) in [
        (
            candidate(Some("请求覆盖"), Some("请求官方")),
            "请求覆盖",
            CatalogResolutionOrigin::RequestedWorkspaceOverride,
        ),
        (
            candidate(None, Some("请求官方")),
            "请求官方",
            CatalogResolutionOrigin::RequestedOfficial,
        ),
    ] {
        let repository =
            CandidateRepository::new(vec![requested, candidate(Some("英文覆盖"), None)]);
        let resolved = CatalogResolver::new(repository.clone(), root_workspace_id)
            .resolve(
                root_workspace_id,
                &identity("Save"),
                &CatalogLocale::new("zh_Hans").unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resolved.value, expected_value);
        assert_eq!(resolved.origin, expected_origin);
        assert_eq!(repository.reads(), 1);
    }
}

#[tokio::test]
async fn english_override_and_official_precede_raw_key() {
    let root_workspace_id = Uuid::now_v7();
    for (english, expected_value, expected_origin) in [
        (
            candidate(Some("English override"), Some("English official")),
            "English override",
            CatalogResolutionOrigin::EnglishWorkspaceOverride,
        ),
        (
            candidate(None, Some("English official")),
            "English official",
            CatalogResolutionOrigin::EnglishOfficial,
        ),
        (empty(), "custom.key", CatalogResolutionOrigin::RawKey),
    ] {
        let repository = CandidateRepository::new(vec![empty(), english]);
        let resolved = CatalogResolver::new(repository.clone(), root_workspace_id)
            .resolve(
                root_workspace_id,
                &identity("custom.key"),
                &CatalogLocale::new("fr_FR").unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resolved.value, expected_value);
        assert_eq!(resolved.origin, expected_origin);
        assert_eq!(repository.reads(), 2);
    }
}

#[tokio::test]
async fn source_locale_uses_stored_translation_and_repository_errors_propagate() {
    let root_workspace_id = Uuid::now_v7();
    let repository = CandidateRepository::new(vec![candidate(None, Some("Stored English"))]);
    let resolved = CatalogResolver::new(repository.clone(), root_workspace_id)
        .resolve(
            root_workspace_id,
            &identity("settings.title"),
            &CatalogLocale::source(),
        )
        .await
        .unwrap();
    assert_eq!(resolved.value, "Stored English");
    assert_eq!(resolved.origin, CatalogResolutionOrigin::RequestedOfficial);
    assert_eq!(repository.reads(), 1);

    let error = CatalogResolver::new(CandidateRepository::failing(1), root_workspace_id)
        .resolve(
            root_workspace_id,
            &identity("settings.title"),
            &CatalogLocale::new("zh_Hans").unwrap(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("controlled repository failure"));
}

#[tokio::test]
async fn non_root_scopes_never_hit_storage() {
    let root_workspace_id = Uuid::now_v7();
    let repository = CandidateRepository::new(vec![candidate(Some("hidden"), Some("hidden"))]);
    let resolver = CatalogResolver::new(repository.clone(), root_workspace_id);

    for forbidden_scope in [domain::SYSTEM_SCOPE_ID, Uuid::now_v7()] {
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
