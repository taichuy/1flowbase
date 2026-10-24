use std::collections::BTreeSet;

use crate::{
    compiled_plan::{CompiledEdge, CompiledEdgeIndex, CompiledNode, CompiledPlan},
    execution_engine::branching::{activate_downstream_nodes, activate_indexed_downstream_nodes},
};

// This specification observes the target set and existence separately. An already-active
// error target must still count as a handled failure, and duplicate edges retain their order.
fn specification(
    plan: &CompiledPlan,
    active: &BTreeSet<String>,
    node: &CompiledNode,
    selected: Option<&str>,
) -> (BTreeSet<String>, bool) {
    let targets: Vec<_> = if plan.edges.is_empty() {
        node.downstream_node_ids.clone()
    } else {
        plan.edges
            .iter()
            .filter(|edge| {
                let handle = edge.source_handle.as_deref();
                let eligible = match (node.node_type.as_str(), selected) {
                    ("if_else", selected) => handle == selected,
                    (_, Some(selected)) => handle == Some(selected),
                    (_, None) => {
                        handle != Some("error")
                            && !handle.is_some_and(|h| h.starts_with("visible_internal_llm_tool:"))
                    }
                };
                edge.source == node.node_id && eligible
            })
            .map(|edge| edge.target.clone())
            .collect()
    };
    (
        active.iter().chain(&targets).cloned().collect(),
        !targets.is_empty(),
    )
}

#[test]
fn indexed_activation_matches_specification_for_all_small_edge_sequences() {
    let handles = [
        None,
        Some("normal_a"),
        Some("normal_b"),
        Some("error"),
        Some("visible_internal_llm_tool:"),
        Some("visible_internal_llm_tool:x"),
    ];
    let mut catalog = Vec::new();
    for source in 0..2 {
        for target in 0..2 {
            for handle in handles {
                catalog.push(CompiledEdge {
                    edge_id: String::new(),
                    source: source.to_string(),
                    target: target.to_string(),
                    source_handle: handle.map(str::to_string),
                    target_handle: None,
                });
            }
        }
    }
    let mut cases = 0;
    let mut projections = 0;
    // All ordered sequences through length 3, including parallel edges and self loops.
    // This finite input domain is evidence of implementation refinement, not a proof
    // that the entire async executor is correct for every graph or external service.
    for len in 0..=3 {
        for code in 0..catalog.len().pow(len) {
            let mut rest = code;
            let mut plan = super::plan();
            for i in 0..len {
                let mut edge = catalog[rest % catalog.len()].clone();
                rest /= catalog.len();
                edge.edge_id = i.to_string();
                plan.edges.push(edge);
            }
            let index = CompiledEdgeIndex::new(&plan.edges);
            for node_id in ["0", "1", "missing"] {
                let expected: Vec<_> = plan.edges.iter().filter(|e| e.source == node_id).collect();
                assert_eq!(index.outgoing(node_id).collect::<Vec<_>>(), expected);
                projections += 1;
            }
            for node_id in ["0", "1"] {
                for kind in ["if_else", "code"] {
                    let mut node = super::node(node_id);
                    node.node_type = kind.into();
                    for selected in handles {
                        for mask in 0..4 {
                            let active: BTreeSet<_> = (0..2)
                                .filter(|n| mask & (1 << n) != 0)
                                .map(|n| n.to_string())
                                .collect();
                            for fallback in 0..if len == 0 { 4 } else { 1 } {
                                node.downstream_node_ids =
                                    (0..fallback).map(|n| (n % 2).to_string()).collect();
                                let expected = specification(&plan, &active, &node, selected);
                                let mut direct = active.clone();
                                let direct_exists =
                                    activate_downstream_nodes(&plan, &mut direct, &node, selected);
                                let mut indexed = active.clone();
                                let indexed_exists = activate_indexed_downstream_nodes(
                                    &index,
                                    &mut indexed,
                                    &node,
                                    selected,
                                );
                                assert_eq!((direct, direct_exists), expected);
                                assert_eq!((indexed, indexed_exists), expected);
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 1_385_088);
    assert_eq!(projections, 43_275);
}
