use std::collections::BTreeMap;

use domain::{CatalogDigest, CatalogLocale, CatalogModuleId, WorkspaceCatalogRevision};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{RuntimeCatalogProjection, RuntimeI18nCatalogRepository},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedCatalogModuleBundle {
    pub module: String,
    pub locale: String,
    pub messages: BTreeMap<String, String>,
}

impl ResolvedCatalogModuleBundle {
    pub fn canonical_body(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn digest(&self) -> anyhow::Result<CatalogDigest> {
        let hash = Sha256::digest(self.canonical_body()?);
        CatalogDigest::new(format!("sha256:{hash:x}")).map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCatalogManifestModule {
    pub bundle: ResolvedCatalogModuleBundle,
    pub digest: CatalogDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCatalogManifest {
    pub revision: WorkspaceCatalogRevision,
    pub modules: Vec<RuntimeCatalogManifestModule>,
}

pub struct RuntimeI18nCatalogService<R> {
    repository: R,
    root_workspace_id: Uuid,
}

impl<R> RuntimeI18nCatalogService<R>
where
    R: RuntimeI18nCatalogRepository,
{
    pub fn new(repository: R, root_workspace_id: Uuid) -> Self {
        Self {
            repository,
            root_workspace_id,
        }
    }

    pub async fn manifest(
        &self,
        workspace_id: Uuid,
        locale: &CatalogLocale,
    ) -> anyhow::Result<RuntimeCatalogManifest> {
        if workspace_id != self.root_workspace_id {
            return Err(ControlPlaneError::PermissionDenied("root_i18n_catalog_workspace").into());
        }
        let RuntimeCatalogProjection { revision, messages } = self
            .repository
            .project_runtime_catalog(workspace_id, locale)
            .await?;
        let mut modules = BTreeMap::<CatalogModuleId, BTreeMap<String, String>>::new();
        for message in messages {
            modules
                .entry(message.module)
                .or_default()
                .insert(message.msgid, message.value);
        }
        let modules = modules
            .into_iter()
            .map(|(module, messages)| {
                let bundle = ResolvedCatalogModuleBundle {
                    module: module.as_str().to_owned(),
                    locale: locale.as_str().to_owned(),
                    messages,
                };
                let digest = bundle.digest()?;
                Ok(RuntimeCatalogManifestModule { bundle, digest })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(RuntimeCatalogManifest { revision, modules })
    }

    pub async fn current_bundle(
        &self,
        workspace_id: Uuid,
        module: &CatalogModuleId,
        locale: &CatalogLocale,
    ) -> anyhow::Result<Option<RuntimeCatalogManifestModule>> {
        Ok(self
            .manifest(workspace_id, locale)
            .await?
            .modules
            .into_iter()
            .find(|candidate| candidate.bundle.module == module.as_str()))
    }
}
