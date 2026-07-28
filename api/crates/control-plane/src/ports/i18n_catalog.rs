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
    pub source_locale: domain::CatalogLocale,
    pub locales: Vec<domain::CatalogLocale>,
    pub modules: Vec<domain::CatalogModuleId>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogManagementOrigin {
    Official,
    OfficialOverride,
    Custom,
    English,
}

#[derive(Debug, Clone)]
pub struct CatalogManagementQuery {
    pub workspace_id: Uuid,
    pub module: Option<domain::CatalogModuleId>,
    pub msgid: Option<String>,
    pub locale: Option<domain::CatalogLocale>,
    pub search: Option<String>,
    pub origin: Option<CatalogManagementOrigin>,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogManagementEntry {
    pub module: domain::CatalogModuleId,
    pub msgid: String,
    pub locale: domain::CatalogLocale,
    pub official_translation: Option<String>,
    pub override_translation: Option<String>,
    pub custom_translation: Option<String>,
    pub effective_value: String,
    pub origin: CatalogManagementOrigin,
    pub missing: bool,
    pub obsolete: bool,
    pub revision: domain::WorkspaceCatalogRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogManagementPage {
    pub entries: Vec<CatalogManagementEntry>,
    pub total: u64,
    pub revision: domain::WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct AuditedCatalogTranslationInput {
    pub workspace_id: Uuid,
    pub value: domain::CatalogTranslation,
    pub expected_revision: domain::WorkspaceCatalogRevision,
    pub audit: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct AuditedDeleteCatalogTranslationInput {
    pub workspace_id: Uuid,
    pub identity: domain::CatalogMessageIdentity,
    pub locale: domain::CatalogLocale,
    pub expected_revision: domain::WorkspaceCatalogRevision,
    pub audit: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct AuditedRestoreAllCatalogOverridesInput {
    pub workspace_id: Uuid,
    pub expected_revision: domain::WorkspaceCatalogRevision,
    pub audit: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct AuditedDeleteCustomCatalogMessageInput {
    pub workspace_id: Uuid,
    pub identity: domain::CatalogMessageIdentity,
    pub expected_revision: domain::WorkspaceCatalogRevision,
    pub audit: domain::AuditLogRecord,
}

#[async_trait]
pub trait I18nCatalogManagementRepository: Send + Sync {
    async fn list_catalog_management_entries(
        &self,
        query: &CatalogManagementQuery,
    ) -> anyhow::Result<CatalogManagementPage>;

    async fn upsert_official_catalog_override(
        &self,
        input: &AuditedCatalogTranslationInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn upsert_custom_catalog_translation_audited(
        &self,
        input: &AuditedCatalogTranslationInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn restore_official_catalog_translation(
        &self,
        input: &AuditedDeleteCatalogTranslationInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn restore_all_official_catalog_overrides(
        &self,
        input: &AuditedRestoreAllCatalogOverridesInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;

    async fn delete_custom_catalog_message_audited(
        &self,
        input: &AuditedDeleteCustomCatalogMessageInput,
    ) -> anyhow::Result<domain::WorkspaceCatalogState>;
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
