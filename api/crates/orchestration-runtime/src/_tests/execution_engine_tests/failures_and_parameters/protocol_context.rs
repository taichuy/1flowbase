use super::*;

fn protocol_context_fixture() -> ProtocolContextEnvelope {
    ProtocolContextEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        source_request: Some(SourceProtocolRequest {
            authentication: Some(ProtocolAuthenticationPresentation::AuthorizationBearer),
            body: Some(json!({
                "model": "claude-opus-4-8",
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "text",
                        "text": "SOURCE-BODY-EPHEMERAL-CANARY",
                        "cache_control": {"type": "ephemeral"}
                    }]
                }]
            })),
        }),
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
async fn ac_001_start_protocol_context_is_normal_downstream_workflow_data() {
    let mut plan = base_plan();
    plan.nodes
        .get_mut("node-llm")
        .expect("llm node should exist")
        .config["protocol_context"] = json!({
        "kind": "selector",
        "value": ["node-start", "protocol_context"]
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
        &extension_contracts::provider_contract::ProviderInvocationCapability::ProtocolContext
    ));
    assert!(captured_input
        .run_context
        .get("resolved_inputs")
        .and_then(|value| value.get("__client_protocol_envelope"))
        .is_none());
    assert_eq!(
        outcome.node_traces[0].input_payload["protocol_context"],
        serde_json::to_value(&protocol_context).unwrap()
    );
    assert!(outcome
        .variable_pool
        .get("__client_protocol_envelope")
        .is_none());
    assert!(outcome.variable_pool["sys"]
        .get("protocol_context")
        .is_none());
    assert_eq!(
        outcome.variable_pool["node-start"]["protocol_context"],
        serde_json::to_value(&protocol_context).unwrap()
    );
}

#[tokio::test]
async fn ac_001_legacy_locator_and_normal_start_variable_can_coexist() {
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
    assert_eq!(
        captured.client_protocol_envelope,
        Some(protocol_context.clone())
    );
    assert_eq!(
        outcome.node_traces[0].input_payload["sys"]["protocol_context"],
        locator
    );
    assert_eq!(
        outcome.node_traces[0].input_payload["protocol_context"],
        serde_json::to_value(&protocol_context).unwrap()
    );
    assert_eq!(
        outcome.variable_pool["node-start"]["protocol_context"],
        serde_json::to_value(&protocol_context).unwrap()
    );
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
            extension_contracts::provider_contract::ProviderInvocationCapability::ProtocolContext,
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
        &extension_contracts::provider_contract::ProviderInvocationCapability::ProtocolContext
    ));
}

#[tokio::test]
async fn ac_004_typed_code_variable_remains_normal_data_and_reaches_the_provider() {
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
        resolved_protocol_context: None,
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
        &extension_contracts::provider_contract::ProviderInvocationCapability::ProtocolContext
    ));
    assert_eq!(
        outcome.node_traces[0].input_payload["protocol_context"],
        Value::Null
    );
    let durable_outcome = json!({
        "variable_pool": outcome.variable_pool.clone(),
        "node_payloads": outcome.node_traces.iter().map(|trace| json!({
            "input": trace.input_payload,
            "output": trace.output_payload,
            "debug": trace.debug_payload,
        })).collect::<Vec<_>>()
    })
    .to_string();
    assert!(!durable_outcome.contains("__test_ephemeral_protocol_context"));
    assert!(durable_outcome.contains("private-beta"));
}

#[tokio::test]
async fn ac_004_invalid_typed_code_value_fails_at_the_producer() {
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
        resolved_protocol_context: None,
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
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-code");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("protocol_context_contract_violation")
            );
            assert_eq!(
                failure.error_payload["selector"],
                json!("node-code.result.protocol_context")
            );
            assert_eq!(
                failure.error_payload["instance_path"],
                json!("/headers/anthropic-beta")
            );
        }
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
    assert!(Value::Array(durable_payloads)
        .to_string()
        .contains("must be an array"));
}

#[tokio::test]
async fn ac_005_llm_final_protocol_context_defense_skips_provider_invocation() {
    let mut plan = code_protocol_context_plan();
    plan.nodes
        .get_mut("node-code")
        .expect("code node should exist")
        .outputs[0]
        .value_type = "json".to_string();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = ProtocolContextCodeInvoker {
        provider: StubProviderInvoker {
            fail: false,
            captured_input: Arc::clone(&captured_input),
            final_content: "must not be invoked".to_string(),
        },
        code_output: json!({
            "protocol_context": {"legacy": "raw value is sealed by the invoker"}
        }),
        resolved_protocol_context: Some(json!({
            "source_protocol": "anthropic_messages",
            "headers": {"anthropic-beta": "must be an array"}
        })),
    };
    let outcome = start_flow_debug_run(
        &plan,
        &json!({"node-start": {"query": "退款政策"}}),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("protocol_context_resolution_failed")
            );
            assert!(failure.error_payload["runtime_message"]
                .as_str()
                .is_some_and(|message| message.contains("/headers/anthropic-beta")));
        }
        other => panic!("invalid protocol context must fail at the LLM defense, got {other:?}"),
    }
}

struct ProtocolContextCodeInvoker {
    provider: StubProviderInvoker,
    code_output: Value,
    resolved_protocol_context: Option<Value>,
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

    async fn resolve_protocol_context_locator(&self, _locator: &Value) -> Result<Option<Value>> {
        Ok(self.resolved_protocol_context.clone())
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
        if !selected_output_paths.is_empty() {
            output.output_payload["protocol_context"] =
                json!({"__test_ephemeral_protocol_context": true});
        }
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
                value_type: "protocol_context".to_string(),
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

