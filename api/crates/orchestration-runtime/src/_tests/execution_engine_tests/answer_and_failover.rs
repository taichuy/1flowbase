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
async fn llm_node_retry_routes_next_target_before_first_token() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);
    llm.llm_runtime = Some(CompiledLlmRuntime {
        provider_instance_id: "provider-primary".to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "primary-model".to_string(),
        routing: Some(CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id: Some("queue-template-1".to_string()),
            queue_snapshot_id: Some("queue-snapshot-1".to_string()),
            queue_targets: vec![
                CompiledLlmRouteTarget {
                    provider_instance_id: "provider-primary".to_string(),
                    provider_instance_display_name: String::new(),
                    provider_code: "fixture_provider".to_string(),
                    protocol: "openai_compatible".to_string(),
                    upstream_model_id: "primary-model".to_string(),
                },
                CompiledLlmRouteTarget {
                    provider_instance_id: "provider-backup".to_string(),
                    provider_instance_display_name: String::new(),
                    provider_code: "fixture_provider".to_string(),
                    protocol: "openai_compatible".to_string(),
                    upstream_model_id: "backup-model".to_string(),
                },
            ],
            distribution_rule: LlmDistributionRule::RoundRobin,
            distribution_key: Some("retry-routing".to_string()),
            context_policy: json!({}),
            stream_policy: json!({}),
        }),
    });
    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = FailFirstFailoverInvoker {
        calls: calls.clone(),
    };

    let runtime_context = ExecutionRuntimeContext::from_plan_input(
        &plan,
        &serde_json::Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]),
    )
    .expect("runtime context should parse")
    .with_llm_routing_counter_store(Arc::new(RecordingRoutingCounterStore::default()));
    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-primary", "provider-backup"]
    );
    assert_eq!(
        llm_trace.output_payload["text"],
        json!("winner:backup-model")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["status"],
        json!("failed")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["status"],
        json!("succeeded")
    );
    assert_eq!(
        llm_trace.metrics_payload["queue_snapshot_id"],
        json!("queue-snapshot-1")
    );
}

#[tokio::test]
async fn failed_retry_attempts_preserve_each_upstream_body_and_terminal_uses_the_last() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);

    let first_body = " {\"attempt\":0,\"future\":{\"shape\":true}}\n ";
    let final_body = "<html>attempt 1 failed</html>\r\n";
    let failed_output = |message: &str| ProviderInvocationOutput {
        events: vec![ProviderStreamEvent::Error {
            error: ProviderRuntimeError {
                kind: ProviderRuntimeErrorKind::ProviderUpstreamError,
                message: message.to_string(),
                provider_summary: Some(message.to_string()),
                provider_details: Some(json!({ "status": 502 })),
            },
        }],
        result: ProviderInvocationResult::default(),
        first_token_at: None,
        time_to_first_token_ms: None,
    };
    let (invoker, _) =
        sequential_tool_output_invoker(vec![failed_output(first_body), failed_output(final_body)]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .expect("runtime should return a failed outcome");
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["error_message_ref"],
        json!(format!("runtime_artifact:inline:error:{first_body}"))
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["error_message_ref"],
        json!(format!("runtime_artifact:inline:error:{final_body}"))
    );
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.error_payload["message"], json!(final_body));
        }
        other => panic!("expected terminal provider failure, got {other:?}"),
    }
}

#[tokio::test]
async fn anthropic_callback_retry_marks_upstream_429_as_pre_token_failure_without_gateway_retry() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);

    let rejected = ProviderInvocationOutput {
        events: vec![ProviderStreamEvent::Error {
            error: ProviderRuntimeError {
                kind: ProviderRuntimeErrorKind::ProviderUpstreamError,
                message:
                    r#"{"error":{"message":"Service Unavailable","type":"error"},"type":"error"}"#
                        .to_string(),
                provider_summary: Some("Service Unavailable".to_string()),
                provider_details: Some(json!({"status": 429})),
            },
        }],
        result: ProviderInvocationResult::default(),
        first_token_at: None,
        time_to_first_token_ms: None,
    };
    let (invoker, captured_inputs) = sequential_tool_output_invoker(vec![
        rejected,
        final_provider_output("must not be called".to_string()),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start": {"query": "hello"}}), &invoker)
        .await
        .expect("runtime should return a failed outcome");
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(captured_inputs.lock().unwrap().len(), 1);
    assert_eq!(
        llm_trace.metrics_payload["attempts"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["error_code"],
        json!("provider_upstream_error")
    );
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.error_payload["status_code"], json!(429));
            assert_eq!(
                failure.error_payload["failed_after_first_token"],
                json!(false)
            );
        }
        other => panic!("expected terminal provider failure, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_node_retries_protocol_only_empty_response() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);
    let (invoker, captured_inputs) = sequential_tool_output_invoker(vec![
        ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::UsageSnapshot {
                    usage: ProviderUsage {
                        input_tokens: Some(19),
                        output_tokens: Some(0),
                        total_tokens: Some(19),
                        ..ProviderUsage::default()
                    },
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                usage: ProviderUsage {
                    input_tokens: Some(19),
                    output_tokens: Some(0),
                    total_tokens: Some(19),
                    ..ProviderUsage::default()
                },
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        },
        provider_output(final_llm_response("retry succeeded")),
    ]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(
        captured_inputs.lock().expect("inputs mutex poisoned").len(),
        2
    );
    assert_eq!(llm_trace.output_payload["text"], json!("retry succeeded"));
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["status"],
        json!("empty_response")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["retry_reason"],
        json!("empty_response")
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
            plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
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

#[tokio::test]
async fn provider_routing_does_not_retry_when_llm_node_retry_is_disabled() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(false);
    llm.llm_runtime = Some(model_group_llm_runtime(
        LlmDistributionRule::None,
        &["provider-a", "provider-b"],
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = FailFirstFailoverInvoker {
        calls: calls.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a"]
    );
}

#[tokio::test]
async fn round_robin_distribution_rotates_first_attempt_across_runs() {
    // AC-006: round_robin rotates aggregate model attempts A/B/A via ephemeral counter.
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.llm_runtime = Some(model_group_llm_runtime(
        LlmDistributionRule::RoundRobin,
        &["provider-a", "provider-b"],
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let counter_store = Arc::new(RecordingRoutingCounterStore::default());
    let invoker = RecordingSuccessInvoker {
        calls: calls.clone(),
    };

    for _ in 0..3 {
        let runtime_context = ExecutionRuntimeContext::from_plan_input(
            &plan,
            &serde_json::Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]),
        )
        .expect("runtime context should parse")
        .with_llm_routing_counter_store(counter_store.clone());
        start_flow_debug_run_with_runtime_context(
            &plan,
            &json!({ "node-start": { "query": "hello" } }),
            runtime_context,
            &invoker,
        )
        .await
        .unwrap();
    }

    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a", "provider-b", "provider-a"]
    );
    let keys = counter_store
        .keys
        .lock()
        .expect("counter keys mutex poisoned");
    assert!(keys.iter().all(|key| key.contains("workspace:workspace-1")));
    assert!(keys
        .iter()
        .all(|key| key.contains("provider:fixture_provider")));
    assert!(keys.iter().all(|key| key.contains("model:gpt-5.4-mini")));
    assert!(keys.iter().all(|key| key.contains("targets:")));
}

#[tokio::test]
async fn none_distribution_keeps_existing_attempt_order_across_runs() {
    // AC-007: none preserves the current ordered failover behavior.
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.llm_runtime = Some(model_group_llm_runtime(
        LlmDistributionRule::None,
        &["provider-a", "provider-b"],
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let counter_store = Arc::new(RecordingRoutingCounterStore::default());
    let invoker = RecordingSuccessInvoker {
        calls: calls.clone(),
    };

    for _ in 0..3 {
        let runtime_context = ExecutionRuntimeContext::from_plan_input(
            &plan,
            &serde_json::Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]),
        )
        .expect("runtime context should parse")
        .with_llm_routing_counter_store(counter_store.clone());
        start_flow_debug_run_with_runtime_context(
            &plan,
            &json!({ "node-start": { "query": "hello" } }),
            runtime_context,
            &invoker,
        )
        .await
        .unwrap();
    }

    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a", "provider-a", "provider-a"]
    );
    assert!(counter_store
        .keys
        .lock()
        .expect("counter keys mutex poisoned")
        .is_empty());
}

#[tokio::test]
async fn retry_round_robin_resets_to_first_target_across_runs() {
    // AC-002: a new LLM node call always starts at target A.
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(false);
    llm.llm_runtime = Some(model_group_llm_runtime(
        LlmDistributionRule::RetryRoundRobin,
        &["provider-a", "provider-b", "provider-c"],
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let counter_store = Arc::new(RecordingRoutingCounterStore::default());
    let invoker = RecordingSuccessInvoker {
        calls: calls.clone(),
    };

    for _ in 0..3 {
        let runtime_context = ExecutionRuntimeContext::from_plan_input(
            &plan,
            &serde_json::Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]),
        )
        .expect("runtime context should parse")
        .with_llm_routing_counter_store(counter_store.clone());
        start_flow_debug_run_with_runtime_context(
            &plan,
            &json!({ "node-start": { "query": "hello" } }),
            runtime_context,
            &invoker,
        )
        .await
        .unwrap();
    }

    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a", "provider-a", "provider-a"]
    );
    assert!(counter_store
        .keys
        .lock()
        .expect("counter keys mutex poisoned")
        .is_empty());
}

#[tokio::test]
async fn retry_round_robin_cycles_targets_within_llm_retry_budget() {
    // AC-003/AC-008: retries use A/B/C/A and keep attempt facts.
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(3);
    llm.config["retry_interval_ms"] = json!(0);
    llm.llm_runtime = Some(model_group_llm_runtime(
        LlmDistributionRule::RetryRoundRobin,
        &["provider-a", "provider-b", "provider-c"],
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let counter_store = Arc::new(RecordingRoutingCounterStore::default());
    let invoker = FailFirstAttemptsInvoker {
        calls: calls.clone(),
        remaining_failures: std::sync::atomic::AtomicUsize::new(3),
    };
    let runtime_context = ExecutionRuntimeContext::from_plan_input(
        &plan,
        &serde_json::Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]),
    )
    .expect("runtime context should parse")
    .with_llm_routing_counter_store(counter_store.clone());

    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a", "provider-b", "provider-c", "provider-a"]
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["is_retry"],
        json!(false)
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["is_retry"],
        json!(true)
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["provider_instance_id"],
        json!("provider-b")
    );
    assert!(counter_store
        .keys
        .lock()
        .expect("counter keys mutex poisoned")
        .is_empty());
}

#[tokio::test]
async fn retry_round_robin_keeps_concurrent_retry_sequences_request_local() {
    // AC-004: concurrent calls do not share a retry cursor or routing counter.
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);
    llm.llm_runtime = Some(model_group_llm_runtime(
        LlmDistributionRule::RetryRoundRobin,
        &["provider-a", "provider-b"],
    ));
    let counter_store = Arc::new(RecordingRoutingCounterStore::default());
    let calls_a = Arc::new(Mutex::new(Vec::new()));
    let calls_b = Arc::new(Mutex::new(Vec::new()));
    let invoker_a = FailFirstAttemptsInvoker {
        calls: calls_a.clone(),
        remaining_failures: std::sync::atomic::AtomicUsize::new(1),
    };
    let invoker_b = FailFirstAttemptsInvoker {
        calls: calls_b.clone(),
        remaining_failures: std::sync::atomic::AtomicUsize::new(1),
    };
    let plan_input =
        serde_json::Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]);
    let runtime_context_a = ExecutionRuntimeContext::from_plan_input(&plan, &plan_input)
        .expect("runtime context should parse")
        .with_llm_routing_counter_store(counter_store.clone());
    let runtime_context_b = ExecutionRuntimeContext::from_plan_input(&plan, &plan_input)
        .expect("runtime context should parse")
        .with_llm_routing_counter_store(counter_store.clone());
    let input = json!({ "node-start": { "query": "hello" } });

    let (outcome_a, outcome_b) = tokio::join!(
        start_flow_debug_run_with_runtime_context(&plan, &input, runtime_context_a, &invoker_a,),
        start_flow_debug_run_with_runtime_context(&plan, &input, runtime_context_b, &invoker_b,)
    );

    outcome_a.unwrap();
    outcome_b.unwrap();
    assert_eq!(
        calls_a.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a", "provider-b"]
    );
    assert_eq!(
        calls_b.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-a", "provider-b"]
    );
    assert!(counter_store
        .keys
        .lock()
        .expect("counter keys mutex poisoned")
        .is_empty());
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
            queue_template_id: None,
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
            distribution_rule,
            distribution_key: (distribution_rule == LlmDistributionRule::RoundRobin).then(|| {
                "llm-router:workspace:workspace-1:provider:fixture_provider:model:gpt-5.4-mini:targets:test"
                    .to_string()
            }),
            context_policy: json!({}),
            stream_policy: json!({}),
        }),
    }
}

#[tokio::test]
async fn failover_queue_stops_when_primary_fails_after_finish_error_with_first_token() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);
    llm.llm_runtime = Some(CompiledLlmRuntime {
        provider_instance_id: "provider-primary".to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "primary-model".to_string(),
        routing: Some(CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id: Some("queue-template-1".to_string()),
            queue_snapshot_id: Some("queue-snapshot-1".to_string()),
            queue_targets: vec![
                CompiledLlmRouteTarget {
                    provider_instance_id: "provider-primary".to_string(),
                    provider_instance_display_name: String::new(),
                    provider_code: "fixture_provider".to_string(),
                    protocol: "openai_compatible".to_string(),
                    upstream_model_id: "primary-model".to_string(),
                },
                CompiledLlmRouteTarget {
                    provider_instance_id: "provider-backup".to_string(),
                    provider_instance_display_name: String::new(),
                    provider_code: "fixture_provider".to_string(),
                    protocol: "openai_compatible".to_string(),
                    upstream_model_id: "backup-model".to_string(),
                },
            ],
            distribution_rule: LlmDistributionRule::RetryRoundRobin,
            distribution_key: None,
            context_policy: json!({}),
            stream_policy: json!({}),
        }),
    });
    let calls = Arc::new(Mutex::new(Vec::new()));
    let invoker = FailAfterTokenFinishErrorFailoverInvoker {
        calls: calls.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert_eq!(
        calls.lock().expect("calls mutex poisoned").as_slice(),
        ["provider-primary"]
    );
    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["error_code"],
                json!("provider_invalid_response")
            );
            assert_eq!(
                outcome.node_traces[1].output_payload["text"],
                failure.error_payload["message"]
            );
            assert_eq!(
                outcome.variable_pool["node-llm"]["text"],
                failure.error_payload["message"]
            );
            assert_eq!(
                outcome.node_traces[1].metrics_payload["attempts"][0]["failed_after_first_token"],
                json!(true)
            );
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}
