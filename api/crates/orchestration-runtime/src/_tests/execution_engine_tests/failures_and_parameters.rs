use super::*;

struct FailClosedPipelineInvoker {
    provider_invocations: Arc<Mutex<usize>>,
}

struct ReceiptPipelineInvoker {
    provider: StubProviderInvoker,
}

#[async_trait]
impl ProviderInvoker for ReceiptPipelineInvoker {
    async fn pipeline_provider_input(
        &self,
        mut input: ProviderInvocationInput,
    ) -> std::result::Result<
        crate::provider_input_pipeline::ProviderInputPipelineOutput,
        crate::provider_input_pipeline::ProviderInputPipelineError,
    > {
        input
            .model_parameters
            .insert("pipeline_marker".to_string(), json!(true));
        input
            .run_context
            .insert("internal_compaction_policy".into(), json!({"enabled":true}));
        input
            .run_context
            .insert("internal_local_summary".into(), json!(true));
        Ok(
            crate::provider_input_pipeline::ProviderInputPipelineOutput {
                input,
                receipt: Some(crate::provider_input_pipeline::PipelineExecutionReceipt {
                    graph_fingerprint: "sha256:fixture-graph".to_string(),
                    point_id: crate::provider_input_pipeline::PROVIDER_INPUT_PIPELINE_POINT_ID
                        .to_string(),
                    before_digest: "sha256:before".to_string(),
                    after_digest: "sha256:after".to_string(),
                    receipt_digest: "sha256:receipt".to_string(),
                    contributions: vec![
                    crate::provider_input_pipeline::PipelineContributionReceipt {
                        contribution_id: "fixture.success".to_string(),
                        order: 0,
                        before_digest: "sha256:before".to_string(),
                        after_digest: "sha256:after".to_string(),
                        changed_fields: vec!["model_parameters".to_string()],
                        duration_micros: 1,
                        status:
                            crate::provider_input_pipeline::PipelineContributionStatus::Succeeded,
                        error: None,
                    },
                ],
                }),
            },
        )
    }

    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.provider.invoke_llm(runtime, input).await
    }
}

#[async_trait]
impl CapabilityInvoker for ReceiptPipelineInvoker {
    async fn invoke_capability_node(
        &self,
        runtime: &CompiledPluginRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        self.provider
            .invoke_capability_node(runtime, config_payload, input_payload)
            .await
    }
}

#[async_trait]
impl CodeInvoker for ReceiptPipelineInvoker {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        self.provider
            .invoke_code_node(runtime, config_payload, input_payload)
            .await
    }
}

#[async_trait]
impl ProviderInvoker for FailClosedPipelineInvoker {
    async fn pipeline_provider_input(
        &self,
        _input: ProviderInvocationInput,
    ) -> std::result::Result<
        crate::provider_input_pipeline::ProviderInputPipelineOutput,
        crate::provider_input_pipeline::ProviderInputPipelineError,
    > {
        Err(crate::provider_input_pipeline::ProviderInputPipelineError {
            code: "contribution_error",
            receipt: crate::provider_input_pipeline::PipelineExecutionReceipt {
                graph_fingerprint: "sha256:fixture-graph".to_string(),
                point_id: crate::provider_input_pipeline::PROVIDER_INPUT_PIPELINE_POINT_ID
                    .to_string(),
                before_digest: "sha256:before".to_string(),
                after_digest: "sha256:before".to_string(),
                receipt_digest: "sha256:receipt".to_string(),
                contributions: vec![
                    crate::provider_input_pipeline::PipelineContributionReceipt {
                        contribution_id: "fixture.fail-closed".to_string(),
                        order: 0,
                        before_digest: "sha256:before".to_string(),
                        after_digest: "sha256:before".to_string(),
                        changed_fields: Vec::new(),
                        duration_micros: 1,
                        status: crate::provider_input_pipeline::PipelineContributionStatus::Failed,
                        error: Some("contribution_error".to_string()),
                    },
                ],
            },
        })
    }

    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        *self
            .provider_invocations
            .lock()
            .expect("provider invocation counter mutex poisoned") += 1;
        unreachable!("fail-closed pipeline must stop before provider invocation")
    }
}

#[async_trait]
impl CapabilityInvoker for FailClosedPipelineInvoker {
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
impl CodeInvoker for FailClosedPipelineInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("fixture has no code nodes")
    }
}

#[tokio::test]
async fn provider_input_pipeline_failure_stops_before_provider_invocation() {
    let provider_invocations = Arc::new(Mutex::new(0));
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "fail closed" } }),
        &FailClosedPipelineInvoker {
            provider_invocations: Arc::clone(&provider_invocations),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        *provider_invocations
            .lock()
            .expect("provider invocation counter mutex poisoned"),
        0
    );
    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_input_pipeline_failed")
            );
            assert_eq!(
                failure.error_payload["pipeline_error"],
                json!("contribution_error")
            );
            assert_eq!(
                failure.error_payload["pipeline_receipt"]["contributions"][0]["status"],
                json!("failed")
            );
        }
        other => panic!("pipeline failure must fail the LLM node, got {other:?}"),
    }
}

#[tokio::test]
async fn successful_pipeline_receipt_and_rewritten_input_share_the_llm_invocation() {
    let captured_input = Arc::new(Mutex::new(None));
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "pipeline success" } }),
        &ReceiptPipelineInvoker {
            provider: StubProviderInvoker {
                fail: false,
                captured_input: Arc::clone(&captured_input),
                final_content: "ok".to_string(),
            },
        },
    )
    .await
    .unwrap();

    assert_eq!(
        captured_input
            .lock()
            .expect("captured input mutex poisoned")
            .as_ref()
            .unwrap()
            .model_parameters["pipeline_marker"],
        json!(true)
    );
    let captured = captured_input.lock().unwrap().clone().unwrap();
    assert_eq!(
        captured.run_context["internal_compaction_policy"]["enabled"], false,
        "input pipeline cannot grant node compaction permission"
    );
    assert!(!captured.run_context.contains_key("internal_local_summary"));
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["provider_input_pipeline"]
            ["receipt_digest"],
        json!("sha256:receipt")
    );
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["provider_input_pipeline"]
            ["contributions"][0]["changed_fields"],
        json!(["model_parameters"])
    );
}

#[tokio::test]
async fn provider_failure_preserves_the_runtime_error_message() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
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
            assert_eq!(failure.error_payload["error_code"], json!("auth_failed"));
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["error_code"],
                json!("auth_failed")
            );
            assert_eq!(failure.error_payload["message"], json!("invalid api_key"));
            assert!(outcome.node_traces[1].output_payload.get("text").is_none());
            assert!(!outcome.variable_pool.contains_key("node-llm"));
            assert!(failure.error_payload.get("provider_summary").is_none());
            assert!(failure.error_payload.get("provider_details").is_none());
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[test]
fn issue_1743_semantic_provider_details_copy_only_bounded_projection_allowlist() {
    let runtime = base_plan().nodes["node-llm"]
        .llm_runtime
        .as_ref()
        .expect("fixture LLM runtime must exist")
        .clone();
    let error = ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::SemanticCapabilityUnsupported,
        "semantic route rejected",
    )
    .with_provider_details(json!({
        "route_id": "llm_route",
        "missing_capabilities": ["message_blocks.reasoning_history.v1"],
        "projection": {
            "cause_count": 1,
            "causes_capped": false,
            "causes": [{
                "cause": "unsupported",
                "error_code": "reasoning_only_message_unsupported",
                "block": {"message_index": 2, "block_index": 3, "block_kind": "reasoning"},
                "raw_body": "RAW_BODY_CANARY",
                "signature": "SIGNATURE_CANARY",
                "cursor": "CURSOR_CANARY"
            }]
        },
        "provider_raw_body": "UPSTREAM_RAW_CANARY"
    }));

    let payload =
        crate::execution_engine::llm_final_content::build_provider_error_payload(&runtime, &error);

    assert_eq!(payload["route_id"], json!("llm_route"));
    assert_eq!(
        payload["projection"]["causes"][0]["error_code"],
        json!("reasoning_only_message_unsupported")
    );
    assert_eq!(
        payload["projection"]["causes"][0]["block"],
        json!({"message_index": 2, "block_index": 3, "block_kind": "reasoning"})
    );
    assert!(payload.get("provider_details").is_none());
    let encoded = payload.to_string();
    for canary in [
        "RAW_BODY_CANARY",
        "SIGNATURE_CANARY",
        "CURSOR_CANARY",
        "UPSTREAM_RAW_CANARY",
    ] {
        assert!(!encoded.contains(canary));
    }
}

#[tokio::test]
async fn provider_upstream_error_body_is_the_durable_message_and_event_message() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &ProviderUpstreamErrorInvoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_upstream_error")
            );
            assert_eq!(
                failure.error_payload["message"],
                json!(PROVIDER_UPSTREAM_ERROR_BODY)
            );
            assert_eq!(failure.error_payload["status_code"], json!(400));
            assert!(failure.error_payload.get("provider_summary").is_none());
            assert_eq!(
                failure.error_payload["provider_details"]["raw_body"],
                PROVIDER_UPSTREAM_ERROR_BODY
            );
            assert_eq!(
                failure.error_payload["provider_details"]["upstream_error"],
                json!({"message":"first line\nsecond line", "code":"future_code", "type":null,"extra":{"opaque":[null,1]}})
            );
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref(),
                Some(&failure.error_payload)
            );
            assert!(failure
                .error_payload
                .to_string()
                .contains("keep complete body"));
            let provider_error = outcome.node_traces[1]
                .provider_events
                .iter()
                .find_map(|event| match event {
                    ProviderStreamEvent::Error { error } => Some(error),
                    _ => None,
                })
                .expect("durable provider events should retain the upstream error");
            assert_eq!(provider_error.message, PROVIDER_UPSTREAM_ERROR_BODY);
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[tokio::test]
async fn d1_ac_008_provider_runtime_contract_error_stays_out_of_llm_output() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &RuntimeContractErrorInvoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(failure.error_payload["error_code"], json!("auth_failed"));
            assert_eq!(
                failure.error_payload["message"],
                json!("401 401 Unauthorized: Incorrect API key provided")
            );
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["message"],
                json!("401 401 Unauthorized: Incorrect API key provided")
            );
            assert!(outcome.node_traces[1].output_payload.get("text").is_none());
            assert!(!outcome.variable_pool.contains_key("node-llm"));
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[tokio::test]
async fn invalid_provider_contract_message_reaches_durable_and_client_failure_projection() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &InvalidProviderContractInvoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_invalid_response")
            );
            assert_eq!(
                failure.error_payload["message"],
                json!(INVALID_PROVIDER_CONTRACT_DISPLAY)
            );
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["message"],
                json!(INVALID_PROVIDER_CONTRACT_DISPLAY)
            );
            assert_eq!(
                outcome.node_traces[1].metrics_payload["attempts"][0]["error_message_ref"],
                json!(format!(
                    "runtime_artifact:inline:error:{INVALID_PROVIDER_CONTRACT_DISPLAY}"
                ))
            );
            assert!(outcome.node_traces[1].output_payload.get("text").is_none());
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[tokio::test]
async fn d1_ac_008_partial_delta_remains_separate_from_failed_output() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &FailsAfterFirstTokenInvoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_invalid_response")
            );
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["error_code"],
                json!("provider_invalid_response")
            );
            assert!(outcome.node_traces[1].output_payload.get("text").is_none());
            assert!(!outcome.variable_pool.contains_key("node-llm"));
            assert_eq!(
                outcome.node_traces[1].provider_events[0],
                ProviderStreamEvent::TextDelta {
                    delta: "partial answer".to_string()
                }
            );
            assert_eq!(
                outcome.node_traces[1].debug_payload["provider_events"][0],
                json!({ "type": "text_delta", "delta": "partial answer" })
            );
            assert_eq!(
                outcome.node_traces[1].metrics_payload["attempts"][0]["failed_after_first_token"],
                json!(true)
            );
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[tokio::test]
async fn event_only_finish_error_is_classified_as_terminal_provider_failure() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &EventOnlyFinishErrorInvoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("provider_invalid_response")
            );
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["error_code"],
                json!("provider_invalid_response")
            );
            assert_eq!(
                outcome.node_traces[1].output_payload["text"],
                json!("provider invocation finished with error")
            );
        }
        other => panic!("expected terminal provider failure, got {other:?}"),
    }
}

#[tokio::test]
async fn d1_ac_007_output_limit_has_an_incomplete_terminal_not_succeeded() {
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![ProviderInvocationResult {
        final_content: Some("partial response at output limit".to_string()),
        finish_reason: Some(ProviderFinishReason::Length),
        ..ProviderInvocationResult::default()
    }]);

    let outcome = start_flow_debug_run(
        &llm_answer_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Incomplete(_)
    ));
}

#[path = "failures_and_parameters/model_parameters.rs"]
mod model_parameters;
mod protocol_context;
