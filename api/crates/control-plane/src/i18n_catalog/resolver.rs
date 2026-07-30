use crate::{
    errors::ControlPlaneError,
    ports::{CatalogResolutionCandidate, CatalogResolutionRepository},
};
use domain::{CatalogLocale, CatalogMessageIdentity};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogResolutionOrigin {
    RequestedWorkspaceOverride,
    RequestedOfficial,
    EnglishWorkspaceOverride,
    EnglishOfficial,
    RawKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCatalogMessage {
    pub value: String,
    pub origin: CatalogResolutionOrigin,
}

pub struct CatalogResolver<R> {
    repository: R,
    root_workspace_id: Uuid,
}

impl<R> CatalogResolver<R>
where
    R: CatalogResolutionRepository,
{
    pub fn new(repository: R, root_workspace_id: Uuid) -> Self {
        Self {
            repository,
            root_workspace_id,
        }
    }

    pub async fn resolve(
        &self,
        workspace_id: Uuid,
        identity: &CatalogMessageIdentity,
        locale: &CatalogLocale,
    ) -> anyhow::Result<ResolvedCatalogMessage> {
        if workspace_id != self.root_workspace_id {
            return Err(ControlPlaneError::PermissionDenied("root_i18n_catalog_workspace").into());
        }
        let CatalogResolutionCandidate {
            root_override,
            active_official,
        } = self
            .repository
            .find_catalog_resolution_candidate(workspace_id, identity, locale)
            .await?;
        if let Some(value) = root_override {
            return Ok(ResolvedCatalogMessage {
                value,
                origin: CatalogResolutionOrigin::RequestedWorkspaceOverride,
            });
        }
        if let Some(value) = active_official {
            return Ok(ResolvedCatalogMessage {
                value,
                origin: CatalogResolutionOrigin::RequestedOfficial,
            });
        }

        if !locale.is_source() {
            let CatalogResolutionCandidate {
                root_override,
                active_official,
            } = self
                .repository
                .find_catalog_resolution_candidate(workspace_id, identity, &CatalogLocale::source())
                .await?;
            if let Some(value) = root_override {
                return Ok(ResolvedCatalogMessage {
                    value,
                    origin: CatalogResolutionOrigin::EnglishWorkspaceOverride,
                });
            }
            if let Some(value) = active_official {
                return Ok(ResolvedCatalogMessage {
                    value,
                    origin: CatalogResolutionOrigin::EnglishOfficial,
                });
            }
        }

        tracing::warn!(
            workspace_id = %workspace_id,
            key = identity.key(),
            requested_locale = locale.as_str(),
            fallback_locale = domain::I18N_CATALOG_SOURCE_LOCALE,
            "i18n catalog resolution fell back to the raw key"
        );
        Ok(ResolvedCatalogMessage {
            value: identity.key().to_owned(),
            origin: CatalogResolutionOrigin::RawKey,
        })
    }
}
