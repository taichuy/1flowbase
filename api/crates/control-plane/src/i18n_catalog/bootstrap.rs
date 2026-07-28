use domain::{
    CatalogDigest, CatalogLocale, CatalogModuleId, CatalogSeedFile, CatalogVersion,
    I18nCatalogInvariantError, OfficialCatalogMessage, VerifiedCatalogRelease,
};
use time::OffsetDateTime;
use uuid::Uuid;

/// A fully validated official Seed whose workspace binding is deferred until the
/// bootstrap transaction has found or created the actual root workspace.
#[derive(Debug, Clone)]
pub struct VerifiedOfficialCatalogSeed {
    release_id: Uuid,
    catalog_version: CatalogVersion,
    locales: Vec<CatalogLocale>,
    modules: Vec<CatalogModuleId>,
    files: Vec<CatalogSeedFile>,
    generated_at: OffsetDateTime,
    semantic_sha256: CatalogDigest,
    messages: Vec<OfficialCatalogMessage>,
}

impl VerifiedOfficialCatalogSeed {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        release_id: Uuid,
        catalog_version: CatalogVersion,
        locales: Vec<CatalogLocale>,
        modules: Vec<CatalogModuleId>,
        files: Vec<CatalogSeedFile>,
        generated_at: OffsetDateTime,
        semantic_sha256: CatalogDigest,
        messages: Vec<OfficialCatalogMessage>,
    ) -> Result<Self, I18nCatalogInvariantError> {
        // Reuse the domain aggregate as the final invariant gate. The nil workspace
        // is never persisted; bind_to_workspace reconstructs the release atomically.
        VerifiedCatalogRelease::new(
            release_id,
            Uuid::nil(),
            catalog_version.clone(),
            locales.clone(),
            modules.clone(),
            files.clone(),
            generated_at,
            semantic_sha256.clone(),
            messages.clone(),
        )?;
        Ok(Self {
            release_id,
            catalog_version,
            locales,
            modules,
            files,
            generated_at,
            semantic_sha256,
            messages,
        })
    }

    pub fn bind_to_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<VerifiedCatalogRelease, I18nCatalogInvariantError> {
        VerifiedCatalogRelease::new(
            self.release_id,
            workspace_id,
            self.catalog_version.clone(),
            self.locales.clone(),
            self.modules.clone(),
            self.files.clone(),
            self.generated_at,
            self.semantic_sha256.clone(),
            self.messages.clone(),
        )
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn semantic_sha256(&self) -> &CatalogDigest {
        &self.semantic_sha256
    }
}
