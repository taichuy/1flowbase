use super::*;
use std::collections::BTreeSet;

#[test]
fn node_identities_are_unique_and_internal_nodes_do_not_become_palette_entries() {
    let nodes = flow_node_capabilities();
    assert_eq!(
        nodes
            .iter()
            .map(|n| n.node_type)
            .collect::<BTreeSet<_>>()
            .len(),
        nodes.len()
    );
    let internal = flow_node_capability("container_start").unwrap();
    assert!(!internal.appears_in_catalog());
    for application_type in [ApplicationType::AgentFlow, ApplicationType::Workflow] {
        assert!(internal.is_importable_builtin(application_type));
        assert!(internal.is_compiler_supported());
    }
    assert!(!internal.is_importable_builtin(ApplicationType::AgentLogs));
    assert!(flow_node_capability("future_node").is_none());
    assert!(!flow_node_capability("knowledge_retrieval")
        .unwrap()
        .is_compiler_supported());
    assert!(!flow_node_capability("unresolved_node")
        .unwrap()
        .is_importable_builtin(ApplicationType::Workflow));
}

#[test]
fn application_boundaries_and_hidden_runtime_support_are_independent() {
    let start = flow_node_capability("workflow_start").unwrap();
    assert!(start.is_importable_builtin(ApplicationType::Workflow));
    assert!(!start.is_importable_builtin(ApplicationType::AgentFlow));
    let tool = flow_node_capability("tool").unwrap();
    assert_eq!(
        tool.authoring_status,
        ApplicationNodeAuthoringStatus::Hidden
    );
    assert!(tool.is_importable_builtin(ApplicationType::Workflow));
    assert!(tool.is_compiler_supported());
    assert!(!flow_node_capability("plugin_node")
        .unwrap()
        .is_importable_builtin(ApplicationType::Workflow));
}
