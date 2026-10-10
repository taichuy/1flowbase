//! Pure safe-merge planning. This module never turns observed target content into ownership.
use super::*;
use anyhow::{ensure, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct PlannedTemplateResource {
    pub desired: ProjectedTemplateResource,
    pub current_fingerprint: Option<String>,
    pub baseline_generation: Option<i64>,
    pub decision: TemplateMergeDecision,
}
impl PlannedTemplateResource {
    pub fn effect(&self) -> PortableTemplateEffect {
        PortableTemplateEffect {
            kind: self.desired.key.kind.clone(),
            source_id: self.desired.key.source_id.clone(),
            target_id: Some(self.desired.target_id.clone()),
            action: self.decision.action().into(),
            reason: self.decision.reason().map(str::to_owned),
        }
    }
    /// Allocate identities before planning new resources; never change target_id after prepare.
    pub fn write_intent(&self, operation_id: uuid::Uuid) -> Option<TemplateWriteIntent> {
        matches!(
            self.decision,
            TemplateMergeDecision::Initialize | TemplateMergeDecision::Update
        )
        .then(|| TemplateWriteIntent {
            operation_id,
            target_id: self.desired.target_id.clone(),
            expected_fingerprint: self.current_fingerprint.clone(),
            desired_fingerprint: self.desired.fingerprint.clone(),
        })
    }
}

pub fn decide_template_resource(
    current: Option<&str>,
    desired: &str,
    baseline: Option<&TemplateResourceBaseline>,
    historically_tracked: bool,
) -> TemplateMergeDecision {
    use TemplateMergeDecision::*;
    if let Some(baseline) = baseline {
        if let Some(pending) = &baseline.pending {
            return if baseline.committed_operation_id == Some(pending.operation_id)
                && baseline.committed_fingerprint.is_some()
            {
                RecoverCommitted
            } else {
                SkipPendingWrite
            };
        }
    }
    let applied = baseline.and_then(|b| b.applied_fingerprint.as_deref());
    match (current, applied) {
        (None, _) if historically_tracked || applied.is_some() => SkipUserDeleted,
        (None, None) => Initialize,
        (Some(_), None) => SkipUnknownBaseline,
        (Some(current), Some(applied)) if current != applied => SkipUserModified,
        (Some(current), Some(_)) if current == desired => Unchanged,
        (Some(_), Some(_)) => Update,
        (None, Some(_)) => SkipUserDeleted,
    }
}

/// The caller supplies historical identities even when a resource is currently absent.
/// Explicit legacy MCP reset may remove ONLY its unbaselined tracking flag before calling.
/// Target-only entries have no plan entry and are preserved by construction.
pub fn plan_template_resources(
    desired: Vec<ProjectedTemplateResource>,
    current: &[ProjectedTemplateResource],
    baselines: &[TemplateResourceBaseline],
    tracked: &std::collections::BTreeSet<TemplateResourceKey>,
) -> Result<Vec<PlannedTemplateResource>> {
    let mut target = BTreeMap::new();
    for item in current {
        ensure!(
            target
                .insert((&item.key.kind, &item.target_id), item)
                .is_none(),
            "portable_template_duplicate_target_resource"
        );
    }
    let mut history = BTreeMap::new();
    for baseline in baselines {
        ensure!(
            history.insert(&baseline.key, baseline).is_none(),
            "portable_template_duplicate_baseline"
        );
    }
    let mut source_keys = std::collections::BTreeSet::new();
    let mut target_keys = std::collections::BTreeSet::new();
    desired
        .into_iter()
        .map(|item| {
            ensure!(
                source_keys.insert(item.key.clone()),
                "portable_template_duplicate_source_resource"
            );
            ensure!(
                target_keys.insert((item.key.kind.clone(), item.target_id.clone())),
                "portable_template_ambiguous_target_identity"
            );
            let baseline = history.get(&item.key).copied();
            ensure!(
                baseline.is_none_or(|b| b.target_id == item.target_id),
                "portable_template_baseline_target_identity_conflict"
            );
            let current = target
                .get(&(&item.key.kind, &item.target_id))
                .map(|value| value.fingerprint.clone());
            let decision = decide_template_resource(
                current.as_deref(),
                &item.fingerprint,
                baseline,
                tracked.contains(&item.key),
            );
            Ok(PlannedTemplateResource {
                desired: item,
                current_fingerprint: current,
                baseline_generation: baseline.map(|b| b.generation),
                decision,
            })
        })
        .collect()
}

/// Recheck inside the owner transaction/lock, immediately before mutation.
/// Matching desired content is insufficient: only the recorded pre-image authorizes a write.
pub fn template_intent_matches_current(
    intent: &TemplateWriteIntent,
    current: Option<&str>,
) -> bool {
    intent.expected_fingerprint.as_deref() == current
}

#[cfg(test)]
#[path = "../_tests/merge.rs"]
mod tests;
