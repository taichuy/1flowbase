use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::compiled_plan::{CompiledNode, CompiledPlan};

mod activation;

pub(crate) fn node(node_id: &str) -> CompiledNode {
    CompiledNode {
        node_id: node_id.into(),
        node_type: "code".into(),
        alias: node_id.into(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

pub(crate) fn plan() -> CompiledPlan {
    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "graph-specification".into(),
        schema_version: "1flowbase.flow/v2".into(),
        topological_order: Vec::new(),
        edges: Vec::new(),
        nodes: BTreeMap::new(),
        compile_issues: Vec::new(),
    }
}
