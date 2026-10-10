use super::*;
use serde_json::json;
fn scope() -> TemplateBaselineScope {
    TemplateBaselineScope {
        workspace_id: Uuid::from_u128(1),
        template_id: "test-template".into(),
    }
}
fn plan(
    source: &domain::McpBundlePackage,
    current: &domain::McpBundlePackage,
    baselines: &[TemplateResourceBaseline],
) -> anyhow::Result<(Vec<PlannedTemplateResource>, domain::McpBundlePackage)> {
    let result = super::planning::plan(&scope(), source, current, baselines)?;
    Ok((result.resources, result.effective))
}

fn fixture() -> domain::McpBundlePackage {
    serde_json::from_value(json!({
        "manifest":{"schema_version":"1flowbase.mcp.bundle/v2","organization":"test","bundle_id":"template","bundle_version":"1.0.0","locale":"en_US","minimum_host_version":"1.0.0","exported_from_system_version":"1.0.0","exported_at":"2026-10-10T00:00:00Z","files":[]},
        "connections":[],
        "tools":[{"tool_id":"shared","name":"Tool","short_description":"Tool","full_description":"Tool","interface_id":"workspace_list","parameter_schema_snapshot":{},"result_schema_snapshot":{},"input_mapping":{},"output_mapping":{},"risk_level_snapshot":"low","status":"enabled"}],
        "instances":[{"instance_id":"1flowbase","name":"Official","description_short":null,"status":"enabled","default_entry_path":"","groups":[{"path":"group","display_name":"Group","description_short":null,"enabled":true,"sort_order":0}],"bindings":[{"group_path":"group","tool_id":"shared","display_alias":null,"visible":true,"sort_order":0}],"discovery_policy":{"list_default_limit":20,"list_max_depth":8,"list_regex_enabled":false,"list_regex_max_length":128,"list_return_fields":[]}}]
    })).unwrap()
}
fn baselines(package: &domain::McpBundlePackage) -> Vec<TemplateResourceBaseline> {
    project(package)
        .unwrap()
        .into_iter()
        .map(|r| TemplateResourceBaseline {
            key: r.key,
            target_id: r.target_id,
            generation: 1,
            applied_fingerprint: Some(r.fingerprint),
            pending: None,
            committed_operation_id: None,
            committed_fingerprint: None,
        })
        .collect()
}
#[test]
fn recovery_requires_pending_intent_and_matching_durable_commit_receipt() {
    let mut baseline = baselines(&fixture()).remove(0);
    let operation_id = Uuid::from_u128(101);
    let committed = "committed-content".to_string();
    baseline.committed_operation_id = Some(operation_id);
    baseline.committed_fingerprint = Some(committed.clone());
    // A receipt without a pending intent is already finalized, not recoverable.
    assert_eq!(recovery_operation(&baseline), None);
    baseline.pending = Some(TemplateWriteIntent {
        operation_id,
        target_id: baseline.target_id.clone(),
        expected_fingerprint: baseline.applied_fingerprint.clone(),
        desired_fingerprint: committed,
    });
    assert_eq!(recovery_operation(&baseline), Some(operation_id));
    baseline.committed_operation_id = Some(Uuid::from_u128(102));
    assert_eq!(recovery_operation(&baseline), None);
    baseline.committed_operation_id = None;
    assert_eq!(recovery_operation(&baseline), None);
    baseline.committed_operation_id = Some(operation_id);
    baseline.committed_fingerprint = None;
    assert_eq!(recovery_operation(&baseline), None);
}

#[test]
fn unknown_identical_shared_tool_can_satisfy_reset_without_adoption() {
    let source = fixture();
    let mut current = source.clone();
    current.instances.clear();
    let (plan, effective) = plan(&source, &current, &[]).unwrap();
    assert!(effective.tools.is_empty());
    assert_eq!(effective.instances.len(), 1);
    assert_eq!(effective.instances[0].bindings.len(), 1);
    assert_eq!(
        plan.iter()
            .find(|p| p.desired.key.kind == "mcp_tool")
            .unwrap()
            .decision,
        TemplateMergeDecision::SkipUnknownBaseline
    );
}
#[test]
fn unknown_different_tool_is_forked_without_overwriting_shared_tool() {
    let source = fixture();
    let mut current = source.clone();
    current.instances.clear();
    current.tools[0].name = "Local".into();
    let (plan, effective) = plan(&source, &current, &[]).unwrap();
    assert_eq!(effective.tools.len(), 1);
    assert_ne!(effective.tools[0].tool_id, source.tools[0].tool_id);
    assert_eq!(
        effective.instances[0].bindings[0].tool_id,
        effective.tools[0].tool_id
    );
    let tool = plan
        .iter()
        .find(|p| p.desired.key.kind == "mcp_tool")
        .unwrap();
    assert_eq!(tool.desired.key.source_id, source.tools[0].tool_id);
    assert_eq!(tool.decision, TemplateMergeDecision::Initialize);
}
#[test]
fn local_metadata_and_target_only_groups_survive_independent_group_update() {
    let original = fixture();
    let mut current = original.clone();
    let mut source = original.clone();
    current.instances[0].name = "Local".into();
    let mut extra = current.instances[0].groups[0].clone();
    extra.path = "extra".into();
    current.instances[0].groups.push(extra);
    source.instances[0].groups[0].display_name = "Updated".into();
    let (plan, effective) = plan(&source, &current, &baselines(&original)).unwrap();
    assert_eq!(effective.instances[0].name, "Local");
    assert_eq!(effective.instances[0].groups.len(), 2);
    assert!(effective.instances[0]
        .groups
        .iter()
        .any(|g| g.display_name == "Updated"));
    assert_eq!(
        plan.iter()
            .find(|p| p.desired.key.kind == "mcp_instance")
            .unwrap()
            .decision,
        TemplateMergeDecision::SkipUserModified
    );
}
#[test]
fn deleting_owned_instance_does_not_recreate_its_children() {
    let source = fixture();
    let mut current = source.clone();
    current.instances.clear();
    let (plan, effective) = plan(&source, &current, &baselines(&source)).unwrap();
    assert!(effective.instances.is_empty());
    assert!(plan
        .iter()
        .filter(|p| p.desired.key.kind != "mcp_tool")
        .all(|p| p.decision == TemplateMergeDecision::SkipUserDeleted));
}
#[test]
fn deleted_group_is_not_restored_by_a_new_binding() {
    let source = fixture();
    let mut current = source.clone();
    current.instances[0].groups.clear();
    current.instances[0].bindings.clear();
    let mut history = baselines(&source);
    history.retain(|b| b.key.kind != "mcp_binding");
    let (_, effective) = plan(&source, &current, &history).unwrap();
    assert!(effective.instances.is_empty());
}

#[test]
fn local_rename_does_not_block_template_discovery_policy_update() {
    let original = fixture();
    let mut current = original.clone();
    let mut source = original.clone();
    current.instances[0].name = "Local name".into();
    source.instances[0].discovery_policy.list_default_limit = 40;
    let (plan, effective) = plan(&source, &current, &baselines(&original)).unwrap();
    assert_eq!(effective.instances[0].name, "Local name");
    assert_eq!(
        effective.instances[0].discovery_policy.list_default_limit,
        40
    );
    assert_eq!(
        plan.iter()
            .find(|p| p.desired.key.kind == "mcp_instance")
            .unwrap()
            .decision,
        TemplateMergeDecision::SkipUserModified
    );
    assert_eq!(
        plan.iter()
            .find(|p| p.desired.key.kind == "mcp_discovery_policy")
            .unwrap()
            .decision,
        TemplateMergeDecision::Update
    );
}
#[test]
fn local_discovery_policy_does_not_block_template_rename() {
    let original = fixture();
    let mut current = original.clone();
    let mut source = original.clone();
    current.instances[0].discovery_policy.list_default_limit = 75;
    source.instances[0].name = "New official name".into();
    let (plan, effective) = plan(&source, &current, &baselines(&original)).unwrap();
    assert_eq!(effective.instances[0].name, "New official name");
    assert_eq!(
        effective.instances[0].discovery_policy.list_default_limit,
        75
    );
    assert_eq!(
        plan.iter()
            .find(|p| p.desired.key.kind == "mcp_instance")
            .unwrap()
            .decision,
        TemplateMergeDecision::Update
    );
    assert_eq!(
        plan.iter()
            .find(|p| p.desired.key.kind == "mcp_discovery_policy")
            .unwrap()
            .decision,
        TemplateMergeDecision::SkipUserModified
    );
}
#[test]
fn fresh_instance_and_legacy_reset_create_independent_policy_baseline() {
    let source = fixture();
    let mut empty = source.clone();
    empty.instances.clear();
    empty.tools.clear();
    let (new_plan, _) = plan(&source, &empty, &[]).unwrap();
    let mut legacy = source.clone();
    legacy.instances.clear();
    let (reset_plan, _) = plan(&source, &legacy, &[]).unwrap();
    for plans in [new_plan, reset_plan] {
        assert_eq!(
            plans
                .iter()
                .find(|p| p.desired.key.kind == "mcp_discovery_policy")
                .unwrap()
                .decision,
            TemplateMergeDecision::Initialize
        );
    }
}

fn tool_plan(planned: &super::planning::MergePlan) -> &PlannedTemplateResource {
    planned
        .resources
        .iter()
        .find(|p| p.desired.key.kind == "mcp_tool")
        .unwrap()
}
fn binding_plan(planned: &super::planning::MergePlan) -> &PlannedTemplateResource {
    planned
        .resources
        .iter()
        .find(|p| p.desired.key.kind == "mcp_binding")
        .unwrap()
}
fn conflict_current(source: &domain::McpBundlePackage) -> domain::McpBundlePackage {
    let mut current = source.clone();
    current.instances.clear();
    current.tools[0].name = "Local shared tool".into();
    current
}
// Materialize the planner's first successful batch as portable postimage + receipts.
// Later assertions exercise the real planner against independent user mutations.
fn first_postimage(
    source: &domain::McpBundlePackage,
    current: &domain::McpBundlePackage,
) -> (
    domain::McpBundlePackage,
    Vec<TemplateResourceBaseline>,
    String,
) {
    let planned = super::planning::plan(&scope(), source, current, &[]).unwrap();
    let target = tool_plan(&planned).desired.target_id.clone();
    let mut actual = current.clone();
    actual.tools.extend(planned.effective.tools.clone());
    actual.instances = planned.effective.instances.clone();
    let applied = planned
        .resources
        .iter()
        .filter(|p| p.write_intent(Uuid::nil()).is_some())
        .map(|p| TemplateResourceBaseline {
            key: p.desired.key.clone(),
            target_id: p.desired.target_id.clone(),
            generation: 1,
            applied_fingerprint: Some(p.desired.fingerprint.clone()),
            pending: None,
            committed_operation_id: None,
            committed_fingerprint: None,
        })
        .collect();
    (actual, applied, target)
}
#[test]
fn copy_target_is_stable_across_repeat_and_versioned_update() {
    let source = fixture();
    let current = conflict_current(&source);
    let (actual, history, target) = first_postimage(&source, &current);
    let retry = super::planning::plan(&scope(), &source, &actual, &history).unwrap();
    assert_eq!(tool_plan(&retry).desired.target_id, target);
    assert_eq!(tool_plan(&retry).decision, TemplateMergeDecision::Unchanged);
    assert!(retry.effective.tools.is_empty());
    let mut next = source.clone();
    next.manifest.bundle_version = "2.0.0".into();
    next.tools[0].name = "Updated upstream".into();
    let update = super::planning::plan(&scope(), &next, &actual, &history).unwrap();
    assert_eq!(tool_plan(&update).desired.target_id, target);
    assert_eq!(tool_plan(&update).desired.key.source_id, "shared");
    assert_eq!(tool_plan(&update).decision, TemplateMergeDecision::Update);
    assert_eq!(update.effective.tools.len(), 1);
    assert_eq!(update.effective.tools[0].tool_id, target);
    assert_eq!(
        actual
            .tools
            .iter()
            .find(|t| t.tool_id == "shared")
            .unwrap()
            .name,
        "Local shared tool"
    );
}
#[test]
fn occupied_equal_copy_is_not_adopted_and_scopes_get_distinct_copies() {
    let source = fixture();
    let mut current = conflict_current(&source);
    let first = super::planning::plan(&scope(), &source, &current, &[]).unwrap();
    let occupied = first.effective.tools[0].clone();
    current.tools.push(occupied.clone());
    let collision = super::planning::plan(&scope(), &source, &current, &[]).unwrap();
    assert_ne!(tool_plan(&collision).desired.target_id, occupied.tool_id);
    assert_eq!(
        tool_plan(&collision).decision,
        TemplateMergeDecision::Initialize
    );
    let mut other = scope();
    other.template_id = "other-template".into();
    let cross_scope = super::planning::plan(&other, &source, &current, &[]).unwrap();
    assert_ne!(
        tool_plan(&cross_scope).desired.target_id,
        tool_plan(&collision).desired.target_id
    );
    assert_ne!(tool_plan(&cross_scope).desired.target_id, occupied.tool_id);
}
#[test]
fn copy_allocation_reserves_other_source_tool_ids_in_the_same_batch() {
    let mut source = fixture();
    let current = conflict_current(&source);
    let first = super::planning::plan(&scope(), &source, &current, &[]).unwrap();
    let candidate = first.effective.tools[0].clone();
    source.tools.push(candidate.clone());
    let planned = super::planning::plan(&scope(), &source, &current, &[]).unwrap();
    let original = planned
        .resources
        .iter()
        .find(|p| p.desired.key.kind == "mcp_tool" && p.desired.key.source_id == "shared")
        .unwrap();
    assert_ne!(original.desired.target_id, candidate.tool_id);
    let ids: BTreeSet<_> = planned.effective.tools.iter().map(|t| &t.tool_id).collect();
    assert_eq!(ids.len(), 2);
}
#[test]
fn edited_deleted_and_pending_owned_copies_never_fork_again() {
    let source = fixture();
    let current = conflict_current(&source);
    let (actual, history, target) = first_postimage(&source, &current);
    for mode in ["edited", "deleted", "pending"] {
        let mut actual = actual.clone();
        let mut history = history.clone();
        let decision = match mode {
            "edited" => {
                actual
                    .tools
                    .iter_mut()
                    .find(|t| t.tool_id == target)
                    .unwrap()
                    .name = "My copy".into();
                TemplateMergeDecision::SkipUserModified
            }
            "deleted" => {
                actual.tools.retain(|t| t.tool_id != target);
                TemplateMergeDecision::SkipUserDeleted
            }
            _ => {
                let baseline = history
                    .iter_mut()
                    .find(|b| b.key.kind == "mcp_tool")
                    .unwrap();
                baseline.pending = Some(TemplateWriteIntent {
                    operation_id: Uuid::now_v7(),
                    target_id: target.clone(),
                    expected_fingerprint: baseline.applied_fingerprint.clone(),
                    desired_fingerprint: "pending".into(),
                });
                TemplateMergeDecision::SkipPendingWrite
            }
        };
        let planned = super::planning::plan(&scope(), &source, &actual, &history).unwrap();
        assert_eq!(tool_plan(&planned).desired.target_id, target);
        assert_eq!(tool_plan(&planned).decision, decision);
        assert!(planned.effective.tools.is_empty());
    }
}
#[test]
fn copy_mapping_changes_only_typed_ids_not_equal_string_literals() {
    let mut source = fixture();
    source.tools[0].name = "shared".into();
    source.tools[0].full_description = "shared".into();
    source.tools[0].input_mapping = json!({"literal":"shared"});
    source.tools[0].output_mapping = json!({"literal":"shared"});
    source.tools[0].parameter_schema_snapshot = json!({"const":"shared"});
    source.tools[0].result_schema_snapshot = json!({"const":"shared"});
    source.instances[0].bindings[0].display_alias = Some("shared".into());
    let current = conflict_current(&source);
    let planned = super::planning::plan(&scope(), &source, &current, &[]).unwrap();
    let copy = &planned.effective.tools[0];
    assert_ne!(copy.tool_id, "shared");
    assert_eq!(copy.name, "shared");
    assert_eq!(copy.full_description, "shared");
    assert_eq!(copy.input_mapping, source.tools[0].input_mapping);
    assert_eq!(copy.output_mapping, source.tools[0].output_mapping);
    assert_eq!(
        copy.parameter_schema_snapshot,
        source.tools[0].parameter_schema_snapshot
    );
    assert_eq!(
        copy.result_schema_snapshot,
        source.tools[0].result_schema_snapshot
    );
    assert_eq!(
        planned.effective.instances[0].bindings[0]
            .display_alias
            .as_deref(),
        Some("shared")
    );
    assert_eq!(
        tool_plan(&planned).desired.fingerprint,
        project(&source)
            .unwrap()
            .iter()
            .find(|p| p.key.kind == "mcp_tool")
            .unwrap()
            .fingerprint
    );
}
#[test]
fn formerly_reused_tool_forks_and_only_unchanged_owned_binding_retargets() {
    let original = fixture();
    let mut current = original.clone();
    current.instances.clear();
    let (actual, history, old_target) = first_postimage(&original, &current);
    assert_eq!(old_target, "shared");
    assert!(!history.iter().any(|b| b.key.kind == "mcp_tool"));
    let mut source = original.clone();
    source.tools[0].name = "Changed upstream".into();
    let planned = super::planning::plan(&scope(), &source, &actual, &history).unwrap();
    assert_eq!(planned.retargets.len(), 1);
    let binding = binding_plan(&planned);
    assert_eq!(binding.decision, TemplateMergeDecision::Update);
    assert_eq!(
        binding.desired.key.source_id,
        composite(&["1flowbase", "group", "shared"])
    );
    assert_eq!(
        planned.retargets[&binding.desired.key],
        binding.desired.key.source_id
    );
    assert_ne!(binding.desired.target_id, binding.desired.key.source_id);
    assert_eq!(planned.effective.instances[0].bindings.len(), 1);
    assert_eq!(
        planned.effective.instances[0].bindings[0].tool_id,
        tool_plan(&planned).desired.target_id
    );
    let mut next_actual = actual.clone();
    next_actual.tools.extend(planned.effective.tools.clone());
    next_actual.instances = planned.effective.instances.clone();
    let mut next_history = history.clone();
    for p in planned
        .resources
        .iter()
        .filter(|p| p.write_intent(Uuid::nil()).is_some())
    {
        next_history.retain(|b| b.key != p.desired.key);
        next_history.push(TemplateResourceBaseline {
            key: p.desired.key.clone(),
            target_id: p.desired.target_id.clone(),
            generation: 3,
            applied_fingerprint: Some(p.desired.fingerprint.clone()),
            pending: None,
            committed_operation_id: None,
            committed_fingerprint: None,
        });
    }
    let repeat = super::planning::plan(&scope(), &source, &next_actual, &next_history).unwrap();
    assert!(repeat.retargets.is_empty());
    assert!(repeat.effective.tools.is_empty());
    assert!(repeat.effective.instances.is_empty());
    assert_eq!(
        binding_plan(&repeat).desired.target_id,
        binding.desired.target_id
    );
    // A pre-existing destination binding cannot be silently claimed by retarget.
    let mut collision_actual = next_actual.clone();
    collision_actual.instances[0]
        .bindings
        .push(actual.instances[0].bindings[0].clone());
    let mut collision_history = next_history.clone();
    collision_history.retain(|b| b.key.kind != "mcp_binding");
    collision_history.extend(
        history
            .iter()
            .filter(|b| b.key.kind == "mcp_binding")
            .cloned(),
    );
    let collision =
        super::planning::plan(&scope(), &source, &collision_actual, &collision_history).unwrap();
    assert!(collision.retargets.is_empty());
    assert_eq!(
        binding_plan(&collision).decision,
        TemplateMergeDecision::SkipUnknownBaseline
    );
    assert!(collision.effective.instances.is_empty());
    for mode in ["edited", "deleted", "unowned", "pending"] {
        let mut current = actual.clone();
        let mut baselines = history.clone();
        let expected = match mode {
            "edited" => {
                current.instances[0].bindings[0].display_alias = Some("Local alias".into());
                TemplateMergeDecision::SkipUserModified
            }
            "deleted" => {
                current.instances[0].bindings.clear();
                TemplateMergeDecision::SkipUserDeleted
            }
            "unowned" => {
                baselines.retain(|b| b.key.kind != "mcp_binding");
                TemplateMergeDecision::SkipUnknownBaseline
            }
            _ => {
                let b = baselines
                    .iter_mut()
                    .find(|b| b.key.kind == "mcp_binding")
                    .unwrap();
                b.pending = Some(TemplateWriteIntent {
                    operation_id: Uuid::now_v7(),
                    target_id: b.target_id.clone(),
                    expected_fingerprint: b.applied_fingerprint.clone(),
                    desired_fingerprint: "pending".into(),
                });
                TemplateMergeDecision::SkipPendingWrite
            }
        };
        let skipped = super::planning::plan(&scope(), &source, &current, &baselines).unwrap();
        assert!(skipped.retargets.is_empty(), "{mode}");
        assert_eq!(binding_plan(&skipped).decision, expected, "{mode}");
        assert!(skipped.effective.instances.is_empty(), "{mode}");
    }
}
#[test]
fn multiple_template_bindings_share_one_copy() {
    let mut source = fixture();
    let mut second = source.instances[0].clone();
    second.instance_id = "second".into();
    source.instances.push(second);
    let current = conflict_current(&source);
    let planned = super::planning::plan(&scope(), &source, &current, &[]).unwrap();
    assert_eq!(planned.effective.tools.len(), 1);
    assert_eq!(planned.effective.instances.len(), 2);
    let target = &planned.effective.tools[0].tool_id;
    assert!(planned
        .effective
        .instances
        .iter()
        .all(|i| i.bindings[0].tool_id == *target));
}
