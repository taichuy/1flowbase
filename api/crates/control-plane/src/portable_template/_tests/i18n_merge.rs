use super::*;
use crate::ports::*;
use async_trait::async_trait;
use domain::{ActorContext, WorkspaceCatalogRevision, WorkspaceCatalogState};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    values: BTreeMap<(String, String), String>,
    baselines: BTreeMap<TemplateResourceKey, TemplateResourceBaseline>,
    revision: i64,
    write_revisions: Vec<i64>,
    fail_key: Option<String>,
    fail_after_receipt: bool,
    change_revision_before_write: bool,
    events: Vec<&'static str>,
}
#[derive(Clone, Default)]
struct Repository(Arc<Mutex<State>>);
fn resource_key(key: &str) -> TemplateResourceKey {
    TemplateResourceKey {
        kind: "i18n_entry".into(),
        source_id: serde_json::to_string(&[key, "zh_Hans"]).unwrap(),
    }
}
fn fingerprint(value: &str) -> String {
    template_resource_fingerprint(&serde_json::json!({"translation": value})).unwrap()
}
fn entry(key: &str, translation: &str) -> PortableI18nEntry {
    PortableI18nEntry {
        key: key.into(),
        locale: "zh_Hans".into(),
        translation: translation.into(),
    }
}
fn tracked(key: &str, value: &str) -> TemplateResourceBaseline {
    let key = resource_key(key);
    TemplateResourceBaseline {
        target_id: key.source_id.clone(),
        key,
        generation: 1,
        applied_fingerprint: Some(fingerprint(value)),
        pending: None,
        committed_operation_id: None,
        committed_fingerprint: None,
    }
}
fn setup() -> (
    Repository,
    I18nCatalogManagementService<Repository>,
    TemplateBaselineScope,
    CatalogManagementAccess,
) {
    let repository = Repository::default();
    let workspace_id = Uuid::now_v7();
    let access = CatalogManagementAccess {
        actor: ActorContext::root(Uuid::now_v7(), workspace_id, "root"),
        current_workspace_id: workspace_id,
    };
    let scope = TemplateBaselineScope {
        workspace_id,
        template_id: "demo".into(),
    };
    (
        repository.clone(),
        I18nCatalogManagementService::new(repository, workspace_id),
        scope,
        access,
    )
}
#[async_trait]
impl I18nCatalogManagementRepository for Repository {
    async fn list_catalog_management_entries(
        &self,
        query: &CatalogManagementQuery,
    ) -> Result<CatalogManagementPage> {
        let state = self.0.lock().unwrap();
        let revision = WorkspaceCatalogRevision::new(state.revision)?;
        let entries = state
            .values
            .iter()
            .filter(|((key, locale), _)| {
                query.key.as_ref().is_none_or(|selected| selected == key)
                    && query
                        .locale
                        .as_ref()
                        .is_none_or(|selected| selected.as_str() == locale)
            })
            .map(|((key, locale), value)| CatalogManagementEntry {
                key: key.clone(),
                locale: CatalogLocale::new(locale).unwrap(),
                official_translation: None,
                override_translation: None,
                custom_translation: Some(value.clone()),
                effective_value: value.clone(),
                origin: CatalogManagementOrigin::Custom,
                missing: false,
                obsolete: false,
                revision,
            })
            .collect::<Vec<_>>();
        let total = entries.len() as u64;
        Ok(CatalogManagementPage {
            entries: entries
                .into_iter()
                .skip(query.offset as usize)
                .take(query.limit as usize)
                .collect(),
            total,
            revision,
        })
    }
    async fn upsert_template_catalog_translation(
        &self,
        input: &AuditedTemplateCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        let mut state = self.0.lock().unwrap();
        let value = &input.translation.value;
        if state.change_revision_before_write {
            state.change_revision_before_write = false;
            state.revision += 1; // A concurrent native editor committed after the read.
        }
        ensure!(
            state.revision == input.translation.expected_revision.value(),
            "stale revision"
        );
        ensure!(
            state.fail_key.as_deref() != Some(value.identity().key()),
            "owner failed"
        );
        let id = (
            value.identity().key().to_owned(),
            value.locale().as_str().to_owned(),
        );
        ensure!(
            state.values.get(&id).map(|value| fingerprint(value))
                == input.intent.expected_fingerprint,
            "stale fingerprint"
        );
        let baseline = state
            .baselines
            .get_mut(&input.key)
            .expect("prepare before owner");
        ensure!(
            baseline.pending.as_ref() == Some(&input.intent),
            "receipt mismatch"
        );
        baseline.committed_operation_id = Some(input.intent.operation_id);
        baseline.committed_fingerprint = Some(fingerprint(value.translation()));
        state.events.push("write");
        let revision = state.revision;
        state.write_revisions.push(revision);
        state.values.insert(id, value.translation().into());
        state.revision += 1;
        if state.fail_after_receipt {
            state.fail_after_receipt = false;
            anyhow::bail!("connection lost after commit");
        }
        Ok(WorkspaceCatalogState::restored(
            input.translation.workspace_id,
            None,
            WorkspaceCatalogRevision::new(state.revision)?,
        ))
    }
    async fn upsert_official_catalog_override(
        &self,
        _input: &AuditedCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("template must use atomic owner")
    }
    async fn upsert_custom_catalog_translation_audited(
        &self,
        _input: &AuditedCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("template must use atomic owner")
    }
    async fn restore_official_catalog_translation(
        &self,
        _input: &AuditedDeleteCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("template must use atomic owner")
    }
    async fn restore_all_official_catalog_overrides(
        &self,
        _input: &AuditedRestoreAllCatalogOverridesInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("template must use atomic owner")
    }
    async fn delete_custom_catalog_message_audited(
        &self,
        _input: &AuditedDeleteCustomCatalogMessageInput,
    ) -> Result<WorkspaceCatalogState> {
        panic!("template must use atomic owner")
    }
}
#[async_trait]
impl PortableTemplateBaselineRepository for Repository {
    async fn load_template_baselines(
        &self,
        _scope: &TemplateBaselineScope,
    ) -> Result<Vec<TemplateResourceBaseline>> {
        Ok(self.0.lock().unwrap().baselines.values().cloned().collect())
    }
    async fn prepare_template_write(
        &self,
        _scope: &TemplateBaselineScope,
        key: &TemplateResourceKey,
        generation: Option<i64>,
        intent: &TemplateWriteIntent,
    ) -> Result<bool> {
        let mut state = self.0.lock().unwrap();
        if let Some(existing) = state.baselines.get(key) {
            if Some(existing.generation) != generation || existing.pending.is_some() {
                return Ok(false);
            }
        } else if generation.is_some() || intent.expected_fingerprint.is_some() {
            return Ok(false);
        }
        let baseline =
            state
                .baselines
                .entry(key.clone())
                .or_insert_with(|| TemplateResourceBaseline {
                    key: key.clone(),
                    target_id: intent.target_id.clone(),
                    generation: 0,
                    applied_fingerprint: None,
                    pending: None,
                    committed_operation_id: None,
                    committed_fingerprint: None,
                });
        baseline.generation += 1;
        baseline.pending = Some(intent.clone());
        state.events.push("prepare");
        Ok(true)
    }
    async fn finalize_template_write(
        &self,
        _scope: &TemplateBaselineScope,
        key: &TemplateResourceKey,
        operation_id: Uuid,
    ) -> Result<bool> {
        let mut state = self.0.lock().unwrap();
        let baseline = state.baselines.get_mut(key).unwrap();
        if baseline.committed_operation_id != Some(operation_id) {
            return Ok(false);
        }
        baseline.applied_fingerprint = baseline.committed_fingerprint.take();
        baseline.pending = None;
        baseline.committed_operation_id = None;
        baseline.generation += 1;
        state.events.push("finalize");
        Ok(true)
    }
    async fn abandon_template_write(
        &self,
        _scope: &TemplateBaselineScope,
        _key: &TemplateResourceKey,
        _operation_id: Uuid,
    ) -> Result<bool> {
        panic!("unknown outcomes must not be abandoned")
    }
}

#[tokio::test]
async fn initializes_and_updates_with_fresh_revision_for_each_translation() {
    let (repository, service, scope, access) = setup();
    let first = install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "初始"), entry("Other", "其他")],
    )
    .await
    .unwrap();
    assert_eq!(first.created.len(), 2);
    assert!(first.failures.is_empty());
    assert_eq!(repository.0.lock().unwrap().write_revisions, vec![0, 1]);
    let next = install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "新版"), entry("Other", "其他")],
    )
    .await
    .unwrap();
    assert_eq!(next.updated.len(), 1);
    assert!(next.created.is_empty());
    assert!(next.failures.is_empty());
    assert_eq!(
        next.effects
            .iter()
            .map(|effect| effect.action.as_str())
            .collect::<Vec<_>>(),
        vec!["update", "unchanged"]
    );
    assert_eq!(
        repository.0.lock().unwrap().baselines[&resource_key("Hello")].applied_fingerprint,
        Some(fingerprint("新版"))
    );
}
#[tokio::test]
async fn preview_and_install_preserve_unknown_modified_deleted_and_pending_entries() {
    let (repository, service, scope, access) = setup();
    {
        let mut state = repository.0.lock().unwrap();
        state
            .values
            .insert(("Unknown".into(), "zh_Hans".into()), "existing".into());
        state
            .values
            .insert(("Modified".into(), "zh_Hans".into()), "user edit".into());
        state
            .baselines
            .insert(resource_key("Modified"), tracked("Modified", "original"));
        state
            .baselines
            .insert(resource_key("Deleted"), tracked("Deleted", "original"));
        let mut pending = tracked("Pending", "original");
        pending.pending = Some(TemplateWriteIntent {
            operation_id: Uuid::now_v7(),
            target_id: pending.target_id.clone(),
            expected_fingerprint: Some(fingerprint("original")),
            desired_fingerprint: fingerprint("next"),
        });
        state.baselines.insert(resource_key("Pending"), pending);
    }
    let entries = [
        entry("Unknown", "next"),
        entry("Modified", "next"),
        entry("Deleted", "next"),
        entry("Pending", "next"),
    ];
    let effects = preview(&service, &repository, &scope, &access, &entries)
        .await
        .unwrap();
    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.reason.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("unknown_baseline"),
            Some("user_modified"),
            Some("user_deleted"),
            Some("pending_write")
        ]
    );
    let before = repository.0.lock().unwrap().baselines.clone();
    let result = install(&service, &repository, &scope, &access, &entries)
        .await
        .unwrap();
    assert_eq!(result.skipped.len(), 4);
    assert!(result.failures.is_empty());
    assert_eq!(repository.0.lock().unwrap().baselines, before);
    assert!(repository.0.lock().unwrap().events.is_empty());
}
#[tokio::test]
async fn recovers_only_committed_receipts_before_applying_a_newer_version() {
    let (repository, service, scope, access) = setup();
    repository.0.lock().unwrap().fail_after_receipt = true;
    let failed = install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "committed")],
    )
    .await
    .unwrap();
    assert_eq!(failed.failures.len(), 1);
    let before = repository.0.lock().unwrap().baselines.clone();
    let effects = preview(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "newer")],
    )
    .await
    .unwrap();
    assert_eq!(effects[0].action, "update");
    assert_eq!(repository.0.lock().unwrap().baselines, before); // Preview never finalizes.
    let result = install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "newer")],
    )
    .await
    .unwrap();
    assert_eq!(result.updated.len(), 1);
    assert!(result.failures.is_empty());
    assert_eq!(
        repository.0.lock().unwrap().events,
        vec!["prepare", "write", "finalize", "prepare", "write", "finalize"]
    );
}
#[tokio::test]
async fn entry_failure_does_not_erase_other_successes_or_adopt_the_failed_write() {
    let (repository, service, scope, access) = setup();
    repository.0.lock().unwrap().fail_key = Some("Fail".into());
    let result = install(
        &service,
        &repository,
        &scope,
        &access,
        &[
            entry("First", "one"),
            entry("Fail", "bad"),
            entry("Last", "last"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(result.created.len(), 2);
    assert_eq!(result.failures.len(), 1);
    let state = repository.0.lock().unwrap();
    assert_eq!(
        state.baselines[&resource_key("First")].applied_fingerprint,
        Some(fingerprint("one"))
    );
    assert_eq!(
        state.baselines[&resource_key("Last")].applied_fingerprint,
        Some(fingerprint("last"))
    );
    assert!(state.baselines[&resource_key("Fail")].pending.is_some());
    assert_eq!(
        state.baselines[&resource_key("Fail")].applied_fingerprint,
        None
    );
}
#[tokio::test]
async fn malformed_selection_and_non_root_access_fail_before_writes() {
    let (repository, service, scope, mut access) = setup();
    assert!(install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "a"), entry("Hello", "b")]
    )
    .await
    .is_err());
    access.actor.is_root = false;
    assert!(install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Hello", "a")]
    )
    .await
    .is_err());
    assert!(repository.0.lock().unwrap().events.is_empty());
}

#[tokio::test]
async fn concurrent_revision_change_cannot_overwrite_and_next_entry_reads_fresh_revision() {
    let (repository, service, scope, access) = setup();
    repository.0.lock().unwrap().change_revision_before_write = true;
    let result = install(
        &service,
        &repository,
        &scope,
        &access,
        &[entry("Raced", "do not write"), entry("Next", "write next")],
    )
    .await
    .unwrap();
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.created.len(), 1);
    let state = repository.0.lock().unwrap();
    assert!(!state
        .values
        .contains_key(&("Raced".into(), "zh_Hans".into())));
    assert_eq!(
        state
            .values
            .get(&("Next".into(), "zh_Hans".into()))
            .map(String::as_str),
        Some("write next")
    );
    assert_eq!(state.write_revisions, vec![1]);
    assert!(state.baselines[&resource_key("Raced")].pending.is_some());
    assert_eq!(
        state.baselines[&resource_key("Raced")].applied_fingerprint,
        None
    );
}
