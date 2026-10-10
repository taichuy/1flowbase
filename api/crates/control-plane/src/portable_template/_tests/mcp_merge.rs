use super::*;
use serde_json::json;
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
fn unknown_different_tool_is_never_overwritten_or_bound_by_reset() {
    let source = fixture();
    let mut current = source.clone();
    current.instances.clear();
    current.tools[0].name = "Local".into();
    let (_, effective) = plan(&source, &current, &[]).unwrap();
    assert!(effective.tools.is_empty());
    assert!(effective.instances[0].bindings.is_empty());
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
