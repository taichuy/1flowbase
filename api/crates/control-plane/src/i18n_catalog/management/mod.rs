use anyhow::Result;
use domain::{
    ActorContext, CatalogLocale, CatalogMessageIdentity, CatalogTranslation,
    WorkspaceCatalogRevision, WorkspaceCatalogState,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        AuditedCatalogTranslationInput, AuditedDeleteCatalogTranslationInput,
        AuditedDeleteCustomCatalogMessageInput, AuditedRestoreAllCatalogOverridesInput,
        CatalogManagementPage, CatalogManagementQuery, I18nCatalogManagementRepository,
    },
};

const AUDIT_TARGET: &str = "i18n_catalog";

#[derive(Debug, Clone)]
pub struct CatalogManagementAccess {
    pub actor: ActorContext,
    pub current_workspace_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ListCatalogEntriesCommand {
    pub access: CatalogManagementAccess,
    pub key: Option<String>,
    pub locale: Option<CatalogLocale>,
    pub search: Option<String>,
    pub origin: Option<crate::ports::CatalogManagementOrigin>,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone)]
pub struct GetCatalogEntryCommand {
    pub access: CatalogManagementAccess,
    pub identity: CatalogMessageIdentity,
    pub locale: CatalogLocale,
}

#[derive(Debug, Clone)]
pub struct UpsertOfficialOverrideCommand {
    pub access: CatalogManagementAccess,
    pub value: CatalogTranslation,
    pub expected_revision: WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct UpsertCustomTranslationCommand {
    pub access: CatalogManagementAccess,
    pub value: CatalogTranslation,
    pub expected_revision: WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct RestoreOfficialTranslationCommand {
    pub access: CatalogManagementAccess,
    pub identity: CatalogMessageIdentity,
    pub locale: CatalogLocale,
    pub expected_revision: WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct RestoreAllOfficialOverridesCommand {
    pub access: CatalogManagementAccess,
    pub expected_revision: WorkspaceCatalogRevision,
}

#[derive(Debug, Clone)]
pub struct DeleteCustomMessageCommand {
    pub access: CatalogManagementAccess,
    pub identity: CatalogMessageIdentity,
    pub expected_revision: WorkspaceCatalogRevision,
}

pub struct I18nCatalogManagementService<R> {
    repository: R,
    bootstrap_workspace_id: Uuid,
}

impl<R> I18nCatalogManagementService<R>
where
    R: I18nCatalogManagementRepository,
{
    pub fn new(repository: R, bootstrap_workspace_id: Uuid) -> Self {
        Self {
            repository,
            bootstrap_workspace_id,
        }
    }

    pub async fn list(&self, command: ListCatalogEntriesCommand) -> Result<CatalogManagementPage> {
        self.authorize(&command.access)?;
        if command.limit == 0 || command.limit > 200 {
            return Err(ControlPlaneError::InvalidInput("i18n_catalog_page_limit").into());
        }
        self.repository
            .list_catalog_management_entries(&CatalogManagementQuery {
                workspace_id: self.bootstrap_workspace_id,
                key: command.key,
                locale: command.locale,
                search: command.search,
                origin: command.origin,
                offset: command.offset,
                limit: command.limit,
            })
            .await
    }

    pub async fn detail(
        &self,
        command: GetCatalogEntryCommand,
    ) -> Result<crate::ports::CatalogManagementEntry> {
        self.authorize(&command.access)?;
        let mut page = self
            .repository
            .list_catalog_management_entries(&CatalogManagementQuery {
                workspace_id: self.bootstrap_workspace_id,
                key: Some(command.identity.key().to_owned()),
                locale: Some(command.locale),
                search: None,
                origin: None,
                offset: 0,
                limit: 1,
            })
            .await?;
        page.entries
            .pop()
            .ok_or_else(|| ControlPlaneError::NotFound("i18n_catalog_management_entry").into())
    }

    pub async fn upsert_official_override(
        &self,
        command: UpsertOfficialOverrideCommand,
    ) -> Result<WorkspaceCatalogState> {
        self.authorize(&command.access)?;
        let identity = command.value.identity();
        let audit = self.audit(
            &command.access.actor,
            "i18n_catalog.official_override.upserted",
            json!({"key": identity.key(), "locale": command.value.locale().as_str()}),
        );
        self.repository
            .upsert_official_catalog_override(&AuditedCatalogTranslationInput {
                workspace_id: self.bootstrap_workspace_id,
                value: command.value,
                expected_revision: command.expected_revision,
                audit,
            })
            .await
    }

    pub async fn upsert_custom_translation(
        &self,
        command: UpsertCustomTranslationCommand,
    ) -> Result<WorkspaceCatalogState> {
        self.authorize(&command.access)?;
        let identity = command.value.identity();
        let audit = self.audit(
            &command.access.actor,
            "i18n_catalog.custom_translation.upserted",
            json!({"key": identity.key(), "locale": command.value.locale().as_str()}),
        );
        self.repository
            .upsert_custom_catalog_translation_audited(&AuditedCatalogTranslationInput {
                workspace_id: self.bootstrap_workspace_id,
                value: command.value,
                expected_revision: command.expected_revision,
                audit,
            })
            .await
    }

    pub async fn restore_official_translation(
        &self,
        command: RestoreOfficialTranslationCommand,
    ) -> Result<WorkspaceCatalogState> {
        self.authorize(&command.access)?;
        let audit = self.audit(
            &command.access.actor,
            "i18n_catalog.official_override.restored",
            json!({"key": command.identity.key(), "locale": command.locale.as_str()}),
        );
        self.repository
            .restore_official_catalog_translation(&AuditedDeleteCatalogTranslationInput {
                workspace_id: self.bootstrap_workspace_id,
                identity: command.identity,
                locale: command.locale,
                expected_revision: command.expected_revision,
                audit,
            })
            .await
    }

    pub async fn restore_all_official_overrides(
        &self,
        command: RestoreAllOfficialOverridesCommand,
    ) -> Result<WorkspaceCatalogState> {
        self.authorize(&command.access)?;
        let audit = self.audit(
            &command.access.actor,
            "i18n_catalog.official_overrides.restored_all",
            json!({}),
        );
        self.repository
            .restore_all_official_catalog_overrides(&AuditedRestoreAllCatalogOverridesInput {
                workspace_id: self.bootstrap_workspace_id,
                expected_revision: command.expected_revision,
                audit,
            })
            .await
    }

    pub async fn delete_custom_message(
        &self,
        command: DeleteCustomMessageCommand,
    ) -> Result<WorkspaceCatalogState> {
        self.authorize(&command.access)?;
        let audit = self.audit(
            &command.access.actor,
            "i18n_catalog.custom_message.deleted",
            json!({"key": command.identity.key()}),
        );
        self.repository
            .delete_custom_catalog_message_audited(&AuditedDeleteCustomCatalogMessageInput {
                workspace_id: self.bootstrap_workspace_id,
                identity: command.identity,
                expected_revision: command.expected_revision,
                audit,
            })
            .await
    }

    fn authorize(&self, access: &CatalogManagementAccess) -> Result<()> {
        if !access.actor.is_root {
            return Err(ControlPlaneError::PermissionDenied("i18n_catalog_root_required").into());
        }
        if access.current_workspace_id != access.actor.current_workspace_id
            || access.current_workspace_id != self.bootstrap_workspace_id
        {
            return Err(ControlPlaneError::PermissionDenied(
                "i18n_catalog_bootstrap_workspace_required",
            )
            .into());
        }
        Ok(())
    }

    fn audit(
        &self,
        actor: &ActorContext,
        event_code: &str,
        payload: serde_json::Value,
    ) -> domain::AuditLogRecord {
        audit_log(
            Some(self.bootstrap_workspace_id),
            Some(actor.user_id),
            AUDIT_TARGET,
            None,
            event_code,
            payload,
        )
    }
}
