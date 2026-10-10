//! Template-owned MCP resources merge through the native bundle owner. The locked
//! owner transaction also contains the write receipts; observation never adopts ownership.
use super::*;
use crate::{
    mcp_bundle::{ExportMcpBundleCommand, ImportMcpBundleCommand, PreviewMcpBundleCommand},
    mcp_management::McpManagementService,
    ports::{
        McpManagementRepository, PortableTemplateBaselineRepository,
        PortableTemplateTransactionRepository,
    },
};
use anyhow::{ensure, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct McpMergePreview {
    pub effects: Vec<PortableTemplateEffect>,
    pub conflicts: Vec<String>,
    pub shared_tool_impacts: Vec<domain::McpBundleSharedToolImpact>,
}
#[derive(Debug, Default)]
pub struct McpMergeOutcome {
    pub effects: Vec<PortableTemplateEffect>,
    pub created: Vec<PortableTemplateCreatedResource>,
    pub updated: Vec<PortableTemplateCreatedResource>,
    pub skipped: Vec<PortableTemplateSkippedResource>,
    pub failures: Vec<String>,
}
fn key(kind: &str, id: impl Into<String>) -> TemplateResourceKey {
    TemplateResourceKey {
        kind: kind.into(),
        source_id: id.into(),
    }
}
fn composite(parts: &[&str]) -> String {
    serde_json::to_string(parts).expect("string tuple")
}
fn project(bundle: &domain::McpBundlePackage) -> Result<Vec<ProjectedTemplateResource>> {
    project_template_resources(
        &PortableTemplatePackage {
            release: None,
            schema_version: PORTABLE_TEMPLATE_SCHEMA_VERSION.into(),
            pages: vec![],
            applications: vec![],
            data_models: vec![],
            plugins: vec![],
            i18n_entries: vec![],
            mcp_bundle: Some(bundle.clone()),
        },
        &BTreeMap::new(),
    )
}
fn recovery_operation(b: &TemplateResourceBaseline) -> Option<Uuid> {
    let intent = b.pending.as_ref()?;
    (b.committed_operation_id == Some(intent.operation_id) && b.committed_fingerprint.is_some())
        .then_some(intent.operation_id)
}
async fn snapshot<R: McpManagementRepository>(
    service: &McpManagementService<R>,
    actor: Uuid,
    package: &domain::McpBundlePackage,
    version: &str,
) -> Result<domain::McpBundlePackage> {
    service
        .export_bundle(ExportMcpBundleCommand {
            actor_user_id: actor,
            organization: package.manifest.organization.clone(),
            bundle_id: package.manifest.bundle_id.clone(),
            bundle_version: package.manifest.bundle_version.clone(),
            locale: "en_US".into(),
            current_system_version: version.into(),
        })
        .await
}
mod planning;
use planning::plan;

async fn validate<R: McpManagementRepository>(
    service: &McpManagementService<R>,
    actor: Uuid,
    package: domain::McpBundlePackage,
    catalog: &[domain::McpInterfaceCatalogEntry],
    version: &str,
) -> Result<(Vec<String>, Vec<domain::McpBundleSharedToolImpact>)> {
    let report = service
        .preview_bundle(PreviewMcpBundleCommand {
            actor_user_id: actor,
            package,
            interface_catalog: catalog.to_vec(),
            current_system_version: version.into(),
        })
        .await?;
    let conflicts = report
        .tools
        .iter()
        .chain(&report.instances)
        .chain(&report.connections)
        .filter(|r| {
            matches!(
                r.effect,
                domain::McpBundleItemEffect::Conflict | domain::McpBundleItemEffect::Failed
            )
        })
        .map(|r| format!("mcp:{}:{}", r.id, r.reason.as_deref().unwrap_or(&r.result)))
        .collect();
    Ok((conflicts, report.shared_tool_impacts))
}
pub async fn preview<R: McpManagementRepository + PortableTemplateBaselineRepository + Clone>(
    repository: &R,
    scope: &TemplateBaselineScope,
    actor_user_id: Uuid,
    package: &domain::McpBundlePackage,
    interface_catalog: &[domain::McpInterfaceCatalogEntry],
    current_system_version: &str,
) -> Result<McpMergePreview> {
    let service = McpManagementService::new(repository.clone());
    ensure!(
        service
            .authorize_manage(actor_user_id)
            .await?
            .current_workspace_id
            == scope.workspace_id,
        "template_mcp_scope_mismatch"
    );
    let current = snapshot(&service, actor_user_id, package, current_system_version).await?;
    let mut baselines = repository.load_template_baselines(scope).await?;
    for b in &mut baselines {
        if recovery_operation(b).is_some() {
            b.applied_fingerprint = b.committed_fingerprint.take();
            b.pending = None;
            b.committed_operation_id = None;
        }
    }
    let planned = plan(scope, package, &current, &baselines)?;
    let plan = planned.resources;
    let effective = planned.effective;
    let (conflicts, shared_tool_impacts) = validate(
        &service,
        actor_user_id,
        effective,
        interface_catalog,
        current_system_version,
    )
    .await?;
    Ok(McpMergePreview {
        effects: plan.iter().map(PlannedTemplateResource::effect).collect(),
        conflicts,
        shared_tool_impacts,
    })
}
pub async fn install<
    R: McpManagementRepository
        + PortableTemplateBaselineRepository
        + PortableTemplateTransactionRepository
        + Clone,
>(
    repository: &R,
    scope: &TemplateBaselineScope,
    actor_user_id: Uuid,
    package: &domain::McpBundlePackage,
    interface_catalog: &[domain::McpInterfaceCatalogEntry],
    current_system_version: &str,
) -> Result<McpMergeOutcome> {
    let mut transaction = repository.begin_portable_template_transaction().await?;
    let repo = &transaction.repository;
    let mut outcome = McpMergeOutcome::default();
    let result: Result<()> = async {
        let service = McpManagementService::new(repo.clone());
        ensure!(
            service
                .authorize_manage(actor_user_id)
                .await?
                .current_workspace_id
                == scope.workspace_id,
            "template_mcp_scope_mismatch"
        );
        let current = snapshot(&service, actor_user_id, package, current_system_version).await?;
        let baselines = repo.load_template_baselines(scope).await?;
        for b in &baselines {
            if !b.key.kind.starts_with("mcp_") {
                continue;
            }
            if let Some(operation_id) = recovery_operation(b) {
                ensure!(
                    repo.finalize_template_write(scope, &b.key, operation_id)
                        .await?,
                    "template_mcp_recovery_conflict"
                );
            }
        }
        let baselines = repo.load_template_baselines(scope).await?;
        let planned = plan(scope, package, &current, &baselines)?;
        let plan = planned.resources;
        let effective = planned.effective;
        outcome.effects = plan.iter().map(PlannedTemplateResource::effect).collect();
        for p in &plan {
            if let Some(reason) = p.decision.reason() {
                outcome.skipped.push(PortableTemplateSkippedResource {
                    kind: p.desired.key.kind.clone(),
                    source_id: p.desired.key.source_id.clone(),
                    target_id: Some(p.desired.target_id.clone()),
                    reason: reason.into(),
                });
            }
        }
        let (conflicts, _) = validate(
            &service,
            actor_user_id,
            effective.clone(),
            interface_catalog,
            current_system_version,
        )
        .await?;
        ensure!(conflicts.is_empty(), "{}", conflicts.join("; "));
        let mut writes = vec![];
        for p in &plan {
            if let Some(intent) = p.write_intent(Uuid::now_v7()) {
                writes.push((p, intent));
            }
        }
        if !writes.is_empty() {
            // Materialize tool copies before redirecting guarded bindings. Both owner
            // calls, identity CAS and receipts remain inside the same locked transaction.
            let mut tools = effective.clone();
            tools.instances.clear();
            if !tools.tools.is_empty() || !tools.connections.is_empty() {
                service
                    .import_bundle(ImportMcpBundleCommand {
                        actor_user_id,
                        package: tools,
                        interface_catalog: interface_catalog.to_vec(),
                        current_system_version: current_system_version.into(),
                    })
                    .await?;
            }
            for (p, intent) in &writes {
                if let Some(old_target) = planned.retargets.get(&p.desired.key) {
                    ensure!(
                        repo.retarget_template_mcp_binding(
                            scope,
                            &control_plane_contracts::ports::TemplateMcpBindingRetarget {
                                key: p.desired.key.clone(),
                                expected_target_id: old_target.clone(),
                                expected_generation: p
                                    .baseline_generation
                                    .context("template_mcp_retarget_baseline_required")?,
                                intent: intent.clone(),
                                actor_user_id,
                            }
                        )
                        .await?,
                        "template_mcp_retarget_conflict"
                    );
                }
            }
            let mut instances = effective;
            instances.tools.clear();
            instances.connections.clear();
            if !instances.instances.is_empty() {
                service
                    .import_bundle(ImportMcpBundleCommand {
                        actor_user_id,
                        package: instances,
                        interface_catalog: interface_catalog.to_vec(),
                        current_system_version: current_system_version.into(),
                    })
                    .await?;
            }
            let actual = project(
                &snapshot(&service, actor_user_id, package, current_system_version).await?,
            )?;
            for (p, intent) in writes {
                let actual = actual
                    .iter()
                    .find(|a| {
                        a.key.kind == p.desired.key.kind && a.target_id == p.desired.target_id
                    })
                    .ok_or_else(|| anyhow::anyhow!("template_mcp_postimage_missing"))?;
                // The native owner may normalize an unsupported tool to the exact
                // existing projection. No applied write means no baseline advance.
                let retargeted = planned.retargets.contains_key(&p.desired.key);
                if !retargeted
                    && p.current_fingerprint.as_deref() == Some(actual.fingerprint.as_str())
                {
                    continue;
                }
                if !retargeted {
                    ensure!(
                        repo.prepare_template_write(
                            scope,
                            &p.desired.key,
                            p.baseline_generation,
                            &intent
                        )
                        .await?,
                        "template_mcp_prepare_conflict"
                    );
                }
                ensure!(
                    repo.acknowledge_portable_template_write(
                        scope,
                        &p.desired.key,
                        &intent,
                        &actual.fingerprint
                    )
                    .await?,
                    "template_mcp_receipt_conflict"
                );
                ensure!(
                    repo.finalize_template_write(scope, &p.desired.key, intent.operation_id)
                        .await?,
                    "template_mcp_finalize_conflict"
                );
                let resource = PortableTemplateCreatedResource {
                    kind: p.desired.key.kind.clone(),
                    source_id: p.desired.key.source_id.clone(),
                    target_id: p.desired.target_id.clone(),
                };
                if p.decision == TemplateMergeDecision::Initialize {
                    outcome.created.push(resource);
                } else {
                    outcome.updated.push(resource);
                }
            }
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => transaction.guard.commit().await?,
        Err(error) => {
            transaction.guard.rollback().await?;
            outcome.created.clear();
            outcome.updated.clear();
            outcome
                .failures
                .push(format!("template_mcp_rollback:{error}"));
        }
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "_tests/mcp_merge.rs"]
mod tests;
