use super::*;
use crate::compiled_plan::CompiledEdge;

struct CountTokensInvoker {
    captured: Arc<Mutex<Vec<ProviderCountTokensInput>>>,
    unsupported: bool,
}

#[async_trait]
impl ProviderInvoker for CountTokensInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        panic!("CountTokens must not fall back to Generate")
    }

    async fn count_tokens(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .push(input);
        if self.unsupported {
            anyhow::bail!("provider CountTokens is unsupported");
        }
        Ok(ProviderCountTokensResult {
            operation: ProviderWireOperation::CountTokens,
            input_tokens: 37,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for CountTokensInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("fixture has no capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for CountTokensInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("fixture has no code nodes")
    }
}

struct CompactInvoker {
    captured: Arc<Mutex<Vec<ProviderInvocationInput>>>,
    result: ProviderCompactResult,
    unsupported: bool,
}

#[async_trait]
impl ProviderInvoker for CompactInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        panic!("Compact must not fall back to Generate")
    }

    async fn compact(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult> {
        self.captured
            .lock()
            .expect("capture mutex poisoned")
            .push(input);
        if self.unsupported {
            anyhow::bail!("provider Compact is unsupported");
        }
        Ok(self.result.clone())
    }
}

#[async_trait]
impl CapabilityInvoker for CompactInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("fixture has no capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for CompactInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("fixture has no code nodes")
    }
}

fn compact_result(profile: ProviderCompactProfile) -> ProviderCompactResult {
    match profile {
        ProviderCompactProfile::ResponsesCompact => ProviderCompactResult::ResponseItems {
            operation: ProviderWireOperation::Compact,
            profile,
            response_items: vec![json!({ "type": "message", "content": "compacted" })],
        },
        ProviderCompactProfile::ResponsesCompactionV2 => {
            ProviderCompactResult::CompletedOpaqueCompactionItem {
                operation: ProviderWireOperation::Compact,
                profile,
                response_id: Some("resp-compact".to_string()),
                compaction_item: json!({
                    "type": "compaction",
                    "encrypted_content": "opaque-canary",
                }),
                encrypted_content: "opaque-canary".to_string(),
            }
        }
    }
}

fn selected_branch_llm_plan() -> (CompiledPlan, CompiledLlmRuntime) {
    let mut plan = branch_plan(true, None);
    let llm_template = base_plan().nodes["node-llm"].clone();
    for node_id in ["node-if-answer", "node-elseif-answer", "node-else-answer"] {
        let mut llm = llm_template.clone();
        llm.node_id = node_id.to_string();
        llm.alias = node_id.to_string();
        llm.dependency_node_ids = vec!["node-if".to_string()];
        llm.downstream_node_ids.clear();
        plan.nodes.insert(node_id.to_string(), llm);
    }
    (plan, llm_template.llm_runtime.expect("fixture LLM runtime"))
}

fn answer_node(node_id: &str, text: &str) -> CompiledNode {
    CompiledNode {
        node_id: node_id.to_string(),
        node_type: "answer".to_string(),
        alias: node_id.to_string(),
        container_id: None,
        dependency_node_ids: vec!["node-if".to_string()],
        downstream_node_ids: vec![],
        bindings: BTreeMap::from([(
            "answer_template".to_string(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "templated_text".to_string(),
                selector_paths: Vec::new(),
                raw_value: json!(text),
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
    }
}

fn branch_plan(include_else_if_edge: bool, regex_pattern: Option<&str>) -> CompiledPlan {
    let regex_pattern = regex_pattern.unwrap_or("^enterprise-");
    let mut nodes = BTreeMap::new();

    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-if".to_string()],
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-if".to_string(),
        CompiledNode {
            node_id: "node-if".to_string(),
            node_type: "if_else".to_string(),
            alias: "If / Else".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec![
                "node-if-answer".to_string(),
                "node-elseif-answer".to_string(),
                "node-else-answer".to_string(),
            ],
            bindings: BTreeMap::from([(
                "branches".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "if_else_branches".to_string(),
                    selector_paths: vec![
                        vec!["node-start".to_string(), "status".to_string()],
                        vec!["node-start".to_string(), "segment".to_string()],
                    ],
                    raw_value: json!({
                        "branches": [
                            {
                                "id": "if",
                                "kind": "if",
                                "title": "If",
                                "sourceHandle": "if",
                                "condition": {
                                    "operator": "and",
                                    "conditions": [{
                                        "kind": "rule",
                                        "left": ["node-start", "status"],
                                        "comparator": "equals",
                                        "right": { "kind": "constant", "value": "vip" }
                                    }]
                                }
                            },
                            {
                                "id": "else-if-1",
                                "kind": "else_if",
                                "title": "Else If 1",
                                "sourceHandle": "else-if-1",
                                "condition": {
                                    "operator": "and",
                                    "conditions": [{
                                        "kind": "rule",
                                        "left": ["node-start", "segment"],
                                        "comparator": "matches_regex",
                                        "right": { "kind": "constant", "value": regex_pattern }
                                    }]
                                }
                            },
                            {
                                "id": "else",
                                "kind": "else",
                                "title": "Else",
                                "sourceHandle": "else"
                            }
                        ]
                    }),
                },
            )]),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-if-answer".to_string(),
        answer_node("node-if-answer", "if"),
    );
    nodes.insert(
        "node-elseif-answer".to_string(),
        answer_node("node-elseif-answer", "else-if"),
    );
    nodes.insert(
        "node-else-answer".to_string(),
        answer_node("node-else-answer", "else"),
    );

    let mut edges = vec![
        CompiledEdge {
            edge_id: "edge-start-if".to_string(),
            source: "node-start".to_string(),
            target: "node-if".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-if-answer".to_string(),
            source: "node-if".to_string(),
            target: "node-if-answer".to_string(),
            source_handle: Some("if".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-else-answer".to_string(),
            source: "node-if".to_string(),
            target: "node-else-answer".to_string(),
            source_handle: Some("else".to_string()),
            target_handle: None,
        },
    ];

    if include_else_if_edge {
        edges.push(CompiledEdge {
            edge_id: "edge-elseif-answer".to_string(),
            source: "node-if".to_string(),
            target: "node-elseif-answer".to_string(),
            source_handle: Some("else-if-1".to_string()),
            target_handle: None,
        });
    }

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-branches".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec![
            "node-start".to_string(),
            "node-if".to_string(),
            "node-if-answer".to_string(),
            "node-elseif-answer".to_string(),
            "node-else-answer".to_string(),
        ],
        edges,
        nodes,
        compile_issues: Vec::new(),
    }
}

async fn run_branch_plan(input_payload: Value, include_else_if_edge: bool) -> Vec<String> {
    start_flow_debug_run(
        &branch_plan(include_else_if_edge, None),
        &input_payload,
        &successful_invoker(),
    )
    .await
    .unwrap()
    .node_traces
    .into_iter()
    .map(|trace| trace.node_id)
    .collect()
}

#[tokio::test]
async fn selected_llm_branch_is_the_only_canonical_generate_consumer() {
    let mut plan = branch_plan(true, None);
    let llm_template = base_plan()
        .nodes
        .get("node-llm")
        .expect("base plan must contain an LLM node")
        .clone();
    for node_id in ["node-if-answer", "node-elseif-answer", "node-else-answer"] {
        let mut llm = llm_template.clone();
        llm.node_id = node_id.to_string();
        llm.alias = node_id.to_string();
        llm.dependency_node_ids = vec!["node-if".to_string()];
        llm.downstream_node_ids = Vec::new();
        plan.nodes.insert(node_id.to_string(), llm);
    }

    let (invoker, captured_inputs) =
        sequential_tool_output_invoker(vec![ProviderInvocationOutput {
            events: vec![ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Stop,
            }],
            result: ProviderInvocationResult {
                final_content: Some("selected".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        }]);
    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "route this request",
                "status": "vip",
                "segment": "enterprise-a",
                "model": "requested/model",
                "operation": {"kind": "generate", "profile": "local_summary"}
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let traced_nodes = outcome
        .node_traces
        .iter()
        .map(|trace| trace.node_id.as_str())
        .collect::<Vec<_>>();
    assert!(traced_nodes.contains(&"node-if-answer"));
    assert!(!traced_nodes.contains(&"node-elseif-answer"));
    assert!(!traced_nodes.contains(&"node-else-answer"));
    let captured = captured_inputs.lock().expect("inputs mutex poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].operation, ProviderWireOperation::Generate);
}

/// Root #1453 / Delivery #1457: the selected graph LLM is the CountTokens consumer.
#[tokio::test]
async fn selected_llm_branch_emits_one_typed_count_tokens_terminal_with_canonical_prompt() {
    let mut plan = branch_plan(true, None);
    let llm_template = base_plan().nodes["node-llm"].clone();
    for node_id in ["node-if-answer", "node-elseif-answer", "node-else-answer"] {
        let mut llm = llm_template.clone();
        llm.node_id = node_id.to_string();
        llm.alias = node_id.to_string();
        llm.dependency_node_ids = vec!["node-if".to_string()];
        llm.downstream_node_ids.clear();
        plan.nodes.insert(node_id.to_string(), llm);
    }
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = CountTokensInvoker {
        captured: captured.clone(),
        unsupported: false,
    };
    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "query": "canonical count prompt",
            "status": "vip",
            "segment": "enterprise-a",
            "operation": { "kind": "count_tokens", "profile": null }
        }}),
        &invoker,
    )
    .await
    .expect("selected CountTokens consumer should complete");

    assert_eq!(
        count_tokens_receipt_from_traces(&outcome.node_traces)
            .unwrap()
            .input_tokens(),
        37
    );
    let captured = captured.lock().expect("capture mutex poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].provider_instance_id,
        llm_template
            .llm_runtime
            .as_ref()
            .unwrap()
            .provider_instance_id
    );
    assert_eq!(
        captured[0].model,
        llm_template.llm_runtime.as_ref().unwrap().model
    );
    assert!(captured[0]
        .messages
        .iter()
        .any(|message| message.content.contains("canonical count prompt")));
}

#[tokio::test]
async fn count_tokens_stops_at_the_selected_llm_instead_of_entering_generate_downstream_nodes() {
    let plan = base_plan();
    let invoker = CountTokensInvoker {
        captured: Arc::new(Mutex::new(Vec::new())),
        unsupported: false,
    };
    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "query": "count without generating an answer",
            "operation": { "kind": "count_tokens", "profile": null }
        }}),
        &invoker,
    )
    .await
    .expect("CountTokens should finish at its selected LLM consumer");

    assert_eq!(
        count_tokens_receipt_from_traces(&outcome.node_traces)
            .unwrap()
            .input_tokens(),
        37
    );
    assert!(!outcome
        .node_traces
        .iter()
        .any(|trace| trace.node_id == "node-answer"));
}

#[tokio::test]
async fn count_tokens_fails_when_no_llm_branch_is_selected() {
    let plan = branch_plan(false, None);
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = CountTokensInvoker {
        captured: captured.clone(),
        unsupported: false,
    };
    let error = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "status": "regular",
            "segment": "enterprise-a",
            "operation": { "kind": "count_tokens", "profile": null }
        }}),
        &invoker,
    )
    .await
    .expect_err("CountTokens completion without an LLM consumer must fail");
    assert!(error
        .to_string()
        .contains("without a typed token-count terminal"));
    assert!(captured.lock().expect("capture mutex poisoned").is_empty());
}

#[tokio::test]
async fn count_tokens_fails_when_multiple_llm_consumers_are_reached() {
    let mut plan = base_plan();
    plan.nodes.remove("node-answer");
    plan.topological_order
        .retain(|node_id| node_id != "node-answer");
    plan.edges.retain(|edge| edge.target != "node-answer");
    plan.nodes
        .get_mut("node-llm")
        .unwrap()
        .downstream_node_ids
        .clear();
    let mut second = plan.nodes["node-llm"].clone();
    second.node_id = "node-llm-second".to_string();
    second.alias = "LLM second".to_string();
    second.dependency_node_ids = vec!["node-start".to_string()];
    second.downstream_node_ids.clear();
    plan.nodes
        .get_mut("node-start")
        .unwrap()
        .downstream_node_ids
        .push(second.node_id.clone());
    plan.edges.push(CompiledEdge {
        edge_id: "edge-start-llm".to_string(),
        source: "node-start".to_string(),
        target: "node-llm".to_string(),
        source_handle: None,
        target_handle: None,
    });
    plan.edges.push(CompiledEdge {
        edge_id: "edge-start-llm-second".to_string(),
        source: "node-start".to_string(),
        target: second.node_id.clone(),
        source_handle: None,
        target_handle: None,
    });
    plan.topological_order.push(second.node_id.clone());
    plan.nodes.insert(second.node_id.clone(), second);
    let invoker = CountTokensInvoker {
        captured: Arc::new(Mutex::new(Vec::new())),
        unsupported: false,
    };
    let error = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "query": "count twice",
            "operation": { "kind": "count_tokens", "profile": null }
        }}),
        &invoker,
    )
    .await
    .expect_err("multiple CountTokens consumers must fail");
    assert!(error.to_string().contains("expected exactly one"));
}

#[tokio::test]
async fn unsupported_count_tokens_provider_fails_without_generate_fallback() {
    let plan = base_plan();
    let invoker = CountTokensInvoker {
        captured: Arc::new(Mutex::new(Vec::new())),
        unsupported: true,
    };
    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "query": "unsupported",
            "operation": { "kind": "count_tokens", "profile": null }
        }}),
        &invoker,
    )
    .await
    .expect("provider failure is represented by the execution outcome");
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
}

/// Root #1453 / Delivery #1457: both canonical Compact profiles use the selected graph LLM.
#[tokio::test]
async fn selected_llm_branch_emits_typed_compact_terminal_for_both_profiles() {
    for (profile, profile_name) in [
        (
            ProviderCompactProfile::ResponsesCompact,
            "responses_compact",
        ),
        (
            ProviderCompactProfile::ResponsesCompactionV2,
            "responses_compaction_v2",
        ),
    ] {
        let (plan, runtime) = selected_branch_llm_plan();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let invoker = CompactInvoker {
            captured: captured.clone(),
            result: compact_result(profile),
            unsupported: false,
        };
        let outcome = start_flow_debug_run(
            &plan,
            &json!({ "node-start": {
                "query": "canonical compact prompt",
                "status": "vip",
                "segment": "enterprise-a",
                "operation": { "kind": "compact", "profile": profile_name }
            }}),
            &invoker,
        )
        .await
        .expect("selected Compact consumer should complete");

        let receipt = compact_operation_receipt_from_traces(&outcome.node_traces).unwrap();
        assert_eq!(receipt.result(), &compact_result(profile));
        let captured = captured.lock().expect("capture mutex poisoned");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].operation, ProviderWireOperation::Compact);
        assert_eq!(captured[0].profile, Some(profile));
        assert_eq!(
            captured[0].provider_instance_id,
            runtime.provider_instance_id
        );
        assert_eq!(captured[0].model, runtime.model);
        assert!(captured[0]
            .messages
            .iter()
            .any(|message| message.content.contains("canonical compact prompt")));
    }
}

#[tokio::test]
async fn compact_fails_when_no_llm_consumer_is_selected() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let invoker = CompactInvoker {
        captured: captured.clone(),
        result: compact_result(ProviderCompactProfile::ResponsesCompact),
        unsupported: false,
    };
    let error = start_flow_debug_run(
        &branch_plan(false, None),
        &json!({ "node-start": {
            "status": "regular",
            "segment": "enterprise-a",
            "operation": { "kind": "compact", "profile": "responses_compact" }
        }}),
        &invoker,
    )
    .await
    .expect_err("Compact completion without an LLM consumer must fail");
    assert!(error
        .to_string()
        .contains("without a typed Compact terminal"));
    assert!(captured.lock().expect("capture mutex poisoned").is_empty());
}

#[tokio::test]
async fn compact_fails_when_multiple_llm_consumers_are_reached() {
    let mut plan = base_plan();
    plan.nodes.remove("node-answer");
    plan.topological_order
        .retain(|node_id| node_id != "node-answer");
    plan.edges.retain(|edge| edge.target != "node-answer");
    plan.nodes
        .get_mut("node-llm")
        .unwrap()
        .downstream_node_ids
        .clear();
    let mut second = plan.nodes["node-llm"].clone();
    second.node_id = "node-llm-second".to_string();
    second.alias = "LLM second".to_string();
    second.dependency_node_ids = vec!["node-start".to_string()];
    second.downstream_node_ids.clear();
    plan.nodes
        .get_mut("node-start")
        .unwrap()
        .downstream_node_ids
        .push(second.node_id.clone());
    plan.edges.push(CompiledEdge {
        edge_id: "edge-start-llm".to_string(),
        source: "node-start".to_string(),
        target: "node-llm".to_string(),
        source_handle: None,
        target_handle: None,
    });
    plan.edges.push(CompiledEdge {
        edge_id: "edge-start-llm-second".to_string(),
        source: "node-start".to_string(),
        target: second.node_id.clone(),
        source_handle: None,
        target_handle: None,
    });
    plan.topological_order.push(second.node_id.clone());
    plan.nodes.insert(second.node_id.clone(), second);
    let invoker = CompactInvoker {
        captured: Arc::new(Mutex::new(Vec::new())),
        result: compact_result(ProviderCompactProfile::ResponsesCompact),
        unsupported: false,
    };
    let error = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "query": "compact twice",
            "operation": { "kind": "compact", "profile": "responses_compact" }
        }}),
        &invoker,
    )
    .await
    .expect_err("multiple Compact consumers must fail");
    assert!(error.to_string().contains("expected exactly one"));
}

#[tokio::test]
async fn compact_rejects_wrong_provider_operation_and_profile() {
    let invalid_results = [
        ProviderCompactResult::ResponseItems {
            operation: ProviderWireOperation::Generate,
            profile: ProviderCompactProfile::ResponsesCompact,
            response_items: vec![json!({ "type": "message" })],
        },
        compact_result(ProviderCompactProfile::ResponsesCompactionV2),
    ];
    for result in invalid_results {
        let (plan, _) = selected_branch_llm_plan();
        let invoker = CompactInvoker {
            captured: Arc::new(Mutex::new(Vec::new())),
            result,
            unsupported: false,
        };
        let outcome = start_flow_debug_run(
            &plan,
            &json!({ "node-start": {
                "query": "invalid compact result",
                "status": "vip",
                "operation": { "kind": "compact", "profile": "responses_compact" }
            }}),
            &invoker,
        )
        .await
        .expect("provider contract mismatch is represented by the execution outcome");
        assert!(matches!(
            outcome.stop_reason,
            ExecutionStopReason::Failed(_)
        ));
    }
}

#[tokio::test]
async fn unsupported_compact_provider_fails_without_generate_fallback() {
    let (plan, _) = selected_branch_llm_plan();
    let invoker = CompactInvoker {
        captured: Arc::new(Mutex::new(Vec::new())),
        result: compact_result(ProviderCompactProfile::ResponsesCompact),
        unsupported: true,
    };
    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": {
            "query": "unsupported compact",
            "status": "vip",
            "operation": { "kind": "compact", "profile": "responses_compact" }
        }}),
        &invoker,
    )
    .await
    .expect("provider failure is represented by the execution outcome");
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
}

#[tokio::test]
async fn if_else_runs_first_matching_if_branch() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "vip", "segment": "enterprise-a" } }),
        true,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if", "node-if-answer"]);
}

#[tokio::test]
async fn if_else_runs_else_if_branch_after_if_misses() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "regular", "segment": "enterprise-a" } }),
        true,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if", "node-elseif-answer"]);
}

#[tokio::test]
async fn if_else_falls_back_to_else_branch() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "regular", "segment": "consumer" } }),
        true,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if", "node-else-answer"]);
}

#[tokio::test]
async fn if_else_selected_unconnected_branch_naturally_ends() {
    let traces = run_branch_plan(
        json!({ "node-start": { "status": "regular", "segment": "enterprise-a" } }),
        false,
    )
    .await;

    assert_eq!(traces, vec!["node-start", "node-if"]);
}

#[tokio::test]
async fn if_else_invalid_regex_does_not_match() {
    let outcome = start_flow_debug_run(
        &branch_plan(true, Some("[")),
        &json!({ "node-start": { "status": "regular", "segment": "enterprise-a" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();
    let traces = outcome
        .node_traces
        .into_iter()
        .map(|trace| trace.node_id)
        .collect::<Vec<_>>();

    assert_eq!(traces, vec!["node-start", "node-if", "node-else-answer"]);
}

#[tokio::test]
async fn if_else_empty_comparator_matches_missing_null_empty_string_and_empty_array() {
    let mut plan = branch_plan(true, None);
    let branch_binding = plan
        .nodes
        .get_mut("node-if")
        .expect("branch plan should include if_else node")
        .bindings
        .get_mut("branches")
        .expect("if_else node should include branches binding");

    branch_binding.raw_value = json!({
        "branches": [
            {
                "id": "if",
                "kind": "if",
                "title": "If",
                "sourceHandle": "if",
                "condition": {
                    "operator": "and",
                    "conditions": [{
                        "kind": "rule",
                        "left": ["node-start", "maybe"],
                        "comparator": "empty"
                    }]
                }
            },
            {
                "id": "else",
                "kind": "else",
                "title": "Else",
                "sourceHandle": "else"
            }
        ]
    });

    for payload in [
        json!({ "node-start": {} }),
        json!({ "node-start": { "maybe": null } }),
        json!({ "node-start": { "maybe": "" } }),
        json!({ "node-start": { "maybe": [] } }),
    ] {
        let outcome = start_flow_debug_run(&plan, &payload, &successful_invoker())
            .await
            .unwrap();
        let traces = outcome
            .node_traces
            .into_iter()
            .map(|trace| trace.node_id)
            .collect::<Vec<_>>();

        assert_eq!(traces, vec!["node-start", "node-if", "node-if-answer"]);
    }

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "maybe": "ready" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();
    let traces = outcome
        .node_traces
        .into_iter()
        .map(|trace| trace.node_id)
        .collect::<Vec<_>>();

    assert_eq!(traces, vec!["node-start", "node-if", "node-else-answer"]);
}

#[tokio::test]
async fn if_else_evaluates_nested_groups_and_selector_right_values() {
    let mut plan = branch_plan(true, None);
    let branch_binding = plan
        .nodes
        .get_mut("node-if")
        .expect("branch plan should include if_else node")
        .bindings
        .get_mut("branches")
        .expect("if_else node should include branches binding");

    branch_binding.raw_value = json!({
        "branches": [
            {
                "id": "if",
                "kind": "if",
                "title": "If",
                "sourceHandle": "if",
                "condition": {
                    "operator": "and",
                    "conditions": [
                        {
                            "kind": "rule",
                            "left": ["node-start", "status"],
                            "comparator": "exists"
                        },
                        {
                            "operator": "or",
                            "conditions": [
                                {
                                    "kind": "rule",
                                    "left": ["node-start", "segment"],
                                    "comparator": "equals",
                                    "right": {
                                        "kind": "selector",
                                        "selector": ["node-start", "expected_segment"]
                                    }
                                },
                                {
                                    "kind": "rule",
                                    "left": ["node-start", "score"],
                                    "comparator": "greater_than",
                                    "right": { "kind": "constant", "value": 90 }
                                }
                            ]
                        }
                    ]
                }
            },
            {
                "id": "else",
                "kind": "else",
                "title": "Else",
                "sourceHandle": "else"
            }
        ]
    });

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "status": "ready",
                "segment": "enterprise-a",
                "expected_segment": "enterprise-a",
                "score": 10
            }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();
    let traces = outcome
        .node_traces
        .into_iter()
        .map(|trace| trace.node_id)
        .collect::<Vec<_>>();

    assert_eq!(traces, vec!["node-start", "node-if", "node-if-answer"]);
}
