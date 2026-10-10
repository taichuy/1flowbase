use super::*;
use sha2::{Digest, Sha256};

pub(super) struct MergePlan {
    pub resources: Vec<PlannedTemplateResource>,
    pub effective: domain::McpBundlePackage,
    /// Only guarded ownership transitions are allowed to change a baseline target.
    pub retargets: BTreeMap<TemplateResourceKey, String>,
}
fn writes(p: &PlannedTemplateResource) -> bool {
    matches!(
        p.decision,
        TemplateMergeDecision::Initialize | TemplateMergeDecision::Update
    )
}
fn usable(p: &PlannedTemplateResource) -> bool {
    writes(p)
        || (!matches!(
            p.decision,
            TemplateMergeDecision::SkipPendingWrite | TemplateMergeDecision::RecoverCommitted
        ) && p.current_fingerprint.as_deref() == Some(p.desired.fingerprint.as_str()))
}
/// A version-independent scope name. Occupied candidates are skipped even when their
/// contents match: only an applied baseline authorizes reuse of a template copy.
fn fresh_tool_id(
    scope: &TemplateBaselineScope,
    source: &str,
    occupied: &mut BTreeSet<String>,
) -> Result<String> {
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            scope.workspace_id,
            &scope.template_id,
            source
        ))?)
    );
    let prefix = format!("template_{}", &hash[..32]);
    for suffix in 0..=occupied.len() {
        let id = if suffix == 0 {
            prefix.clone()
        } else {
            format!("{prefix}_{suffix}")
        };
        if occupied.insert(id.clone()) {
            return Ok(id);
        }
    }
    anyhow::bail!("template_mcp_tool_identity_exhausted")
}
fn target_parts(target: &str) -> Result<[String; 3]> {
    Ok(serde_json::from_str(target)?)
}

pub(super) fn plan(
    scope: &TemplateBaselineScope,
    package: &domain::McpBundlePackage,
    current: &domain::McpBundlePackage,
    baselines: &[TemplateResourceBaseline],
) -> Result<MergePlan> {
    let mut desired = project(package)?;
    let actual = project(current)?;
    let actual_by_target: BTreeMap<_, _> = actual
        .iter()
        .map(|p| ((p.key.kind.as_str(), p.target_id.as_str()), p))
        .collect();
    let history: BTreeMap<_, _> = baselines.iter().map(|b| (&b.key, b)).collect();
    // Reserve the complete source namespace up front, not only already-created rows.
    // fresh_tool_id also reserves each allocation before planning the next tool.
    let mut occupied: BTreeSet<_> = current
        .tools
        .iter()
        .chain(&package.tools)
        .map(|t| t.tool_id.clone())
        .chain(
            baselines
                .iter()
                .filter(|b| b.key.kind == "mcp_tool")
                .map(|b| b.target_id.clone()),
        )
        .collect();
    let mut tool_targets = BTreeMap::new();
    for source in desired.iter().filter(|p| p.key.kind == "mcp_tool") {
        let target = if let Some(baseline) = history.get(&source.key) {
            // Never fork around pending, edited or deleted owned copies.
            baseline.target_id.clone()
        } else if actual_by_target
            .get(&("mcp_tool", source.target_id.as_str()))
            .is_some_and(|p| p.fingerprint != source.fingerprint)
        {
            fresh_tool_id(scope, &source.key.source_id, &mut occupied)?
        } else {
            source.target_id.clone()
        };
        tool_targets.insert(source.key.source_id.clone(), target);
    }
    let mut proposed_bindings = BTreeMap::new();
    for p in &mut desired {
        match p.key.kind.as_str() {
            "mcp_tool" => p.target_id = tool_targets[&p.key.source_id].clone(),
            "mcp_binding" => {
                let parts = target_parts(&p.key.source_id)?;
                let tool = tool_targets.get(&parts[2]).unwrap_or(&parts[2]);
                let proposed = composite(&[&parts[0], &parts[1], tool]);
                proposed_bindings.insert(p.key.clone(), proposed.clone());
                p.target_id = if let Some(baseline) = history.get(&p.key) {
                    // First plan against the old identity. The general planner's strict
                    // target invariant remains intact; retarget is an explicit CAS below.
                    baseline.target_id.clone()
                } else if proposed != p.key.source_id
                    && actual_by_target.contains_key(&("mcp_binding", p.key.source_id.as_str()))
                {
                    // An unowned existing binding is not permission to add a duplicate
                    // binding with a new tool identity, even when its metadata is equal.
                    p.key.source_id.clone()
                } else {
                    proposed
                };
            }
            _ => {}
        }
    }
    // No broad text rewriting: only typed tool identity and binding target slots change.
    let mut resources = plan_template_resources(desired, &actual, baselines, &BTreeSet::new())?;
    let mut retargets = BTreeMap::new();
    for p in &mut resources {
        if p.desired.key.kind != "mcp_binding" {
            continue;
        }
        let proposed = &proposed_bindings[&p.desired.key];
        if let Some(baseline) = history.get(&p.desired.key) {
            if baseline.target_id != *proposed
                && matches!(
                    p.decision,
                    TemplateMergeDecision::Unchanged | TemplateMergeDecision::Update
                )
            {
                let old = target_parts(&baseline.target_id)?;
                let new = target_parts(proposed)?;
                ensure!(
                    old[..2] == new[..2],
                    "template_mcp_binding_identity_conflict"
                );
                if actual_by_target.contains_key(&("mcp_binding", proposed.as_str())) {
                    p.decision = TemplateMergeDecision::SkipUnknownBaseline;
                } else {
                    retargets.insert(p.desired.key.clone(), baseline.target_id.clone());
                    p.desired.target_id = proposed.clone();
                    p.decision = TemplateMergeDecision::Update;
                }
            }
        }
    }
    let present_instances: BTreeSet<_> = current
        .instances
        .iter()
        .map(|i| i.instance_id.clone())
        .collect();
    let usable_instances: BTreeSet<_> = resources
        .iter()
        .filter(|p| {
            p.desired.key.kind == "mcp_instance"
                && (present_instances.contains(&p.desired.target_id)
                    || p.decision == TemplateMergeDecision::Initialize)
        })
        .map(|p| p.desired.target_id.clone())
        .collect();
    let usable_groups: BTreeSet<_> = resources
        .iter()
        .filter(|p| {
            p.desired.key.kind == "mcp_group"
                && (p.current_fingerprint.is_some()
                    || p.decision == TemplateMergeDecision::Initialize)
        })
        .map(|p| p.desired.target_id.clone())
        .chain(current.instances.iter().flat_map(|i| {
            i.groups
                .iter()
                .map(move |g| composite(&[&i.instance_id, &g.path]))
        }))
        .collect();
    let usable_connections: BTreeSet<_> = resources
        .iter()
        .filter(|p| p.desired.key.kind == "mcp_connection" && usable(p))
        .map(|p| p.desired.target_id.clone())
        .collect();
    for p in &mut resources {
        if p.desired.key.kind == "mcp_tool" && writes(p) {
            if let Some(tool) = package
                .tools
                .iter()
                .find(|t| t.tool_id == p.desired.key.source_id)
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
    let usable_tools: BTreeSet<_> = resources
        .iter()
        .filter(|p| p.desired.key.kind == "mcp_tool" && usable(p))
        .map(|p| p.desired.target_id.clone())
        .collect();
    for p in &mut resources {
        if p.desired.key.kind == "mcp_discovery_policy"
            && !usable_instances.contains(&p.desired.target_id)
        {
            p.decision = TemplateMergeDecision::SkipUserDeleted;
        }
        if matches!(p.desired.key.kind.as_str(), "mcp_group" | "mcp_binding") {
            let parts: Vec<String> = serde_json::from_str(&p.desired.target_id)?;
            if !usable_instances.contains(&parts[0]) {
                p.decision = TemplateMergeDecision::SkipUserDeleted;
            } else if p.desired.key.kind == "mcp_binding"
                && writes(p)
                && (!usable_tools.contains(&parts[2])
                    || (!parts[1].is_empty()
                        && !usable_groups.contains(&composite(&[&parts[0], &parts[1]]))))
            {
                p.decision = TemplateMergeDecision::SkipUserModified;
            }
        }
    }
    let allowed: BTreeMap<_, _> = resources
        .iter()
        .filter(|p| writes(p))
        .map(|p| (p.desired.key.clone(), p))
        .collect();
    retargets.retain(|key, _| allowed.contains_key(key));
    let mut effective = package.clone();
    effective.tools = package
        .tools
        .iter()
        .filter_map(|tool| {
            allowed.get(&key("mcp_tool", &tool.tool_id)).map(|p| {
                let mut t = tool.clone();
                t.tool_id = p.desired.target_id.clone();
                t
            })
        })
        .collect();
    effective
        .connections
        .retain(|c| allowed.contains_key(&key("mcp_connection", c.connection_id.to_string())));
    effective.instances.clear();
    for source in &package.instances {
        let id = &source.instance_id;
        let existing = current.instances.iter().find(|i| i.instance_id == *id);
        let metadata = allowed.contains_key(&key("mcp_instance", id));
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
            let policy = instance.discovery_policy;
            instance = source.clone();
            instance.groups = groups;
            instance.bindings = bindings;
            instance.discovery_policy = policy;
        }
        let policy = allowed.contains_key(&key("mcp_discovery_policy", id));
        if policy {
            instance.discovery_policy = source.discovery_policy.clone();
        }
        let mut changed = metadata || policy;
        for group in &source.groups {
            if allowed.contains_key(&key("mcp_group", composite(&[id, &group.path]))) {
                instance.groups.retain(|g| g.path != group.path);
                instance.groups.push(group.clone());
                changed = true;
            }
        }
        for source_binding in &source.bindings {
            let source_key = key(
                "mcp_binding",
                composite(&[id, &source_binding.group_path, &source_binding.tool_id]),
            );
            if let Some(p) = allowed.get(&source_key) {
                let parts = target_parts(&p.desired.target_id)?;
                if let Some(old) = retargets.get(&source_key) {
                    let old = target_parts(old)?;
                    instance
                        .bindings
                        .retain(|b| b.group_path != old[1] || b.tool_id != old[2]);
                }
                let mut binding = source_binding.clone();
                binding.tool_id = parts[2].clone();
                instance
                    .bindings
                    .retain(|b| b.group_path != binding.group_path || b.tool_id != binding.tool_id);
                instance.bindings.push(binding);
                changed = true;
            }
        }
        if changed {
            effective.instances.push(instance);
        }
    }
    drop(allowed);
    Ok(MergePlan {
        resources,
        effective,
        retargets,
    })
}
