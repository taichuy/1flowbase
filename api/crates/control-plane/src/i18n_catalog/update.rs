use std::sync::Arc;

use anyhow::{anyhow, Result};
use semver::Version;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{
        I18nCatalogRepository, OfficialI18nCatalogReleaseDescriptor, OfficialI18nCatalogSourcePort,
        StoredI18nCatalogReleaseDescriptor,
    },
};
use domain::{CatalogVersion, WorkspaceCatalogRevision, WorkspaceCatalogState};

#[derive(Debug, Clone, Copy)]
pub struct OfficialI18nCatalogUpdateCommand {
    pub workspace_id: Uuid,
    pub expected_revision: WorkspaceCatalogRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfficialI18nCatalogUpdateOutcome {
    Current {
        catalog_version: CatalogVersion,
    },
    Activated {
        catalog_version: CatalogVersion,
        state: WorkspaceCatalogState,
    },
}

pub struct OfficialI18nCatalogUpdateService<R> {
    repository: R,
    source: Arc<dyn OfficialI18nCatalogSourcePort>,
}

impl<R> OfficialI18nCatalogUpdateService<R>
where
    R: I18nCatalogRepository,
{
    pub fn new(repository: R, source: Arc<dyn OfficialI18nCatalogSourcePort>) -> Self {
        Self { repository, source }
    }

    pub async fn check_and_activate(
        &self,
        command: OfficialI18nCatalogUpdateCommand,
    ) -> Result<OfficialI18nCatalogUpdateOutcome> {
        let state = self
            .repository
            .get_workspace_catalog_state(command.workspace_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
        if state.revision() != command.expected_revision {
            return Err(ControlPlaneError::Conflict("i18n_catalog_revision").into());
        }
        let active = self.active_release_descriptor(&state).await?;

        // All mutable remote reads and fixed artifact verification finish before
        // staging opens its own short database transaction.
        let latest = self.source.check_latest_release().await?;
        if let Some(active) = active.as_ref() {
            match compare_releases(active, &latest)? {
                ReleaseDifference::Current => {
                    return Ok(OfficialI18nCatalogUpdateOutcome::Current {
                        catalog_version: active.catalog_version.clone(),
                    });
                }
                ReleaseDifference::ContentDrift => {
                    return Err(ControlPlaneError::Conflict(
                        "official_i18n_catalog_version_content_drift",
                    )
                    .into());
                }
                ReleaseDifference::Newer => {}
            }
        }

        let seed = self.source.fetch_verified_release(&latest).await?;
        if seed.catalog_version() != &latest.catalog_version
            || seed.semantic_sha256() != &latest.semantic_sha256
        {
            return Err(anyhow!(
                "verified official i18n catalog does not match the fixed release descriptor"
            ));
        }
        let release = seed.bind_to_workspace(command.workspace_id)?;
        self.repository.import_verified_release(&release).await?;
        let state = self
            .repository
            .activate_verified_release(
                command.workspace_id,
                release.id(),
                command.expected_revision,
            )
            .await?;
        Ok(OfficialI18nCatalogUpdateOutcome::Activated {
            catalog_version: latest.catalog_version,
            state,
        })
    }

    async fn active_release_descriptor(
        &self,
        state: &WorkspaceCatalogState,
    ) -> Result<Option<StoredI18nCatalogReleaseDescriptor>> {
        let Some(release_id) = state.active_release_id() else {
            return Ok(None);
        };
        self.repository
            .get_i18n_catalog_release_descriptor(state.workspace_id(), release_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("active_i18n_catalog_release"))
            .map(Some)
            .map_err(Into::into)
    }
}

enum ReleaseDifference {
    Current,
    ContentDrift,
    Newer,
}

fn compare_releases(
    active: &StoredI18nCatalogReleaseDescriptor,
    latest: &OfficialI18nCatalogReleaseDescriptor,
) -> Result<ReleaseDifference> {
    let active_version = Version::parse(active.catalog_version.as_str())
        .map_err(|_| ControlPlaneError::InvalidInput("active_i18n_catalog_version"))?;
    let latest_version = Version::parse(latest.catalog_version.as_str())
        .map_err(|_| ControlPlaneError::InvalidInput("official_i18n_catalog_version"))?;
    if latest_version == active_version {
        return Ok(if latest.semantic_sha256 == active.semantic_sha256 {
            ReleaseDifference::Current
        } else {
            ReleaseDifference::ContentDrift
        });
    }
    if latest_version < active_version {
        return Ok(ReleaseDifference::Current);
    }
    Ok(ReleaseDifference::Newer)
}
