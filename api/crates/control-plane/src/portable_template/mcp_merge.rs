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
use anyhow::{ensure, Result};
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
fn recoverable(b: &TemplateResourceBaseline) -> bool {
    b.pending.as_ref().is_some_and(|i| {
        b.committed_operation_id == Some(i.operation_id) && b.committed_fingerprint.is_some()
    })
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
fn plan(
    package: &domain::McpBundlePackage,
    current: &domain::McpBundlePackage,
    baselines: &[TemplateResourceBaseline],
) -> Result<(Vec<PlannedTemplateResource>, domain::McpBundlePackage)> {
    let mut plan = plan_template_resources(
        project(package)?,
        &project(current)?,
        baselines,
        &BTreeSet::new(),
    )?;
    // Missing baseline-owned parents remain deleted. An absent legacy, unbaselined
    // instance (including 1flowbase) can be explicitly reset by deleting it.
    let present_instances: BTreeSet<_> = current
        .instances
        .iter()
        .map(|i| i.instance_id.clone())
        .collect();
    let usable_instances: BTreeSet<_> = plan
        .iter()
        .filter(|p| {
            p.desired.key.kind == "mcp_instance"
                && (present_instances.contains(&p.desired.target_id)
                    || p.decision == TemplateMergeDecision::Initialize)
        })
        .map(|p| p.desired.target_id.clone())
        .collect();
    let usable_groups: BTreeSet<_> = plan
        .iter()
        .filter(|p| {
            p.desired.key.kind == "mcp_group"
                && (p.current_fingerprint.is_some()
                    || p.decision == TemplateMergeDecision::Initialize)
        })
        .map(|p| p.desired.key.source_id.clone())
        .chain(current.instances.iter().flat_map(|i| {
            i.groups
                .iter()
                .map(move |g| composite(&[&i.instance_id, &g.path]))
        }))
        .collect();
    let usable_connections: BTreeSet<_> = plan
        .iter()
        .filter(|p| {
            p.desired.key.kind == "mcp_connection"
                && (p.write_intent(Uuid::nil()).is_some()
                    || p.current_fingerprint.as_deref() == Some(p.desired.fingerprint.as_str()))
        })
        .map(|p| p.desired.target_id.clone())
        .collect();
    for p in &mut plan {
        if p.desired.key.kind == "mcp_tool" && p.write_intent(Uuid::nil()).is_some() {
            if let Some(tool) = package
                .tools
                .iter()
                .find(|t| t.tool_id == p.desired.target_id)
            {
                if let domain::McpToolExecutionTarget::McpProxy {
                    upstream_connection_id,
                    ..
                } = &tool.execution_target
                {
                    if !usable_connections.contains(&upstream_connection_id.to_string()) {
                        p.decision = TemplateMergeDecision::SkipUserModified;
                    }
                }
            }
        }
    }
    let usable_tools: BTreeSet<_> = plan
        .iter()
        .filter(|p| {
            p.desired.key.kind == "mcp_tool"
                && (p.write_intent(Uuid::nil()).is_some()
                    || p.current_fingerprint.as_deref() == Some(p.desired.fingerprint.as_str()))
        })
        .map(|p| p.desired.target_id.clone())
        .collect();
    for p in &mut plan {
        if p.desired.key.kind == "mcp_discovery_policy"
            && !usable_instances.contains(&p.desired.target_id)
        {
            p.decision = TemplateMergeDecision::SkipUserDeleted;
        }
        if matches!(p.desired.key.kind.as_str(), "mcp_group" | "mcp_binding") {
            let parts: Vec<String> = serde_json::from_str(&p.desired.key.source_id)?;
            if !usable_instances.contains(&parts[0]) {
                p.decision = TemplateMergeDecision::SkipUserDeleted;
            } else if p.desired.key.kind == "mcp_binding"
                && p.write_intent(Uuid::nil()).is_some()
                && (!usable_tools.contains(&parts[2])
                    || (!parts[1].is_empty()
                        && !usable_groups.contains(&composite(&[&parts[0], &parts[1]]))))
            {
                p.decision = TemplateMergeDecision::SkipUserModified;
            }
        }
    }
    let allowed: BTreeSet<_> = plan
        .iter()
        .filter(|p| p.write_intent(Uuid::nil()).is_some())
        .map(|p| p.desired.key.clone())
        .collect();
    let mut effective = package.clone();
    effective
        .tools
        .retain(|t| allowed.contains(&key("mcp_tool", &t.tool_id)));
    effective
        .connections
        .retain(|c| allowed.contains(&key("mcp_connection", c.connection_id.to_string())));
    effective.instances.clear();
    for source in &package.instances {
        let id = &source.instance_id;
        let existing = current.instances.iter().find(|i| i.instance_id == *id);
        let metadata = allowed.contains(&key("mcp_instance", id));
        let mut instance = match existing {
            Some(i) => i.clone(),
            None if metadata => {
                let mut i = source.clone();
                i.groups.clear();
                i.bindings.clear();
                i
            }
            None => continue,
        };
        if metadata {
            let groups = instance.groups;
            let bindings = instance.bindings;
            let discovery_policy = instance.discovery_policy;
            instance = source.clone();
            instance.groups = groups;
            instance.bindings = bindings;
            instance.discovery_policy = discovery_policy;
        }
        let policy = allowed.contains(&key("mcp_discovery_policy", id));
        if policy {
            instance.discovery_policy = source.discovery_policy.clone();
        }
        let mut changed = metadata || policy;
        for group in &source.groups {
            if allowed.contains(&key("mcp_group", composite(&[id, &group.path]))) {
                instance.groups.retain(|g| g.path != group.path);
                instance.groups.push(group.clone());
                changed = true;
            }
        }
        for binding in &source.bindings {
            if allowed.contains(&key(
                "mcp_binding",
                composite(&[id, &binding.group_path, &binding.tool_id]),
            )) {
                instance
                    .bindings
                    .retain(|b| b.group_path != binding.group_path || b.tool_id != binding.tool_id);
                instance.bindings.push(binding.clone());
                changed = true;
            }
        }
        if changed {
            effective.instances.push(instance);
        }
    }
    Ok((plan, effective))
}
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
        if recoverable(b) {
            b.applied_fingerprint = b.committed_fingerprint.take();
            b.pending = None;
            b.committed_operation_id = None;
        }
    }
    let (plan, effective) = plan(package, &current, &baselines)?;
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
            if b.key.kind.starts_with("mcp_") && recoverable(b) {
                ensure!(
                    repo.finalize_template_write(
                        scope,
                        &b.key,
                        b.pending.as_ref().unwrap().operation_id
                    )
                    .await?,
                    "template_mcp_recovery_conflict"
                );
            }
        }
        let baselines = repo.load_template_baselines(scope).await?;
        let (plan, effective) = plan(package, &current, &baselines)?;
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
            service
                .import_bundle(ImportMcpBundleCommand {
                    actor_user_id,
                    package: effective,
                    interface_catalog: interface_catalog.to_vec(),
                    current_system_version: current_system_version.into(),
                })
                .await?;
            let actual = project(
                &snapshot(&service, actor_user_id, package, current_system_version).await?,
            )?;
            for (p, intent) in writes {
                let actual = actual
                    .iter()
                    .find(|a| a.key == p.desired.key)
                    .ok_or_else(|| anyhow::anyhow!("template_mcp_postimage_missing"))?;
                // The native owner may normalize an unsupported tool to the exact
                // existing projection. No applied write means no baseline advance.
                if p.current_fingerprint.as_deref() == Some(actual.fingerprint.as_str()) {
                    continue;
                }
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
