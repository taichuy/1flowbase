use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialI18nCatalogReleaseDescriptor {
    pub catalog_version: domain::CatalogVersion,
    pub semantic_sha256: domain::CatalogDigest,
    pub seed_sha256: domain::CatalogDigest,
}

#[async_trait]
pub trait OfficialI18nCatalogSourcePort: Send + Sync {
    async fn check_latest_release(&self) -> anyhow::Result<OfficialI18nCatalogReleaseDescriptor>;

    async fn fetch_verified_release(
        &self,
        release: &OfficialI18nCatalogReleaseDescriptor,
    ) -> anyhow::Result<crate::i18n_catalog::VerifiedOfficialCatalogSeed>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredI18nCatalogReleaseDescriptor {
    pub catalog_version: domain::CatalogVersion,
    pub semantic_sha256: domain::CatalogDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogResolutionCandidate {
    pub root_override: Option<String>,
    pub active_official: Option<String>,
}

#[async_trait]
pub trait CatalogResolutionRepository: Send + Sync {
    async fn find_catalog_resolution_candidate(
        &self,
        workspace_id: Uuid,
        identity: &domain::CatalogMessageIdentity,
        locale: &domain::CatalogLocale,
    ) -> anyhow::Result<CatalogResolutionCandidate>;
}

#[derive(Debug, Clone)]
pub struct UpsertCatalogTranslationInput {
    pub workspace_id: Uuid,
    pub value: domain::CatalogTranslation,
    pub expected_revision: domain::WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct DeleteCatalogTranslationInput {
    pub workspace_id: Uuid,
    pub identity: domain::CatalogMessageIdentity,
    pub locale: domain::CatalogLocale,
    pub expected_revision: domain::WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct DeleteCustomCatalogMessageInput {
    pub workspace_id: Uuid,
    pub identity: domain::CatalogMessageIdentity,
    pub expected_revision: domain::WorkspaceCatalogRevision,
}

#[async_trait]
pub trait I18nCatalogRepository: Send + Sync {
    async fn import_verified_release(
        &self,
        release: &domain::VerifiedCatalogRelease,
    ) -> anyhow::Result<()>;

    async fn bootstrap_workspace_catalog_state(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn activate_verified_release(
        &self,
        workspace_id: Uuid,
        release_id: Uuid,
        expected_revision: domain::WorkspaceCatalogRevision,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn get_workspace_catalog_state(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Option<domain::WorkspaceCatalogState>>;

    async fn get_i18n_catalog_release_descriptor(
        &self,
        workspace_id: Uuid,
        release_id: Uuid,
    ) -> anyhow::Result<Option<StoredI18nCatalogReleaseDescriptor>>;

    async fn list_active_official_messages(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ActiveOfficialCatalogMessage>>;

    async fn list_catalog_overrides(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::CatalogTranslation>>;

    async fn list_custom_catalog_translations(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::CatalogTranslation>>;

    async fn upsert_catalog_override(
        &self,
        input: &UpsertCatalogTranslationInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn delete_catalog_override(
        &self,
        input: &DeleteCatalogTranslationInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn upsert_custom_catalog_translation(
        &self,
        input: &UpsertCatalogTranslationInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn delete_custom_catalog_message(
        &self,
        input: &DeleteCustomCatalogMessageInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn mark_superseded_release_obsolete_against_active(
        &self,
        workspace_id: Uuid,
        superseded_release_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ObsoleteCatalogMessage>>;

    async fn list_obsolete_catalog_messages(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ObsoleteCatalogMessage>>;
}
