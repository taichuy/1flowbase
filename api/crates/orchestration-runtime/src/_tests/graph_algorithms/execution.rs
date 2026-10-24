use super::*;
use crate::_tests::graph_algorithms::{node, plan};
use crate::compiler::ensure_plan_execution_contract;
use std::time::Instant;

fn node_id(index: usize) -> String {
    format!("node-{index:08}")
}

fn chain_plan(count: usize, root_references: bool) -> CompiledPlan {
    let mut plan = plan();
    for i in 0..count {
        let id = node_id(i);
        let mut node = node(&id);
        node.node_type = match i {
            0 => "workflow_start",
            i if i + 1 == count => "workflow_end",
            _ => "template_transform",
        }
        .into();
        node.outputs.push(CompiledOutput {
            key: "text".into(),
            title: "Text".into(),
            value_type: "string".into(),
            selector: vec!["text".into()],
            json_schema: None,
        });
        if i > 0 {
            let source = node_id(if root_references { 0 } else { i - 1 });
            let end = i + 1 == count;
            node.bindings.insert(
                "text".into(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: if end { "selector" } else { "templated_text" }.into(),
                    raw_value: if end {
                        json!([source, "text"])
                    } else {
                        json!(format!("{{{{ {source}.text }}}}"))
                    },
                    selector_paths: vec![vec![source, "text".into()]],
                },
            );
            node.dependency_node_ids.push(node_id(i - 1));
            plan.edges.push(CompiledEdge {
                edge_id: format!("edge-{i:08}"),
                source: node_id(i - 1),
                target: id.clone(),
                source_handle: None,
                target_handle: None,
            });
        }
        if i + 1 < count {
            node.downstream_node_ids.push(node_id(i + 1));
        }
        plan.topological_order.push(id.clone());
        plan.nodes.insert(id, node);
    }
    plan
}

async fn execute_chain(
    plan: &CompiledPlan,
    bytes: usize,
) -> crate::execution_state::FlowDebugExecutionOutcome {
    let text = "x".repeat(bytes);
    let outcome = start_flow_debug_run(
        plan,
        &json!({ (node_id(0)): { "text": text } }),
        &StubProviderInvoker {
            fail: false,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: String::new(),
        },
    )
    .await
    .expect("chain must execute");
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(outcome.node_traces.len(), plan.nodes.len());
    assert!(outcome.checkpoint_snapshot.is_none());
    for (index, trace) in outcome.node_traces.iter().enumerate() {
        assert_eq!(trace.node_id, node_id(index));
        assert!(trace.error_payload.is_none());
        assert_eq!(outcome.variable_pool[&trace.node_id]["text"], text);
    }
    outcome
}

#[tokio::test]
async fn large_workflow_chain_preserves_each_output_and_trace_order() {
    for count in [750, 1400] {
        let plan = chain_plan(count, false);
        ensure_plan_execution_contract(&plan).expect("valid connected selectors");
        execute_chain(&plan, 1024).await;
    }
}

#[tokio::test]
#[ignore = "explicit profiling only; no timing threshold in the regular gate"]
async fn profile_workflow_graph_pressure() {
    let count = std::env::var("FLOW_PROFILE_NODES")
        .ok()
        .map(|value| value.parse().unwrap())
        .unwrap_or(1400);
    let bytes = std::env::var("FLOW_PROFILE_BYTES")
        .ok()
        .map(|value| value.parse().unwrap())
        .unwrap_or(1024);
    let root_references = std::env::var("FLOW_PROFILE_REFERENCE").as_deref() == Ok("root");
    let plan = chain_plan(count, root_references);
    let mut contract_us = Vec::new();
    let mut execute_us = Vec::new();
    for _ in 0..11 {
        let start = Instant::now();
        ensure_plan_execution_contract(&plan).unwrap();
        contract_us.push(start.elapsed().as_micros());
        let start = Instant::now();
        let outcome = execute_chain(&plan, bytes).await;
        execute_us.push(start.elapsed().as_micros());
        std::hint::black_box(outcome);
    }
    contract_us.sort();
    execute_us.sort();
    println!(
        "FLOW_PRESSURE {}",
        json!({
            "nodes": count,
            "edges": plan.edges.len(),
            "bytes_per_output": bytes,
            "root_references": root_references,
            "samples": 11,
            "contract_p50_us": contract_us[5],
            "contract_p95_us": contract_us[10],
            "execute_p50_us": execute_us[5],
            "execute_p95_us": execute_us[10],
            "plan_json_bytes": serde_json::to_vec(&plan).unwrap().len(),
        })
    );
}
