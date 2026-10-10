//! Per-translation merge orchestration. The native owner guards revision/content and
//! commits the translation, audit and receipt atomically. No observed value is adopted.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{ensure, Result};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogTranslation};
use uuid::Uuid;

use super::{i18n, *};
use crate::{
    i18n_catalog::management::{
        ApplyTemplateTranslationCommand, CatalogManagementAccess, I18nCatalogManagementService,
        ListCatalogEntriesCommand,
    },
    ports::{I18nCatalogManagementRepository, PortableTemplateBaselineRepository},
};

#[derive(Debug, Default)]
pub struct I18nMergeOutcome {
    pub effects: Vec<PortableTemplateEffect>,
    pub created: Vec<PortableTemplateCreatedResource>,
    pub updated: Vec<PortableTemplateCreatedResource>,
    pub skipped: Vec<PortableTemplateSkippedResource>,
    pub failures: Vec<String>,
}

fn projected(entry: &PortableI18nEntry) -> Result<ProjectedTemplateResource> {
    // Validate the complete selected input before any resource writes.
    CatalogMessageIdentity::new(&entry.key)?;
    CatalogLocale::new(&entry.locale)?;
    let source_id = serde_json::to_string(&[&entry.key, &entry.locale])?;
    let value = serde_json::json!({"translation": entry.translation});
    Ok(ProjectedTemplateResource {
        key: TemplateResourceKey {
            kind: "i18n_entry".into(),
            source_id: source_id.clone(),
        },
        target_id: source_id,
        fingerprint: template_resource_fingerprint(&value)?,
        value,
    })
}
fn desired(entries: &[PortableI18nEntry]) -> Result<Vec<ProjectedTemplateResource>> {
    let values = entries.iter().map(projected).collect::<Result<Vec<_>>>()?;
    let mut unique = BTreeSet::new();
    for value in &values {
        ensure!(
            unique.insert(value.key.clone()),
            "template_i18n_duplicate_selection"
        );
    }
    Ok(values)
}
fn current(
    entries: &[crate::ports::CatalogManagementEntry],
) -> Result<Vec<ProjectedTemplateResource>> {
    let mut values = BTreeMap::new();
    for entry in entries {
        if let Some(translation) = i18n::current_translation(entry) {
            let item = projected(&PortableI18nEntry {
                key: entry.key.clone(),
                locale: entry.locale.as_str().into(),
                translation: translation.into(),
            })?;
            if let Some(previous) = values.insert(item.key.clone(), item.clone()) {
                ensure!(
                    previous.fingerprint == item.fingerprint,
                    "template_i18n_ambiguous_current_translation"
                );
            }
        }
    }
    Ok(values.into_values().collect())
}
fn recoverable(baseline: &TemplateResourceBaseline) -> bool {
    baseline.pending.as_ref().is_some_and(|intent| {
        baseline.committed_operation_id == Some(intent.operation_id)
            && baseline.committed_fingerprint.is_some()
    })
}
fn scope_matches(scope: &TemplateBaselineScope, access: &CatalogManagementAccess) -> Result<()> {
    ensure!(
        scope.workspace_id == access.current_workspace_id,
        "template_i18n_scope_mismatch"
    );
    Ok(())
}

/// Read-only preview. A durable committed receipt is projected as finalized without
/// writing the journal; actual installation must finalize that receipt first.
pub async fn preview<R: I18nCatalogManagementRepository, B: PortableTemplateBaselineRepository>(
    service: &I18nCatalogManagementService<R>,
    repository: &B,
    scope: &TemplateBaselineScope,
    access: &CatalogManagementAccess,
    entries: &[PortableI18nEntry],
) -> Result<Vec<PortableTemplateEffect>> {
    scope_matches(scope, access)?;
    let desired = desired(entries)?;
    let snapshot = i18n::snapshot(service, access).await?;
    let mut baselines = repository.load_template_baselines(scope).await?;
    for baseline in &mut baselines {
        if recoverable(baseline) {
            baseline.applied_fingerprint = baseline.committed_fingerprint.take();
            baseline.pending = None;
            baseline.committed_operation_id = None;
        }
    }
    Ok(plan_template_resources(
        desired,
        &current(&snapshot.entries)?,
        &baselines,
        &BTreeSet::new(),
    )?
    .iter()
    .map(PlannedTemplateResource::effect)
    .collect())
}

/// Caller serializes template installation using its install guard. Resource editor
/// concurrency remains protected by the native owner's latest revision + fingerprint CAS.
/// Entry failures are collected and do not roll back earlier successful entries. Unknown
/// write outcomes retain pending intent; a later retry may recover only an atomic receipt.
pub async fn install<R: I18nCatalogManagementRepository, B: PortableTemplateBaselineRepository>(
    service: &I18nCatalogManagementService<R>,
    repository: &B,
    scope: &TemplateBaselineScope,
    access: &CatalogManagementAccess,
    entries: &[PortableI18nEntry],
) -> Result<I18nMergeOutcome> {
    scope_matches(scope, access)?;
    let desired = desired(entries)?;
    // Authorize through the native management owner even for an empty selection.
    service
        .list(ListCatalogEntriesCommand {
            access: access.clone(),
            key: None,
            locale: None,
            search: None,
            origin: None,
            offset: 0,
            limit: 1,
        })
        .await?;
    let mut baselines = repository.load_template_baselines(scope).await?;
    let mut outcome = I18nMergeOutcome::default();
    for (entry, desired) in entries.iter().zip(desired) {
        let mut phase = "read";
        let result: Result<()> = async {
            if let Some(baseline) = baselines
                .iter()
                .find(|baseline| baseline.key == desired.key)
            {
                if recoverable(baseline) {
                    phase = "recover";
                    let operation_id = baseline
                        .pending
                        .as_ref()
                        .expect("recoverable intent")
                        .operation_id;
                    ensure!(
                        repository
                            .finalize_template_write(scope, &desired.key, operation_id)
                            .await?,
                        "template_i18n_recovery_conflict"
                    );
                    baselines = repository.load_template_baselines(scope).await?;
                }
            }
            phase = "read";
            let page = service
                .list(ListCatalogEntriesCommand {
                    access: access.clone(),
                    key: Some(entry.key.clone()),
                    locale: Some(CatalogLocale::new(&entry.locale)?),
                    search: None,
                    origin: None,
                    offset: 0,
                    limit: 200,
                })
                .await?;
            ensure!(
                page.total == page.entries.len() as u64,
                "template_i18n_entry_page_incomplete"
            );
            let item = plan_template_resources(
                vec![desired.clone()],
                &current(&page.entries)?,
                &baselines,
                &BTreeSet::new(),
            )?
            .remove(0);
            let effect = item.effect();
            if let Some(reason) = item.decision.reason() {
                outcome.skipped.push(PortableTemplateSkippedResource {
                    kind: desired.key.kind.clone(),
                    source_id: desired.key.source_id.clone(),
                    target_id: Some(desired.target_id.clone()),
                    reason: reason.into(),
                });
                outcome.effects.push(effect);
                return Ok(());
            }
            let Some(intent) = item.write_intent(Uuid::now_v7()) else {
                outcome.effects.push(effect);
                return Ok(());
            };
            phase = "prepare";
            ensure!(
                repository
                    .prepare_template_write(scope, &desired.key, item.baseline_generation, &intent)
                    .await?,
                "template_i18n_prepare_conflict"
            );
            phase = "write";
            service
                .apply_template_translation(ApplyTemplateTranslationCommand {
                    access: access.clone(),
                    value: CatalogTranslation::new(
                        CatalogMessageIdentity::new(&entry.key)?,
                        CatalogLocale::new(&entry.locale)?,
                        &entry.translation,
                    )?,
                    expected_revision: page.revision,
                    scope: scope.clone(),
                    key: desired.key.clone(),
                    intent: intent.clone(),
                })
                .await?;
            let written = PortableTemplateCreatedResource {
                kind: desired.key.kind.clone(),
                source_id: desired.key.source_id.clone(),
                target_id: desired.target_id.clone(),
            };
            if item.decision == TemplateMergeDecision::Initialize {
                outcome.created.push(written);
            } else {
                outcome.updated.push(written);
            }
            outcome.effects.push(effect);
            phase = "finalize";
            ensure!(
                repository
                    .finalize_template_write(scope, &desired.key, intent.operation_id)
                    .await?,
                "template_i18n_finalize_conflict"
            );
            Ok(())
        }
        .await;
        if result.is_err() {
            let guidance = match phase {
                "write" => "The write could not be confirmed. Check the translation state before retrying.",
                "finalize" | "recover" => "The saved write receipt could not be finalized. Retry to reconcile its recorded outcome.",
                _ => "The translation could not be prepared from its current state. Refresh the preview before retrying.",
            };
            outcome.failures.push(format!(
                "Translation '{}' ({}): {}",
                entry.key, entry.locale, guidance
            ));
            // Do not claim rollback or abandon a journal intent on an unknown owner outcome.
        }
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "_tests/i18n_merge.rs"]
mod tests;
