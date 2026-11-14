//! Template orchestration delegates translation authority and writes to the native owner.
use super::plugins::TemplateDependencies;
use control_plane::{
    errors::ControlPlaneError,
    i18n_catalog::management::{CatalogManagementAccess, I18nCatalogManagementService},
    portable_template::{self, PortableTemplatePackage, PortableTemplateSelection},
};

impl TemplateDependencies {
    pub(super) fn translation_access(
        &self,
        actor: &domain::ActorContext,
    ) -> CatalogManagementAccess {
        CatalogManagementAccess {
            actor: actor.clone(),
            current_workspace_id: actor.current_workspace_id,
        }
    }

    pub(super) fn translation_service(
        &self,
    ) -> I18nCatalogManagementService<storage_durable_postgres::MainDurableStore> {
        I18nCatalogManagementService::new(self.store.clone(), self.bootstrap_workspace_id)
    }

    pub(super) async fn translation_catalog(
        &self,
        actor: &domain::ActorContext,
    ) -> anyhow::Result<Vec<portable_template::PortableI18nCatalogItem>> {
        match portable_template::i18n::snapshot(
            &self.translation_service(),
            &self.translation_access(actor),
        )
        .await
        {
            Ok(snapshot) => Ok(portable_template::i18n::catalog(&snapshot)),
            Err(error)
                if matches!(
                    error.downcast_ref::<ControlPlaneError>(),
                    Some(ControlPlaneError::PermissionDenied(_))
                ) =>
            {
                Ok(Vec::new())
            }
            Err(error) => Err(error),
        }
    }

    pub(super) async fn export_template(
        &self,
        actor: &domain::ActorContext,
        selection: PortableTemplateSelection,
    ) -> anyhow::Result<PortableTemplatePackage> {
        let translations = if selection.i18n_keys.is_empty() {
            Vec::new()
        } else {
            let snapshot = portable_template::i18n::snapshot(
                &self.translation_service(),
                &self.translation_access(actor),
            )
            .await?;
            portable_template::i18n::export_selected(&snapshot, &selection.i18n_keys)?
        };
        portable_template::PortableTemplateService::new(self.store.for_actor(actor.clone()))
            .export_with_i18n(actor.user_id, selection, translations)
            .await
    }
}
