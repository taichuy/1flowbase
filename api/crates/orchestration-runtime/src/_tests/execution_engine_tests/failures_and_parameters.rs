use super::*;

use anyhow::anyhow;

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
            assert!(failure.error_payload.get("provider_details").is_none());
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
            assert_eq!(failure.error_payload["message"], json!("invalid api_key"));
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["message"],
                json!("invalid api_key")
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
                outcome.node_traces[1].metrics_payload["attempts"][0]["error_payload"]["message"],
                json!(INVALID_PROVIDER_CONTRACT_DISPLAY)
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

#[tokio::test]
async fn d1_ac_010_legacy_max_tokens_is_rejected_before_provider_invocation() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "llm_parameters": {
            "schema_version": "1.0.0",
            "items": {
                "max_tokens": { "enabled": true, "value": 8192 }
            }
        }
    });
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::clone(&captured_input),
        final_content: "should not be invoked".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("unsupported_model_parameter")
            );
            assert_eq!(
                failure.error_payload["field"],
                json!("llm_parameters.items.max_tokens")
            );
        }
        other => panic!("legacy max_tokens must fail before provider invocation, got {other:?}"),
    }
    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
}

#[tokio::test]
async fn llm_runtime_sends_enabled_model_parameters_and_keeps_undeclared_structured_output_private()
{
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "model_provider": {
            "provider_instance_id": "provider-ready",
            "model_id": "gpt-5.4-mini"
        },
        "llm_parameters": {
            "schema_version": "1.0.0",
            "items": {
                "temperature": { "enabled": true, "value": 0.7 },
                "top_p": { "enabled": false, "value": 0.9 }
            }
        },
        "response_format": {
            "mode": "json_schema",
            "schema": { "type": "object" }
        }
    });

    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "{\"ok\":true}".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "输出 JSON" } }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input.model_parameters.get("temperature"),
        Some(&json!(0.7))
    );
    assert!(!captured_input.model_parameters.contains_key("top_p"));
    assert_eq!(
        captured_input.response_format,
        Some(json!({ "mode": "json_schema", "schema": { "type": "object" } }))
    );
    assert_eq!(
        outcome.node_traces[1].output_payload["text"],
        json!("{\"ok\":true}")
    );
    assert!(outcome.node_traces[1]
        .output_payload
        .get("structured_output")
        .is_none());
}

fn protocol_context_fixture() -> ProtocolContextEnvelope {
    ProtocolContextEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        query: BTreeMap::from([(
            "preview".to_string(),
            vec!["one".to_string(), "two".to_string()],
        )]),
        headers: BTreeMap::from([(
            "anthropic-beta".to_string(),
            vec!["prompt-caching".to_string(), "private-beta".to_string()],
        )]),
        body: BTreeMap::from([(
            "context_management".to_string(),
            json!({ "edits": [{ "type": "clear_thinking_20251015" }] }),
        )]),
    }
}

#[tokio::test]
async fn wp_d1b_start_protocol_context_reference_reaches_only_the_provider_invocation() {
    let mut plan = base_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("llm node should exist")
        .config["protocol_context"] = json!({
        "kind": "selector",
        "value": ["sys", "protocol_context"]
    });
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };
    let protocol_context = protocol_context_fixture();
    let runtime_context =
        ExecutionRuntimeContext::default().with_protocol_context(protocol_context.clone());
    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({
            "__client_protocol_envelope": {
                "must_not": "become durable runtime state"
            },
            "node-start": { "query": "退款政策" },
            "sys": {
                "conversation_id": "conversation-1",
                "protocol_context": {
                    "must_not": "become a Start payload"
                }
            }
        }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    let envelope = captured_input
        .client_protocol_envelope
        .expect("selected protocol context should be forwarded");

    assert_eq!(envelope, protocol_context);
    assert!(captured_input.required_capabilities.contains(
        &plugin_framework::provider_contract::ProviderInvocationCapability::ProtocolContext
    ));
    assert!(captured_input
        .run_context
        .get("resolved_inputs")
        .and_then(|value| value.get("__client_protocol_envelope"))
        .is_none());
    assert!(outcome.node_traces[0].input_payload["sys"]
        .get("protocol_context")
        .is_none());
    assert!(outcome
        .variable_pool
        .get("__client_protocol_envelope")
        .is_none());
    assert!(outcome.variable_pool["sys"]
        .get("protocol_context")
        .is_none());
}

#[tokio::test]
async fn wp_d1c_start_exposes_only_the_safe_locator_while_llm_receives_the_raw_context() {
    let mut plan = base_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("llm node should exist")
        .config["protocol_context"] = json!({
        "kind": "selector",
        "value": ["sys", "protocol_context"]
    });
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };
    let protocol_context = protocol_context_fixture();
    let locator = json!({
        "__test_ephemeral_protocol_context": {
            "storage": "ephemeral",
            "digest": "sha256:test"
        }
    });
    let runtime_context = ExecutionRuntimeContext::default()
        .with_ephemeral_protocol_context(locator.clone(), protocol_context.clone());

    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({
            "node-start": { "query": "退款政策" },
            "sys": { "conversation_id": "conversation-1" }
        }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();

    let captured = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(captured.client_protocol_envelope, Some(protocol_context));
    assert_eq!(
        outcome.node_traces[0].input_payload["sys"]["protocol_context"],
        locator
    );
    assert!(!outcome.node_traces[0]
        .input_payload
        .to_string()
        .contains("private-beta"));
    assert!(!Value::Object(outcome.variable_pool)
        .to_string()
        .contains("private-beta"));
}

#[tokio::test]
async fn wp_d1c_missing_original_protocol_context_slot_fails_at_the_selected_llm() {
    let mut plan = base_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("llm node should exist")
        .config["protocol_context"] = json!({
        "kind": "selector",
        "value": ["sys", "protocol_context"]
    });
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::clone(&captured_input),
        final_content: "must not be invoked".to_string(),
    };
    let locator = json!({"__test_ephemeral_protocol_context": true});
    let runtime_context = ExecutionRuntimeContext::default()
        .with_unavailable_ephemeral_protocol_context(
            locator.clone(),
            "ephemeral_protocol_context_missing",
        );

    let outcome = start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "退款政策" } }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
    assert_eq!(
        outcome.node_traces[0].input_payload["sys"]["protocol_context"],
        locator
    );
    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => assert_eq!(
            failure.error_payload["runtime_message"],
            json!("ephemeral_protocol_context_missing")
        ),
        other => panic!("missing original slot must fail explicitly, got {other:?}"),
    }
}

#[tokio::test]
async fn wp_d1b_null_protocol_context_reference_disables_forwarding() {
    let mut plan = base_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("llm node should exist")
        .config["protocol_context"] = Value::Null;
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };
    let runtime_context = ExecutionRuntimeContext::default()
        .with_protocol_context(protocol_context_fixture())
        .with_provider_invocation_capability(
            plugin_framework::provider_contract::ProviderInvocationCapability::ProtocolContext,
        );
    start_flow_debug_run_with_runtime_context(
        &plan,
        &json!({ "node-start": { "query": "退款政策" } }),
        runtime_context,
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert!(captured_input.client_protocol_envelope.is_none());
    assert!(!captured_input.required_capabilities.contains(
        &plugin_framework::provider_contract::ProviderInvocationCapability::ProtocolContext
    ));
}

#[tokio::test]
async fn wp_d1b_code_json_variable_becomes_the_invocation_protocol_context() {
    let protocol_context = protocol_context_fixture();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = ProtocolContextCodeInvoker {
        provider: StubProviderInvoker {
            fail: false,
            captured_input: Arc::clone(&captured_input),
            final_content: "ok".to_string(),
        },
        code_output: json!({
            "protocol_context": serde_json::to_value(&protocol_context).unwrap()
        }),
        protected_protocol_context: Arc::new(Mutex::new(None)),
        protocol_context_missing: false,
    };

    let outcome = start_flow_debug_run(
        &code_protocol_context_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(
        captured_input.client_protocol_envelope,
        Some(protocol_context)
    );
    assert!(captured_input.required_capabilities.contains(
        &plugin_framework::provider_contract::ProviderInvocationCapability::ProtocolContext
    ));
    assert!(outcome.node_traces[0]
        .input_payload
        .get("protocol_context")
        .is_none());
    let durable_outcome = json!({
        "variable_pool": outcome.variable_pool.clone(),
        "node_payloads": outcome.node_traces.iter().map(|trace| json!({
            "input": trace.input_payload,
            "output": trace.output_payload,
            "debug": trace.debug_payload,
        })).collect::<Vec<_>>()
    })
    .to_string();
    assert!(durable_outcome.contains("__test_ephemeral_protocol_context"));
    assert!(!durable_outcome.contains("private-beta"));
}

#[tokio::test]
async fn wp_d1b_invalid_code_json_fails_before_provider_invocation() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = ProtocolContextCodeInvoker {
        provider: StubProviderInvoker {
            fail: false,
            captured_input: Arc::clone(&captured_input),
            final_content: "must not be invoked".to_string(),
        },
        code_output: json!({
            "protocol_context": {
                "source_protocol": "anthropic_messages",
                "headers": { "anthropic-beta": "must be an array" }
            }
        }),
        protected_protocol_context: Arc::new(Mutex::new(None)),
        protocol_context_missing: false,
    };

    let outcome = start_flow_debug_run(
        &code_protocol_context_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => assert_eq!(
            failure.error_payload["error_code"],
            json!("protocol_context_resolution_failed")
        ),
        other => panic!("invalid protocol context must fail before invocation, got {other:?}"),
    }
    let durable_payloads = outcome
        .node_traces
        .iter()
        .map(|trace| {
            json!({
                "input": trace.input_payload,
                "output": trace.output_payload,
                "debug": trace.debug_payload,
            })
        })
        .collect::<Vec<_>>();
    assert!(!Value::Array(durable_payloads)
        .to_string()
        .contains("must be an array"));
}

#[tokio::test]
async fn wp_d1c_missing_selected_code_protocol_context_slot_fails_explicitly() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = ProtocolContextCodeInvoker {
        provider: StubProviderInvoker {
            fail: false,
            captured_input: Arc::clone(&captured_input),
            final_content: "must not be invoked".to_string(),
        },
        code_output: json!({
            "protocol_context": serde_json::to_value(protocol_context_fixture()).unwrap()
        }),
        protected_protocol_context: Arc::new(Mutex::new(None)),
        protocol_context_missing: true,
    };

    let outcome = start_flow_debug_run(
        &code_protocol_context_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => assert_eq!(
            failure.error_payload["runtime_message"],
            json!("ephemeral_protocol_context_missing")
        ),
        other => panic!("missing protocol context slot must fail explicitly, got {other:?}"),
    }
    let durable_payloads = outcome
        .node_traces
        .iter()
        .map(|trace| {
            json!({
                "input": trace.input_payload,
                "output": trace.output_payload,
                "debug": trace.debug_payload,
            })
        })
        .collect::<Vec<_>>();
    assert!(!Value::Array(durable_payloads)
        .to_string()
        .contains("private-beta"));
}

struct ProtocolContextCodeInvoker {
    provider: StubProviderInvoker,
    code_output: Value,
    protected_protocol_context: Arc<Mutex<Option<Value>>>,
    protocol_context_missing: bool,
}

#[async_trait]
impl ProviderInvoker for ProtocolContextCodeInvoker {
    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.provider.invoke_llm(runtime, input).await
    }

    async fn resolve_protocol_context_locator(&self, locator: &Value) -> Result<Option<Value>> {
        if locator
            .get("__test_ephemeral_protocol_context")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Ok(None);
        }
        if self.protocol_context_missing {
            return Err(anyhow!("ephemeral_protocol_context_missing"));
        }
        Ok(self
            .protected_protocol_context
            .lock()
            .expect("protected protocol context mutex poisoned")
            .clone())
    }
}

#[async_trait]
impl CapabilityInvoker for ProtocolContextCodeInvoker {
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
impl CodeInvoker for ProtocolContextCodeInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        Ok(CodeInvocationOutput {
            output_payload: self.code_output.clone(),
            console_logs: Vec::new(),
        })
    }

    async fn protect_protocol_context_output(
        &self,
        output: &mut CodeInvocationOutput,
        selected_output_paths: &[Vec<String>],
    ) -> Result<()> {
        assert_eq!(
            selected_output_paths,
            &[vec!["protocol_context".to_string()]]
        );
        let raw = output
            .output_payload
            .get_mut("protocol_context")
            .map(std::mem::take)
            .expect("selected Code protocol context should exist");
        *self
            .protected_protocol_context
            .lock()
            .expect("protected protocol context mutex poisoned") = Some(raw);
        output.output_payload["protocol_context"] =
            json!({"__test_ephemeral_protocol_context": true});
        Ok(())
    }
}

fn code_protocol_context_plan() -> CompiledPlan {
    let mut plan = base_plan();
    plan.topological_order.insert(1, "node-code".to_string());
    plan.nodes
        .get_mut("node-start")
        .expect("start node should exist")
        .downstream_node_ids = vec!["node-code".to_string()];
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.dependency_node_ids = vec!["node-code".to_string()];
    llm.config["protocol_context"] = json!({
        "kind": "selector",
        "value": ["node-code", "result", "protocol_context"]
    });
    plan.nodes.insert(
        "node-code".to_string(),
        CompiledNode {
            node_id: "node-code".to_string(),
            node_type: "code".to_string(),
            alias: "Protocol Context".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-llm".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "protocol_context".to_string(),
                title: "Protocol Context".to_string(),
                value_type: "json".to_string(),
                selector: vec!["result".to_string(), "protocol_context".to_string()],
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: Some(CompiledCodeRuntime {
                language: "javascript".to_string(),
                source: None,
                source_ref: None,
                entrypoint: "main".to_string(),
                imports: Vec::new(),
                dependencies: Vec::new(),
                isolation_profile: CodeIsolationProfile::quickjs_default(),
            }),
        },
    );
    plan
}

#[tokio::test]
async fn llm_runtime_ignores_external_reasoning_parameters_without_node_opt_in() {
    let plan = base_plan();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "reasoning": {
                        "mode": "adaptive",
                        "effort": "high",
                        "budget_tokens": 4096
                    }
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert!(!captured_input.model_parameters.contains_key("reasoning"));
    assert!(!captured_input
        .model_parameters
        .contains_key("thinking_budget_tokens"));
}

#[tokio::test]
async fn llm_runtime_preserves_typed_external_reasoning_for_openai_runtime() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "external_reasoning_policy": {
            "follow_external_reasoning": true
        }
    });
    let runtime = llm.llm_runtime.as_mut().expect("llm runtime should exist");
    runtime.provider_code = "openai".to_string();
    runtime.protocol = "openai_responses".to_string();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "reasoning": {
                        "mode": "adaptive",
                        "effort": "high",
                        "budget_tokens": 4096
                    }
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input.model_parameters.get("reasoning"),
        Some(&json!({
            "mode": "adaptive",
            "effort": "high",
            "budget_tokens": 4096
        }))
    );
    assert!(!captured_input
        .model_parameters
        .contains_key("reasoning_effort"));
}

#[tokio::test]
async fn llm_runtime_preserves_typed_external_reasoning_for_openai_compatible_runtime() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "external_reasoning_policy": { "follow_external_reasoning": true }
    });
    let runtime = llm.llm_runtime.as_mut().expect("llm runtime should exist");
    runtime.provider_code = "openai_compatible".to_string();
    runtime.protocol = "openai_compatible".to_string();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": { "model_parameters": { "reasoning": {
                "mode": "enabled", "effort": "medium", "budget_tokens": 2048
            } } }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(
        captured_input.model_parameters.get("reasoning"),
        Some(&json!({ "mode": "enabled", "effort": "medium", "budget_tokens": 2048 }))
    );
    assert!(!captured_input
        .model_parameters
        .contains_key("reasoning_effort"));
}

#[tokio::test]
async fn llm_runtime_preserves_typed_external_reasoning_for_anthropic_runtime() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "external_reasoning_policy": {
            "follow_external_reasoning": true
        }
    });
    let runtime = llm.llm_runtime.as_mut().expect("llm runtime should exist");
    runtime.provider_code = "anthropic".to_string();
    runtime.protocol = "anthropic_messages".to_string();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "reasoning": {
                        "mode": "adaptive",
                        "effort": "high",
                        "budget_tokens": 4096
                    }
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input.model_parameters.get("reasoning"),
        Some(&json!({
            "mode": "adaptive",
            "effort": "high",
            "budget_tokens": 4096
        }))
    );
    assert!(!captured_input
        .model_parameters
        .contains_key("thinking_type"));
    assert!(!captured_input
        .model_parameters
        .contains_key("thinking_budget_tokens"));
    assert!(!captured_input
        .model_parameters
        .contains_key("reasoning_effort"));
}

#[tokio::test]
async fn llm_runtime_maps_external_reasoning_parameters_for_bailian_runtime() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "external_reasoning_policy": {
            "follow_external_reasoning": true
        }
    });
    let runtime = llm.llm_runtime.as_mut().expect("llm runtime should exist");
    runtime.provider_code = "aliyun_bailian".to_string();
    runtime.protocol = "openai_compatible".to_string();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "reasoning": {
                        "mode": "enabled",
                        "effort": "high",
                        "budget_tokens": 4096
                    }
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input.model_parameters.get("enable_thinking"),
        Some(&json!(true))
    );
    assert_eq!(
        captured_input.model_parameters.get("reasoning_effort"),
        Some(&json!("high"))
    );
    assert!(!captured_input
        .model_parameters
        .contains_key("thinking_budget_tokens"));
}

#[tokio::test]
async fn ac_005_llm_runtime_follows_external_max_output_tokens_by_default() {
    let plan = base_plan();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "max_output_tokens": 32768
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input.model_parameters.get("max_output_tokens"),
        Some(&json!(32768))
    );
    assert!(!captured_input.model_parameters.contains_key("max_tokens"));
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["effective_max_output_tokens"],
        json!(32768)
    );
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["max_output_tokens_source"],
        json!("external_request")
    );
}

#[tokio::test]
async fn ac_005_llm_runtime_preserves_external_requested_context_window() {
    let plan = base_plan();
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "requested_context_window": 1_000_000
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input
            .model_parameters
            .get("requested_context_window"),
        Some(&json!(1_000_000))
    );
}

#[tokio::test]
async fn ac_005_llm_runtime_can_disable_external_max_output_tokens_and_trace_provider_default() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "external_model_parameter_policy": {
            "follow_external_max_output_tokens": false
        }
    });
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![ProviderInvocationResult {
        final_content: Some("ok".to_string()),
        finish_reason: Some(ProviderFinishReason::Stop),
        provider_metadata: json!({
            "effective_max_output_tokens": 4096
        }),
        ..ProviderInvocationResult::default()
    }]);

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "max_output_tokens": 32768
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_inputs = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned");
    assert!(!captured_inputs[0]
        .model_parameters
        .contains_key("max_output_tokens"));
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["effective_max_output_tokens"],
        json!(4096)
    );
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["max_output_tokens_source"],
        json!("provider_default")
    );
}

#[tokio::test]
async fn ac_006_llm_runtime_keeps_enabled_node_max_output_tokens_over_external_limit() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "llm_parameters": {
            "schema_version": "1.0.0",
            "items": {
                "max_output_tokens": { "enabled": true, "value": 8192 }
            }
        },
        "external_model_parameter_policy": {
            "follow_external_max_output_tokens": true
        }
    });
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "ok".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": { "query": "hello" },
            "sys": {
                "model_parameters": {
                    "max_output_tokens": 32768
                }
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    let captured_input = invoker
        .captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        captured_input.model_parameters.get("max_output_tokens"),
        Some(&json!(8192))
    );
    assert!(!captured_input.model_parameters.contains_key("max_tokens"));
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["effective_max_output_tokens"],
        json!(8192)
    );
    assert_eq!(
        outcome.node_traces[1].debug_payload["llm_context"]["max_output_tokens_source"],
        json!("llm_node")
    );
}

#[tokio::test]
async fn llm_json_schema_response_exposes_structured_output_only_when_declared() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "model_provider": {
            "provider_instance_id": "provider-ready",
            "model_id": "gpt-5.4-mini"
        },
        "response_format": {
            "mode": "json_schema",
            "schema": { "type": "object" }
        }
    });
    llm.outputs.push(CompiledOutput {
        key: "structured_output".to_string(),
        title: "结构化输出".to_string(),
        value_type: "json".to_string(),
        selector: Vec::new(),
        json_schema: None,
    });

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "输出 JSON" } }),
        &StubProviderInvoker {
            fail: false,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: "{\"ok\":true}".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.node_traces[1].output_payload["text"],
        json!("{\"ok\":true}")
    );
    assert_eq!(
        outcome.node_traces[1].output_payload["structured_output"],
        json!({ "ok": true })
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["usage"]["total_tokens"],
        json!(12)
    );
}

#[tokio::test]
async fn llm_json_schema_response_rejects_invalid_structured_output() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.config = json!({
        "model_provider": {
            "provider_instance_id": "provider-ready",
            "model_id": "gpt-5.4-mini"
        },
        "response_format": {
            "mode": "json_schema",
            "schema": { "type": "object" }
        }
    });
    llm.outputs.push(CompiledOutput {
        key: "structured_output".to_string(),
        title: "结构化输出".to_string(),
        value_type: "json".to_string(),
        selector: Vec::new(),
        json_schema: None,
    });

    let error = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "输出 JSON" } }),
        &StubProviderInvoker {
            fail: false,
            captured_input: Arc::new(Mutex::new(None)),
            final_content: "not json".to_string(),
        },
    )
    .await
    .expect_err("invalid structured LLM output should fail the node");

    assert!(error.to_string().contains("invalid structured LLM output"));
}
#[tokio::test]
async fn generate_llm_consumer_rejects_non_generate_operations_before_provider_invocation() {
    for operation in [
        json!({"kind": "count_tokens", "profile": null}),
        json!({"kind": "compact", "profile": "responses_compact"}),
    ] {
        let invoker = successful_invoker();
        let captured = invoker.captured_input.clone();
        let outcome = start_flow_debug_run(
            &base_plan(),
            &json!({
                "node-start": {
                    "query": "unsupported operation",
                    "operation": operation
                }
            }),
            &invoker,
        )
        .await
        .unwrap();

        let llm_trace = outcome
            .node_traces
            .iter()
            .find(|trace| trace.node_id == "node-llm")
            .expect("LLM consumer must emit a typed failure trace");
        assert_eq!(
            llm_trace.error_payload.as_ref().unwrap()["error_code"],
            "ai_native_operation_unsupported"
        );
        assert!(captured.lock().expect("input mutex poisoned").is_none());
    }
}
