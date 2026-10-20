use super::*;
use crate::node_error_policy::ERROR_BRANCH_SOURCE_HANDLE;

#[tokio::test]
async fn failed_llm_does_not_expose_error_text_to_downstream_answer_contract() {
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![
        final_llm_response("first answer"),
        ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
    ]);
    let mut plan = multi_llm_answer_plan();
    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.dependency_node_ids = vec!["node-llm".to_string(), "node-llm-2".to_string()];
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "templated_text".to_string(),
            selector_paths: vec![
                vec!["node-llm".to_string(), "text".to_string()],
                vec!["node-llm-2".to_string(), "text".to_string()],
            ],
            raw_value: json!("{{ node-llm.text }}\n----\n{{ node-llm-2.text }}"),
        },
    )]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm-2");
            assert!(!outcome.variable_pool.contains_key("node-llm-2"));
            assert!(!outcome.variable_pool.contains_key("node-answer"));
            assert!(outcome
                .node_traces
                .iter()
                .all(|trace| trace.node_id != "node-answer"));
        }
        other => panic!("expected failed stop reason before answer, got {other:?}"),
    }
}

#[tokio::test]
async fn failed_llm_with_compiled_edges_does_not_activate_terminal_answer() {
    let mut plan = llm_answer_plan();
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-llm".to_string(),
            source: "node-start".to_string(),
            target: "node-llm".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &StubProviderInvoker {
            fail: true,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: String::new(),
        },
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert!(!outcome.variable_pool.contains_key("node-llm"));
            assert!(!outcome.variable_pool.contains_key("node-answer"));
            assert!(outcome
                .node_traces
                .iter()
                .all(|trace| trace.node_id != "node-answer"));
        }
        other => panic!("expected failed stop reason before terminal answer, got {other:?}"),
    }
}

#[tokio::test]
async fn d1_ac_001_provider_failure_without_explicit_error_policy_does_not_materialize_answer() {
    let mut plan = llm_answer_plan();
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-llm".to_string(),
            source: "node-start".to_string(),
            target: "node-llm".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &StubProviderInvoker {
            fail: true,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: String::new(),
        },
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
    assert!(
        !outcome.variable_pool.contains_key("node-answer"),
        "D1-AC-001: a provider failure cannot synthesize a public Answer without an explicit error policy"
    );
}

#[tokio::test]
async fn failed_llm_with_default_value_policy_continues_normal_branch() {
    let mut plan = llm_answer_plan();
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-llm".to_string(),
            source: "node-start".to_string(),
            target: "node-llm".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["error_policy"] = json!("default_value");
    llm.config["error_default_output"] = json!({ "text": "兜底回复" });

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &StubProviderInvoker {
            fail: true,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: String::new(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(outcome.variable_pool["node-llm"]["text"], json!("兜底回复"));
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("兜底回复")
    );
}

#[tokio::test]
async fn failed_llm_with_error_branch_policy_activates_only_error_branch() {
    let mut plan = llm_answer_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-answer".to_string(),
        "node-error-answer".to_string(),
    ];
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-llm".to_string(),
            source: "node-start".to_string(),
            target: "node-llm".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-error-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-error-answer".to_string(),
            source_handle: Some(ERROR_BRANCH_SOURCE_HANDLE.to_string()),
            target_handle: None,
        },
    ];
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["error_policy"] = json!("error_branch");
    llm.downstream_node_ids = vec!["node-answer".to_string(), "node-error-answer".to_string()];
    let mut error_answer = plan
        .nodes
        .get("node-answer")
        .expect("answer node should exist")
        .clone();
    error_answer.node_id = "node-error-answer".to_string();
    error_answer.alias = "Error Answer".to_string();
    error_answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "templated_text".to_string(),
            selector_paths: Vec::new(),
            raw_value: json!("handled: provider unavailable"),
        },
    )]);
    plan.nodes
        .insert("node-error-answer".to_string(), error_answer);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &StubProviderInvoker {
            fail: true,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: String::new(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert!(!outcome.variable_pool.contains_key("node-answer"));
    assert_eq!(
        outcome.variable_pool["node-error-answer"]["answer"],
        json!("handled: provider unavailable")
    );
    assert_eq!(
        outcome
            .node_traces
            .iter()
            .map(|trace| trace.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["node-start", "node-llm", "node-error-answer"]
    );
}

#[tokio::test]
async fn failed_llm_with_inactive_later_branch_still_stops_before_terminal_answer() {
    let mut plan = llm_answer_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-if".to_string(),
        "node-llm".to_string(),
        "node-answer".to_string(),
        "node-plugin".to_string(),
    ];
    plan.edges = vec![
        CompiledEdge {
            edge_id: "edge-start-if".to_string(),
            source: "node-start".to_string(),
            target: "node-if".to_string(),
            source_handle: None,
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-if-llm".to_string(),
            source: "node-if".to_string(),
            target: "node-llm".to_string(),
            source_handle: Some("if".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-else-plugin".to_string(),
            source: "node-if".to_string(),
            target: "node-plugin".to_string(),
            source_handle: Some("else".to_string()),
            target_handle: None,
        },
        CompiledEdge {
            edge_id: "edge-llm-answer".to_string(),
            source: "node-llm".to_string(),
            target: "node-answer".to_string(),
            source_handle: None,
            target_handle: None,
        },
    ];
    let start = plan
        .nodes
        .get_mut("node-start")
        .expect("start node should exist");
    start.downstream_node_ids = vec!["node-if".to_string()];
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.dependency_node_ids = vec!["node-if".to_string()];
    llm.downstream_node_ids = vec!["node-answer".to_string()];
    plan.nodes.insert(
        "node-if".to_string(),
        CompiledNode {
            node_id: "node-if".to_string(),
            node_type: "if_else".to_string(),
            alias: "If / Else".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-llm".to_string(), "node-plugin".to_string()],
            bindings: BTreeMap::from([(
                "branches".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "if_else_branches".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
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
                                        "left": ["node-start", "query"],
                                        "comparator": "exists"
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
    plan.nodes.insert(
        "node-plugin".to_string(),
        CompiledNode {
            node_id: "node-plugin".to_string(),
            node_type: "plugin_node".to_string(),
            alias: "Inactive Plugin".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-if".to_string()],
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &StubProviderInvoker {
            fail: true,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: String::new(),
        },
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert!(!outcome.variable_pool.contains_key("node-llm"));
            assert!(!outcome.variable_pool.contains_key("node-answer"));
            assert!(!outcome.variable_pool.contains_key("node-plugin"));
            assert!(outcome
                .node_traces
                .iter()
                .all(|trace| trace.node_id != "node-answer"));
        }
        other => panic!("expected failed stop reason before active terminal answer, got {other:?}"),
    }
}

#[tokio::test]
async fn answer_node_keeps_partial_output_when_template_selector_is_unresolved() {
    let mut plan = llm_answer_plan();
    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "templated_text".to_string(),
            selector_paths: vec![
                vec!["node-llm".to_string(), "text".to_string()],
                vec!["node-llm-1".to_string(), "text".to_string()],
            ],
            raw_value: json!("Answer: {{ node-llm.text }}\nMissing: {{ node-llm-1.text }}"),
        },
    )]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &StubProviderInvoker {
            fail: false,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: "visible answer".to_string(),
        },
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-answer");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("prompt_template_unresolved")
            );
        }
        other => panic!("expected answer node failure, got {other:?}"),
    }

    let answer_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-answer")
        .expect("answer trace should exist");
    assert_eq!(
        answer_trace.output_payload["answer"],
        json!("Answer: visible answer\nMissing: ")
    );
    assert_eq!(
        answer_trace.output_payload["error"]["error_code"],
        json!("prompt_template_unresolved")
    );
    assert_eq!(
        answer_trace.output_payload["error"]["details"][0]["selector"],
        json!("node-llm-1.text")
    );
    assert_eq!(
        answer_trace
            .error_payload
            .as_ref()
            .expect("answer trace should keep structured error")["error_code"],
        json!("prompt_template_unresolved")
    );
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("Answer: visible answer\nMissing: ")
    );
}

#[tokio::test]
async fn native_responses_terminal_persists_only_ephemeral_continuation_marker() {
    let plan = base_plan();
    let (invoker, _) = sequential_tool_output_invoker(vec![ProviderInvocationOutput {
        events: vec![ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        }],
        result: ProviderInvocationResult {
            response_id: Some("resp_provider_owned".to_string()),
            finish_reason: Some(ProviderFinishReason::Stop),
            ..ProviderInvocationResult::default()
        },
        first_token_at: None,
        time_to_first_token_ms: None,
    }]);
    let plan_input = serde_json::Map::from_iter([(
        "node-start".to_string(),
        json!({ "query": "native terminal" }),
    )]);
    let runtime_context = ExecutionRuntimeContext::from_plan_input(&plan, &plan_input)
        .unwrap()
        .with_provider_invocation_capability(
            extension_contracts::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
        );

    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "native terminal" } }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::WaitingHuman(_)
    ));
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert!(llm_trace.output_payload.get("response_id").is_none());
    assert_eq!(
        llm_trace.output_payload["provider_continuation"]["storage"],
        json!("ephemeral")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["status"],
        "succeeded"
    );
}

struct CanonicalDynamicDistributionInvoker;

impl_noop_code_invoker!(CanonicalDynamicDistributionInvoker);

#[async_trait]
impl ProviderInvoker for CanonicalDynamicDistributionInvoker {
    async fn provider_distribution_registry_fingerprint(&self) -> Result<String> {
        Ok("fixture-registry-fingerprint".to_string())
    }

    async fn select_provider_distribution(
        &self,
        _plugin_id: &str,
        invocation: extension_contracts::ProviderDistributionInvocation,
    ) -> Result<extension_contracts::ProviderDistributionSelectionReceipt> {
        Ok(extension_contracts::ProviderDistributionSelectionReceipt {
            invocation_id: invocation.invocation_id,
            rule_id: invocation.rule_id,
            rule_version: invocation.rule_version,
            contract_version: invocation.contract_version,
            registry_fingerprint: invocation.registry_fingerprint,
            attempt: invocation.attempt,
            decision: extension_contracts::ProviderDistributionDecision::Select {
                target_id: invocation.candidates[0].target_id.clone(),
            },
        })
    }

    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Ok(final_provider_output(format!(
            "winner:{}",
            runtime.provider_instance_id
        )))
    }
}

#[async_trait]
impl CapabilityInvoker for CanonicalDynamicDistributionInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("distribution fixture does not execute capability nodes")
    }
}

fn model_group_llm_runtime(
    distribution_rule: LlmDistributionRule,
    provider_instance_ids: &[&str],
) -> CompiledLlmRuntime {
    CompiledLlmRuntime {
        provider_instance_id: provider_instance_ids[0].to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: Some(CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id: Some("model-group-test".to_string()),
            queue_snapshot_id: None,
            queue_targets: provider_instance_ids
                .iter()
                .map(|provider_instance_id| CompiledLlmRouteTarget {
                    provider_instance_id: (*provider_instance_id).to_string(),
                    provider_instance_display_name: String::new(),
                    provider_code: "fixture_provider".to_string(),
                    protocol: "openai_compatible".to_string(),
                    upstream_model_id: "gpt-5.4-mini".to_string(),
                })
                .collect(),
            distribution_rule: distribution_rule.clone(),
            distribution_key: (distribution_rule == LlmDistributionRule::RoundRobin).then(|| {
                "llm-router:workspace:workspace-1:provider:fixture_provider:model:gpt-5.4-mini:targets:test"
                    .to_string()
            }),
            context_policy: json!({}),
            stream_policy: json!({}),
        }),
    }
}

#[path = "session_affinity/mod.rs"]
mod session_affinity;

mod retry_semantics;
mod routing_failover;
