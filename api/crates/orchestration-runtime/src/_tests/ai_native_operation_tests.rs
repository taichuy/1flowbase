use std::collections::BTreeMap;

use domain::{AiNativeCompactProfile, AiNativeOperation};
use serde_json::{json, Map};
use uuid::Uuid;

use crate::{
    compiled_plan::{CompiledNode, CompiledPlan},
    execution_engine::{normalize_plan_variable_pool, ExecutionRuntimeContext},
};

fn plan_with_start() -> CompiledPlan {
    CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1".to_string(),
        topological_order: vec!["node-start".to_string()],
        edges: Vec::new(),
        nodes: BTreeMap::from([(
            "node-start".to_string(),
            CompiledNode {
                node_id: "node-start".to_string(),
                node_type: "start".to_string(),
                alias: "start".to_string(),
                container_id: None,
                dependency_node_ids: Vec::new(),
                downstream_node_ids: Vec::new(),
                bindings: BTreeMap::new(),
                outputs: Vec::new(),
                config: json!({}),
                plugin_runtime: None,
                llm_runtime: None,
                code_runtime: None,
            },
        )]),
        compile_issues: Vec::new(),
    }
}

#[test]
fn start_builtin_always_materializes_the_canonical_default_operation() {
    let plan = plan_with_start();
    let mut variable_pool = Map::from_iter([("node-start".to_string(), json!({"query": "hello"}))]);

    normalize_plan_variable_pool(&plan, &mut variable_pool);

    assert_eq!(
        variable_pool["node-start"]["operation"],
        json!({"kind": "generate", "profile": "standard"})
    );
}

#[test]
fn runtime_context_reads_the_typed_operation_from_the_start_variable_pool() {
    let plan = plan_with_start();
    let variable_pool = Map::from_iter([(
        "node-start".to_string(),
        json!({
            "operation": {
                "kind": "compact",
                "profile": "responses_compaction_v2"
            }
        }),
    )]);

    let context = ExecutionRuntimeContext::from_plan_input(&plan, &variable_pool).unwrap();

    assert_eq!(
        context.operation(),
        AiNativeOperation::compact(AiNativeCompactProfile::ResponsesCompactionV2)
    );
}

#[test]
fn runtime_context_rejects_an_unknown_typed_operation() {
    let plan = plan_with_start();
    let variable_pool = Map::from_iter([(
        "node-start".to_string(),
        json!({"operation": {"kind": "compact", "profile": "unknown"}}),
    )]);

    let error = ExecutionRuntimeContext::from_plan_input(&plan, &variable_pool)
        .err()
        .expect("unknown operation profile must fail");

    assert!(error.to_string().contains("invalid AI Native operation"));
}

#[test]
fn start_operation_view_cannot_carry_sealed_execution_fields() {
    let plan = plan_with_start();
    let variable_pool = Map::from_iter([(
        "node-start".to_string(),
        json!({
            "operation": {
                "kind": "generate",
                "profile": "standard",
                "raw_body": {"secret": "must-not-enter-workflow-variables"}
            }
        }),
    )]);

    assert!(ExecutionRuntimeContext::from_plan_input(&plan, &variable_pool).is_err());
}
