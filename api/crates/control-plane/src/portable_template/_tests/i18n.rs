use super::*;
use crate::{i18n_catalog::management::ApplyTemplateTranslationCommand, ports::*};
use async_trait::async_trait;
use control_plane_contracts::portable_template::{
    TemplateBaselineScope, TemplateResourceKey, TemplateWriteIntent,
};
use domain::{
    ActorContext, CatalogLocale, CatalogMessageIdentity, CatalogTranslation, WorkspaceCatalogState,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone, Default)]
struct Repository {
    pages: Arc<Mutex<VecDeque<CatalogManagementPage>>>,
    writes: Arc<Mutex<Vec<AuditedTemplateCatalogTranslationInput>>>,
}
#[async_trait]
impl I18nCatalogManagementRepository for Repository {
    async fn list_catalog_management_entries(
        &self,
        _query: &CatalogManagementQuery,
    ) -> Result<CatalogManagementPage> {
        Ok(self
            .pages
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected catalog read"))
    }
    async fn upsert_template_catalog_translation(
        &self,
        input: &AuditedTemplateCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        self.writes.lock().unwrap().push(input.clone());
        if input.translation.expected_revision != WorkspaceCatalogRevision::new(7)? {
            return Err(ControlPlaneError::Conflict("i18n_catalog_revision").into());
        }
        Ok(WorkspaceCatalogState::restored(
            input.translation.workspace_id,
            None,
            WorkspaceCatalogRevision::new(8)?,
        ))
    }
    async fn upsert_official_catalog_override(
        &self,
        _input: &AuditedCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("ordinary editor write must not be used by templates")
    }
    async fn upsert_custom_catalog_translation_audited(
        &self,
        _input: &AuditedCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("ordinary editor write must not be used by templates")
    }
    async fn restore_official_catalog_translation(
        &self,
        _input: &AuditedDeleteCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("ordinary editor write must not be used by templates")
    }
    async fn restore_all_official_catalog_overrides(
        &self,
        _input: &AuditedRestoreAllCatalogOverridesInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("ordinary editor write must not be used by templates")
    }
    async fn delete_custom_catalog_message_audited(
        &self,
        _input: &AuditedDeleteCustomCatalogMessageInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("ordinary editor write must not be used by templates")
    }
}
fn access(workspace_id: Uuid) -> CatalogManagementAccess {
    CatalogManagementAccess {
        actor: ActorContext::root(Uuid::now_v7(), workspace_id, "root"),
        current_workspace_id: workspace_id,
    }
}
fn entry(key: &str, locale: &str, value: Option<&str>) -> CatalogManagementEntry {
    CatalogManagementEntry {
        key: key.to_owned(),
        locale: CatalogLocale::new(locale).unwrap(),
        official_translation: value.map(str::to_owned),
        override_translation: None,
        custom_translation: None,
        effective_value: value.unwrap_or(key).to_owned(),
        origin: if value.is_some() {
            CatalogManagementOrigin::Official
        } else {
            CatalogManagementOrigin::English
        },
        missing: value.is_none(),
        obsolete: false,
        revision: WorkspaceCatalogRevision::new(7).unwrap(),
    }
}
fn command(workspace_id: Uuid) -> ApplyTemplateTranslationCommand {
    let source_id = serde_json::to_string(&["Hello", "zh_Hans"]).unwrap();
    ApplyTemplateTranslationCommand {
        access: access(workspace_id),
        value: CatalogTranslation::new(
            CatalogMessageIdentity::new("Hello").unwrap(),
            CatalogLocale::new("zh_Hans").unwrap(),
            "你好",
        )
        .unwrap(),
        expected_revision: WorkspaceCatalogRevision::new(7).unwrap(),
        scope: TemplateBaselineScope {
            workspace_id,
            template_id: "demo".into(),
        },
        key: TemplateResourceKey {
            kind: "i18n_entry".into(),
            source_id: source_id.clone(),
        },
        intent: TemplateWriteIntent {
            operation_id: Uuid::now_v7(),
            target_id: source_id,
            expected_fingerprint: None,
            desired_fingerprint: crate::portable_template::template_resource_fingerprint(
                &serde_json::json!({"translation":"你好"}),
            )
            .unwrap(),
        },
    }
}

#[test]
fn explicit_keys_export_all_real_locales_and_never_fallback_values() {
    let mut overridden = entry("Hello", "zh_Hans", Some("你好"));
    overridden.override_translation = Some("您好".into());
    let snapshot = I18nTemplateSnapshot {
        revision: WorkspaceCatalogRevision::new(7).unwrap(),
        entries: vec![
            entry("Hello", "en_US", Some("Hello")),
            overridden,
            entry("Missing", "zh_Hans", None),
            entry("Other", "en_US", Some("Other")),
        ],
    };
    let values = export_selected(&snapshot, &["Hello".into()]).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[1].translation, "您好");
    assert_eq!(
        catalog(&snapshot)
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        ["Hello", "Other"]
    );
    assert!(export_selected(&snapshot, &["Missing".into()]).is_err());
    assert!(export_selected(&snapshot, &[]).unwrap().is_empty());
}

#[tokio::test]
async fn export_rejects_a_revision_change_between_pages() {
    let repository = Repository::default();
    repository.pages.lock().unwrap().extend([
        CatalogManagementPage {
            entries: vec![entry("Hello", "en_US", Some("Hello"))],
            total: 2,
            revision: WorkspaceCatalogRevision::new(7).unwrap(),
        },
        CatalogManagementPage {
            entries: vec![],
            total: 0,
            revision: WorkspaceCatalogRevision::new(8).unwrap(),
        },
    ]);
    let workspace_id = Uuid::now_v7();
    let service = I18nCatalogManagementService::new(repository, workspace_id);
    let error = snapshot(&service, &access(workspace_id))
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("template_i18n_catalog_revision"));
}

#[tokio::test]
async fn native_owner_rejects_non_root_or_wrong_workspace_before_repository_access() {
    let repository = Repository::default();
    let workspace_id = Uuid::now_v7();
    let service = I18nCatalogManagementService::new(repository.clone(), workspace_id);
    let mut denied = access(workspace_id);
    denied.actor.is_root = false;
    assert!(snapshot(&service, &denied).await.is_err());
    assert!(snapshot(&service, &access(Uuid::now_v7())).await.is_err());
    let mut denied_write = command(workspace_id);
    denied_write.access.actor.is_root = false;
    assert!(service
        .apply_template_translation(denied_write)
        .await
        .is_err());
    let mut mismatched = command(workspace_id);
    mismatched.key.source_id = "wrong".into();
    assert!(service
        .apply_template_translation(mismatched)
        .await
        .is_err());
    assert!(repository.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn native_owner_passes_revision_audit_and_journal_without_using_regular_writes() {
    let repository = Repository::default();
    let workspace_id = Uuid::now_v7();
    let service = I18nCatalogManagementService::new(repository.clone(), workspace_id);
    let command = command(workspace_id);
    let actor_id = command.access.actor.user_id;
    let operation_id = command.intent.operation_id;
    service
        .apply_template_translation(command.clone())
        .await
        .unwrap();
    let writes = repository.writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0].translation.expected_revision,
        WorkspaceCatalogRevision::new(7).unwrap()
    );
    assert_eq!(writes[0].translation.audit.workspace_id, Some(workspace_id));
    assert_eq!(writes[0].translation.audit.actor_user_id, Some(actor_id));
    assert_eq!(writes[0].intent.operation_id, operation_id);
    drop(writes);
    let mut stale = command;
    stale.expected_revision = WorkspaceCatalogRevision::new(6).unwrap();
    assert!(service.apply_template_translation(stale).await.is_err());
}
