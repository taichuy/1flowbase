use super::*;

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
            distribution_rule: LlmDistributionRule::RetryRoundRobin,
            distribution_key: None,
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
async fn deterministic_upstream_400_does_not_consume_llm_node_retries() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(10);
    llm.config["retry_interval_ms"] = json!(0);

    let rejected = ProviderInvocationOutput {
        events: vec![ProviderStreamEvent::Error {
            error: ProviderRuntimeError {
                kind: ProviderRuntimeErrorKind::ProviderUpstreamError,
                message:
                    "The reasoning_content in the thinking mode must be passed back to the API."
                        .to_string(),
                provider_summary: Some("invalid request".to_string()),
                provider_details: Some(json!({
                    "status_code": 400,
                    "error_type": "invalid_request_error"
                })),
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
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.error_payload["status_code"], json!(400));
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_upstream_error")
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
async fn llm_node_retries_unknown_finish_reason_after_partial_text_without_exposing_it() {
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
                ProviderStreamEvent::TextDelta {
                    delta: "I will search for the function next.".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Unknown,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("I will search for the function next.".to_string()),
                finish_reason: Some(ProviderFinishReason::Unknown),
                provider_metadata: json!({
                    "stream_termination": {
                        "raw_finish_reason": "insufficient_system_resource",
                        "raw_finish_reason_state": "present",
                        "transport_termination": "done"
                    }
                }),
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
        json!("failed")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["error_code"],
        json!("provider_invalid_response")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["retry_reason"],
        json!("provider_invalid_response")
    );
}

#[tokio::test]
async fn unknown_finish_reason_fails_without_exposing_partial_text_or_losing_termination_evidence()
{
    let (invoker, captured_inputs) =
        sequential_tool_output_invoker(vec![ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "I will search for the function next.".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Unknown,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("I will search for the function next.".to_string()),
                finish_reason: Some(ProviderFinishReason::Unknown),
                provider_metadata: json!({
                    "stream_termination": {
                        "raw_finish_reason": "insufficient_system_resource",
                        "raw_finish_reason_status": "unrecognized",
                        "transport_termination": "done"
                    }
                }),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        }]);

    let outcome = start_flow_debug_run(
        &llm_answer_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert_eq!(
        captured_inputs.lock().expect("inputs mutex poisoned").len(),
        1
    );
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                "provider_invalid_response"
            );
            assert_eq!(
                failure.error_payload["stream_termination"]["raw_finish_reason"],
                "insufficient_system_resource"
            );
            assert_eq!(
                failure.error_payload["stream_termination"]["transport_termination"],
                "done"
            );
        }
        other => panic!("expected unknown finish reason failure, got {other:?}"),
    }
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert!(llm_trace.output_payload.get("text").is_none());
}

#[tokio::test]
async fn llm_node_retries_stop_with_reasoning_only_without_persisting_it_as_an_answer() {
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
                ProviderStreamEvent::ReasoningDelta {
                    delta: "I will inspect the implementation next.".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
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
        llm_trace.metrics_payload["attempts"][0]["error_code"],
        json!("provider_invalid_response")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["retry_reason"],
        json!("provider_invalid_response")
    );
}

#[tokio::test]
async fn stop_with_reasoning_only_fails_without_projecting_reasoning_as_answer() {
    let (invoker, captured_inputs) =
        sequential_tool_output_invoker(vec![ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::ReasoningDelta {
                    delta: "I will inspect the implementation next.".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        }]);

    let outcome = start_flow_debug_run(
        &llm_answer_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert_eq!(
        captured_inputs.lock().expect("inputs mutex poisoned").len(),
        1
    );
    match &outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_invalid_response")
            );
        }
        other => panic!("expected reasoning-only provider failure, got {other:?}"),
    }
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert!(llm_trace.output_payload.get("text").is_none());
    assert_eq!(
        llm_trace.debug_payload["provider_events"][0],
        json!({
            "type": "reasoning_delta",
            "delta": "I will inspect the implementation next."
        })
    );
}

#[tokio::test]
async fn llm_node_retry_consumes_output_protocol_failure_and_adds_model_feedback() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config["retry_enabled"] = json!(true);
    llm.config["max_retries"] = json!(1);
    llm.config["retry_interval_ms"] = json!(0);

    let retry_feedback = "Your previous tool-call markup was incomplete. Emit one complete tool call or answer normally.";
    let malformed = ProviderInvocationOutput {
        events: vec![
            ProviderStreamEvent::ReasoningDelta {
                delta: "I will call the tool".to_string(),
            },
            ProviderStreamEvent::TextDelta {
                delta: "visible prefix".to_string(),
            },
            ProviderStreamEvent::OutputProtocolFailure {
                failure: ProviderOutputProtocolFailure {
                    protocol: "dsml".to_string(),
                    error_code: "incomplete_envelope".to_string(),
                    message: "provider returned incomplete tool-call markup".to_string(),
                    retry_feedback: retry_feedback.to_string(),
                    provider_details: json!({"candidate": "</｜DSML｜tool_calls>"}),
                },
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            },
        ],
        result: ProviderInvocationResult {
            final_content: Some("visible prefix".to_string()),
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
        first_token_at: None,
        time_to_first_token_ms: None,
    };
    let (invoker, captured_inputs) = sequential_tool_output_invoker(vec![
        malformed,
        provider_output(final_llm_response("retry succeeded")),
    ]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start": {"query": "hello"}}), &invoker)
        .await
        .unwrap();
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    let inputs = captured_inputs.lock().expect("inputs mutex poisoned");

    assert_eq!(inputs.len(), 2);
    assert_eq!(
        inputs[1].messages.last().unwrap().role,
        ProviderMessageRole::User
    );
    assert_eq!(inputs[1].messages.last().unwrap().content, retry_feedback);
    assert_eq!(llm_trace.output_payload["text"], json!("retry succeeded"));
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["error_code"],
        json!("provider_output_protocol_failure")
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][0]["failed_after_first_token"],
        json!(true)
    );
    assert_eq!(
        llm_trace.metrics_payload["attempts"][1]["retry_reason"],
        json!("provider_output_protocol_failure")
    );
}

#[tokio::test]
async fn output_protocol_failure_without_node_retry_fails_without_exposing_partial_text() {
    let plan = base_plan();
    let malformed = ProviderInvocationOutput {
        events: vec![
            ProviderStreamEvent::TextDelta {
                delta: "visible prefix".to_string(),
            },
            ProviderStreamEvent::OutputProtocolFailure {
                failure: ProviderOutputProtocolFailure {
                    protocol: "dsml".to_string(),
                    error_code: "invalid_arguments".to_string(),
                    message: "provider returned invalid tool-call arguments".to_string(),
                    retry_feedback: "Emit valid JSON arguments.".to_string(),
                    provider_details: json!({}),
                },
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            },
        ],
        result: ProviderInvocationResult {
            final_content: Some("visible prefix".to_string()),
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
        first_token_at: None,
        time_to_first_token_ms: None,
    };
    let (invoker, captured_inputs) = sequential_tool_output_invoker(vec![malformed]);

    let outcome = start_flow_debug_run(&plan, &json!({"node-start": {"query": "hello"}}), &invoker)
        .await
        .expect("runtime should return failed outcome");

    assert_eq!(captured_inputs.lock().unwrap().len(), 1);
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_output_protocol_failure")
            );
            assert_eq!(failure.error_payload["protocol"], json!("dsml"));
            assert_eq!(
                failure.error_payload["protocol_error_code"],
                json!("invalid_arguments")
            );
        }
        other => panic!("expected output protocol failure, got {other:?}"),
    }
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert!(llm_trace.output_payload.get("text").is_none());
}
