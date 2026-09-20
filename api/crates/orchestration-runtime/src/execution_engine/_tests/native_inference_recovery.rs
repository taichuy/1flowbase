use super::*;
use crate::compiled_plan::{CompiledEdge, CompiledLlmRuntime, CompiledNode};

fn node(id: &str, kind: &str) -> CompiledNode {
    CompiledNode {
        node_id: id.into(),
        node_type: kind.into(),
        alias: id.into(),
        container_id: None,
        dependency_node_ids: vec![],
        downstream_node_ids: vec![],
        bindings: Default::default(),
        outputs: vec![],
        config: json!({}),
        plugin_runtime: None,
        code_runtime: None,
        llm_runtime: (kind == "llm").then(|| CompiledLlmRuntime {
            provider_instance_id: "provider".into(),
            provider_instance_display_name: "fixture".into(),
            provider_code: "openai".into(),
            protocol: "responses".into(),
            model: "model".into(),
            routing: None,
        }),
    }
}
fn routed_plan() -> CompiledPlan {
    let shapes = [
        ("start", "start"),
        ("router", "if_else"),
        ("llm-a", "llm"),
        ("llm-b", "llm"),
        ("llm-c", "llm"),
        ("llm-d", "llm"),
        ("aggregate", "variable_aggregator"),
        ("answer-a", "answer"),
        ("answer-b", "answer"),
    ];
    let pairs = [
        ("start", "router"),
        ("router", "llm-a"),
        ("router", "llm-b"),
        ("router", "llm-c"),
        ("router", "llm-d"),
        ("llm-a", "aggregate"),
        ("llm-b", "aggregate"),
        ("llm-c", "aggregate"),
        ("llm-d", "aggregate"),
        ("aggregate", "answer-a"),
        ("aggregate", "answer-b"),
    ];
    CompiledPlan {
        flow_id: uuid::Uuid::nil(),
        source_draft_id: "fixture".into(),
        schema_version: "1".into(),
        topological_order: shapes.iter().map(|(id, _)| id.to_string()).collect(),
        nodes: shapes
            .iter()
            .map(|(id, kind)| (id.to_string(), node(id, kind)))
            .collect(),
        compile_issues: vec![],
        edges: pairs
            .iter()
            .enumerate()
            .map(|(i, (source, target))| CompiledEdge {
                edge_id: i.to_string(),
                source: source.to_string(),
                target: target.to_string(),
                source_handle: None,
                target_handle: None,
            })
            .collect(),
    }
}
#[test]
fn actual_four_llm_shape_admits_each_selected_llm_without_other_routes() {
    let plan = routed_plan();
    for selected in ["llm-a", "llm-b", "llm-c", "llm-d"] {
        let scope = recovery_scope(&plan, selected).unwrap();
        assert_eq!(
            scope,
            BTreeSet::from([
                selected.into(),
                "aggregate".into(),
                "answer-a".into(),
                "answer-b".into()
            ])
        );
    }
    assert!(validate_native_inference_recovery_scope(&plan, "router").is_err());
}
#[test]
fn downstream_side_effect_second_llm_and_backward_edge_are_rejected() {
    for kind in [
        "llm",
        "sql",
        "http_request",
        "code",
        "plugin_node",
        "variable_assigner",
    ] {
        let mut plan = routed_plan();
        plan.nodes
            .insert("aggregate".into(), node("aggregate", kind));
        assert!(
            validate_native_inference_recovery_scope(&plan, "llm-b").is_err(),
            "{kind}"
        );
    }
    let mut plan = routed_plan();
    plan.edges.push(CompiledEdge {
        edge_id: "cycle".into(),
        source: "aggregate".into(),
        target: "llm-b".into(),
        source_handle: None,
        target_handle: None,
    });
    assert!(validate_native_inference_recovery_scope(&plan, "llm-b").is_err());
}
