use std::collections::BTreeMap;

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderFinishReason, ProviderInvocationInput,
    ProviderInvocationResult, ProviderWireOperation,
};
use serde_json::{json, Value};
use uuid::Uuid;

use orchestration_runtime::{
    compiled_plan::{
        CompiledBinding, CompiledCodeRuntime, CompiledEdge, CompiledLlmRuntime, CompiledNode,
        CompiledOutput, CompiledPlan, CompiledPluginRuntime, COMPACT_SOURCE_HANDLE_ID,
    },
    execution_engine::{
        start_flow_debug_run_with_runtime_context, CapabilityInvocationOutput, CapabilityInvoker,
        CodeInvocationOutput, CodeInvoker, ExecutionRuntimeContext, ProviderInvocationOutput,
        ProviderInvoker,
    },
    execution_state::{
        compact_response_receipt_from_trace, ApplicationFlowExecutionIntent,
        CompactResponseIngress, CompactResponseProfile, CompactResponseReceipt,
    },
};

struct CompactResponseInvoker;

#[async_trait]
impl ProviderInvoker for CompactResponseInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!("Compact Response fixture must not invoke an LLM node")
    }
}

#[async_trait]
impl CapabilityInvoker for CompactResponseInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("Compact Response fixture must not invoke a capability node")
    }
}

#[async_trait]
impl CodeInvoker for CompactResponseInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("Compact Response fixture must not invoke a Code node")
    }
}

fn compact_dispatch_plan() -> CompiledPlan {
    let start = CompiledNode {
        node_id: "node-start".to_string(),
        node_type: "start".to_string(),
        alias: "Start".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: vec![
            "node-answer".to_string(),
            "node-compact-response".to_string(),
        ],
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({ "compact_dispatch": "application_flow" }),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };
    let answer = CompiledNode {
        node_id: "node-answer".to_string(),
        node_type: "answer".to_string(),
        alias: "Answer".to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-start".to_string()],
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::from([(
            "answer_template".to_string(),
            CompiledBinding {
                kind: "templated_text".to_string(),
                raw_value: json!("ordinary answer"),
                selector_paths: Vec::new(),
            },
        )]),
        outputs: vec![CompiledOutput {
            key: "answer".to_string(),
            title: "Answer".to_string(),
            value_type: "string".to_string(),
            selector: Vec::new(),
            json_schema: None,
        }],
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };
    let compact_response = CompiledNode {
        node_id: "node-compact-response".to_string(),
        node_type: "compact_response".to_string(),
        alias: "Compact Response".to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-start".to_string()],
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };

    CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: "compact-dispatch".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec![
            "node-start".to_string(),
            "node-answer".to_string(),
            "node-compact-response".to_string(),
        ],
        edges: vec![
            CompiledEdge {
                edge_id: "edge-start-answer".to_string(),
                source: "node-start".to_string(),
                target: "node-answer".to_string(),
                source_handle: None,
                target_handle: None,
            },
            CompiledEdge {
                edge_id: "edge-start-compact-response".to_string(),
                source: "node-start".to_string(),
                target: "node-compact-response".to_string(),
                source_handle: Some(COMPACT_SOURCE_HANDLE_ID.to_string()),
                target_handle: None,
            },
        ],
        nodes: BTreeMap::from([
            (start.node_id.clone(), start),
            (answer.node_id.clone(), answer),
            (compact_response.node_id.clone(), compact_response),
        ]),
        compile_issues: Vec::new(),
    }
}

fn successful_local_generate_result() -> ProviderInvocationResult {
    ProviderInvocationResult {
        final_content: Some("typed local summary".to_string()),
        finish_reason: Some(ProviderFinishReason::Stop),
        ..ProviderInvocationResult::default()
    }
}

fn trace_ids(
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> Vec<&str> {
    outcome
        .node_traces
        .iter()
        .map(|trace| trace.node_id.as_str())
        .collect()
}

#[tokio::test]
async fn ordinary_application_flow_selects_start_default_and_never_materializes_compact_response() {
    let plan = compact_dispatch_plan();
    let context = ExecutionRuntimeContext::default();
    assert_eq!(
        context.execution_intent(),
        ApplicationFlowExecutionIntent::Ordinary
    );
    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "ordinary request" } }),
        context,
        &CompactResponseInvoker,
    )
    .await
    .expect("ordinary application-flow run should complete through Answer");

    assert_eq!(trace_ids(&outcome), vec!["node-start", "node-answer"]);
    assert!(outcome
        .node_traces
        .iter()
        .all(|trace| trace.node_type != "compact_response"));
}

#[tokio::test]
async fn typed_compact_ingress_selects_start_compact_and_emits_the_local_generate_receipt() {
    let plan = compact_dispatch_plan();
    let context = ExecutionRuntimeContext::default().with_application_flow_compact_ingress(
        CompactResponseIngress::local_generate(successful_local_generate_result())
            .expect("successful typed Generate should enter Compact Response"),
    );
    assert_eq!(
        context.execution_intent(),
        ApplicationFlowExecutionIntent::Compact(CompactResponseProfile::LocalSummary)
    );

    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "compact request" } }),
        context,
        &CompactResponseInvoker,
    )
    .await
    .expect("typed Compact ingress should select the Compact terminal");

    assert_eq!(
        trace_ids(&outcome),
        vec!["node-start", "node-compact-response"]
    );
    assert!(outcome
        .node_traces
        .iter()
        .all(|trace| trace.node_type != "answer"));
    let compact_trace = outcome
        .node_traces
        .last()
        .expect("Compact Response trace should exist");
    let receipt = compact_response_receipt_from_trace(compact_trace)
        .expect("runtime Compact receipt should be decodable")
        .expect("Compact Response trace should contain a receipt");
    assert_eq!(receipt.profile(), CompactResponseProfile::LocalSummary);
    assert_eq!(
        receipt
            .generate_result()
            .and_then(|result| result.final_content.as_deref()),
        Some("typed local summary")
    );
}

#[tokio::test]
async fn compact_response_preserves_typed_legacy_items_and_v2_opaque_provider_result() {
    let plan = compact_dispatch_plan();
    let legacy_result = ProviderCompactResult::ResponseItems {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompact,
        response_items: vec![json!({ "type": "message", "id": "item-1" })],
    };
    let legacy_context = ExecutionRuntimeContext::default().with_application_flow_compact_ingress(
        CompactResponseIngress::responses_compact(legacy_result.clone())
            .expect("typed legacy Compact items should be accepted"),
    );
    let legacy_outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "legacy compact" } }),
        legacy_context,
        &CompactResponseInvoker,
    )
    .await
    .expect("legacy Compact ingress should complete");
    let legacy_receipt = compact_response_receipt_from_trace(
        legacy_outcome
            .node_traces
            .last()
            .expect("legacy Compact trace should exist"),
    )
    .expect("legacy Compact receipt should decode")
    .expect("legacy Compact terminal should carry a receipt");
    assert_eq!(
        legacy_receipt.profile(),
        CompactResponseProfile::ResponsesCompact
    );
    assert_eq!(legacy_receipt.compact_result(), Some(&legacy_result));

    let opaque_canary = "opaque-v2-canary";
    let v2_result = ProviderCompactResult::CompletedOpaqueCompactionItem {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompactionV2,
        response_id: Some("resp-compact".to_string()),
        compaction_item: json!({
            "type": "compaction",
            "encrypted_content": opaque_canary,
        }),
        encrypted_content: opaque_canary.to_string(),
    };
    let v2_context = ExecutionRuntimeContext::default().with_application_flow_compact_ingress(
        CompactResponseIngress::responses_compaction_v2(v2_result.clone())
            .expect("real opaque V2 Compact result should be accepted"),
    );
    let v2_outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "v2 compact" } }),
        v2_context,
        &CompactResponseInvoker,
    )
    .await
    .expect("V2 Compact ingress should complete");
    let v2_receipt = compact_response_receipt_from_trace(
        v2_outcome
            .node_traces
            .last()
            .expect("V2 Compact trace should exist"),
    )
    .expect("V2 Compact receipt should decode")
    .expect("V2 Compact terminal should carry a receipt");
    assert_eq!(
        v2_receipt.profile(),
        CompactResponseProfile::ResponsesCompactionV2
    );
    assert_eq!(v2_receipt.compact_result(), Some(&v2_result));
}

#[test]
fn failed_or_malformed_provider_results_cannot_construct_a_compact_ingress() {
    let failed_generate = ProviderInvocationResult {
        final_content: Some("partial".to_string()),
        finish_reason: Some(ProviderFinishReason::Error),
        ..ProviderInvocationResult::default()
    };
    assert!(CompactResponseIngress::local_generate(failed_generate).is_err());

    let malformed_v2 = ProviderCompactResult::CompletedOpaqueCompactionItem {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompactionV2,
        response_id: None,
        compaction_item: json!({ "type": "message", "encrypted_content": "forged" }),
        encrypted_content: "forged".to_string(),
    };
    assert!(CompactResponseIngress::responses_compaction_v2(malformed_v2).is_err());

    assert!(CompactResponseReceipt::from_payload(&json!({
        "semantic_terminal": "compact_response",
        "profile": "unknown_profile",
        "result": {}
    }))
    .is_err());
}

#[tokio::test]
async fn typed_compact_ingress_fails_closed_if_a_transparent_plan_is_routed_into_the_engine() {
    let mut plan = compact_dispatch_plan();
    plan.nodes.remove("node-compact-response");
    let start = plan
        .nodes
        .get_mut("node-start")
        .expect("fixture Start should exist");
    start.config = json!({});
    start.downstream_node_ids = vec!["node-answer".to_string()];
    plan.topological_order = vec!["node-start".to_string(), "node-answer".to_string()];
    plan.edges
        .retain(|edge| edge.target != "node-compact-response");

    let context = ExecutionRuntimeContext::default().with_application_flow_compact_ingress(
        CompactResponseIngress::local_generate(successful_local_generate_result())
            .expect("successful Compact ingress fixture should be valid"),
    );
    let error = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "incorrectly routed compact" } }),
        context,
        &CompactResponseInvoker,
    )
    .await
    .expect_err("transparent Compact routing must bypass this engine");

    assert!(error.to_string().contains("transparent Start node"));
}
