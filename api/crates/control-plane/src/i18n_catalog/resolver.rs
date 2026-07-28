use crate::{
    errors::ControlPlaneError,
    ports::{CatalogResolutionCandidate, CatalogResolutionRepository},
};
use domain::{CatalogLocale, CatalogMessageIdentity};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogResolutionOrigin {
    RootOverride,
    ActiveOfficial,
    EnglishIdentity,
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
        if locale.is_source() {
            return Ok(ResolvedCatalogMessage {
                value: identity.msgid().to_owned(),
                origin: CatalogResolutionOrigin::EnglishIdentity,
            });
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
                origin: CatalogResolutionOrigin::RootOverride,
            });
        }
        if let Some(value) = active_official {
            return Ok(ResolvedCatalogMessage {
                value,
                origin: CatalogResolutionOrigin::ActiveOfficial,
            });
        }
        Ok(ResolvedCatalogMessage {
            value: identity.msgid().to_owned(),
            origin: CatalogResolutionOrigin::EnglishIdentity,
        })
    }
}
