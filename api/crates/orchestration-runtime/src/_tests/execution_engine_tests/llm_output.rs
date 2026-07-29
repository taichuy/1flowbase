use super::*;

#[tokio::test]
async fn llm_node_success_keeps_processed_result_fields_in_output_payload() {
    let trace = run_llm_trace_with_fixture_provider().await;

    assert_eq!(trace.output_payload["text"], json!("echo:gpt-5.4-mini"));
    assert_eq!(
        trace.output_payload["provider_route"]["provider_instance_id"],
        "provider-ready"
    );
    assert_eq!(
        trace.output_payload["provider_route"]["provider_code"],
        "fixture_provider"
    );
    assert_eq!(
        trace.output_payload["provider_route"]["protocol"],
        "openai_compatible"
    );
    assert_eq!(
        trace.output_payload["provider_route"]["model"],
        "gpt-5.4-mini"
    );
    assert_eq!(trace.output_payload["finish_reason"], json!("stop"));
    assert_eq!(trace.output_payload["usage"]["input_tokens"], json!(5));
    assert_eq!(trace.output_payload["usage"]["output_tokens"], json!(7));
    assert_eq!(trace.output_payload["usage"]["total_tokens"], json!(12));
    assert!(trace.output_payload.get("route").is_none());
    assert!(trace.output_payload.get("attempts").is_none());
    assert!(trace.output_payload.get("assistant_message").is_none());
    assert!(trace.output_payload.get("raw_response_ref").is_none());
    assert!(trace.output_payload.get("context_projection_ref").is_none());
    assert!(trace.output_payload.get("attempt_refs").is_none());
    assert!(trace.output_payload.get("winner_attempt_ref").is_none());
    assert!(trace.debug_payload.get("raw_response_ref").is_none());
    assert!(trace.debug_payload.get("context_projection_ref").is_none());
    assert!(trace.debug_payload.get("attempt_refs").is_none());
    assert!(trace.debug_payload.get("winner_attempt_ref").is_none());
    assert_no_pending_observability_ref(&trace.debug_payload);
    assert_eq!(
        trace.debug_payload["provider_events"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(trace.output_payload.get("provider_events").is_none());
}

#[tokio::test]
async fn llm_node_final_usage_preserves_input_cache_snapshot_fields_in_metrics_payload() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": {
                "query": "hello"
            }
        }),
        &InputCacheUsageSnapshotInvoker,
    )
    .await
    .unwrap();
    let trace = outcome
        .node_traces
        .into_iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(trace.output_payload["text"], json!("cache-aware response"));
    assert_eq!(
        trace.output_payload["usage"],
        trace.metrics_payload["usage"]
    );
    assert_eq!(trace.metrics_payload["usage"]["input_tokens"], json!(100));
    assert_eq!(
        trace.metrics_payload["usage"]["input_cache_hit_tokens"],
        json!(40)
    );
    assert_eq!(
        trace.metrics_payload["usage"]["input_cache_miss_tokens"],
        json!(60)
    );
    assert_eq!(trace.metrics_payload["usage"]["output_tokens"], json!(12));
    assert_eq!(trace.metrics_payload["usage"]["total_tokens"], json!(112));
    assert_eq!(
        trace.metrics_payload["usage"]["cache_write_tokens"],
        Value::Null
    );
}

#[tokio::test]
async fn llm_output_payload_keeps_think_tags_in_standard_text_content() {
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "<think>先分析用户问题</think>正式回答".to_string(),
    };
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": {
                "query": "hello"
            }
        }),
        &invoker,
    )
    .await
    .unwrap();
    let trace = outcome
        .node_traces
        .into_iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(
        trace.output_payload["text"],
        json!("<think>先分析用户问题</think>正式回答")
    );
    assert_eq!(
        trace.output_payload["answer_segments"],
        json!([
            { "kind": "reasoning", "text": "先分析用户问题" },
            { "kind": "message", "text": "正式回答" }
        ])
    );
    assert!(trace.output_payload.get("reasoning_content").is_none());
    assert!(trace.debug_payload.get("reasoning_content").is_none());
    assert!(trace.output_payload.get("message").is_none());
}

struct ReasoningDeltaProviderInvoker;

#[async_trait]
impl ProviderInvoker for ReasoningDeltaProviderInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::ReasoningDelta {
                    delta: "先分析".to_string(),
                },
                ProviderStreamEvent::TextDelta {
                    delta: "正式回答".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("正式回答".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for ReasoningDeltaProviderInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for ReasoningDeltaProviderInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("base plan does not execute code nodes")
    }
}

#[tokio::test]
async fn llm_output_payload_merges_reasoning_deltas_into_dify_style_text() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": {
                "query": "hello"
            }
        }),
        &ReasoningDeltaProviderInvoker,
    )
    .await
    .unwrap();
    let trace = outcome
        .node_traces
        .into_iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(
        trace.output_payload["text"],
        json!("<think>先分析</think>正式回答")
    );
    assert_eq!(
        trace.output_payload["answer_segments"],
        json!([
            { "kind": "reasoning", "text": "先分析" },
            { "kind": "message", "text": "正式回答" }
        ])
    );
    assert!(trace.output_payload.get("reasoning_content").is_none());
    assert!(trace.debug_payload.get("reasoning_content").is_none());
    assert!(trace.output_payload.get("message").is_none());
}

#[tokio::test]
async fn answer_node_output_payload_projects_segments_from_llm_text() {
    let outcome = start_flow_debug_run(
        &llm_answer_plan(),
        &json!({
            "node-start": {
                "query": "hello"
            }
        }),
        &ReasoningDeltaProviderInvoker,
    )
    .await
    .unwrap();
    let trace = outcome
        .node_traces
        .into_iter()
        .find(|trace| trace.node_id == "node-answer")
        .expect("answer trace should exist");

    assert_eq!(
        trace.output_payload["answer"],
        json!("<think>先分析</think>正式回答")
    );
    assert_eq!(
        trace.output_payload["answer_segments"],
        json!([
            { "kind": "reasoning", "text": "先分析" },
            { "kind": "message", "text": "正式回答" }
        ])
    );
}

#[tokio::test]
async fn llm_node_output_payload_keeps_provider_result_fields_out_of_debug_payload() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": {
                "query": "hello"
            }
        }),
        &ToolMcpMetadataInvoker,
    )
    .await
    .unwrap();
    let trace = outcome
        .node_traces
        .into_iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");

    assert_eq!(trace.output_payload["text"], json!("tool-aware response"));
    assert_eq!(
        trace.output_payload["tool_calls"][0]["name"],
        "lookup_order"
    );
    assert_eq!(trace.output_payload["mcp_calls"][0]["method"], "get_order");
    assert_eq!(
        trace.output_payload["provider_metadata"]["raw_id"],
        "provider-response-1"
    );
    assert_eq!(
        trace.output_payload["provider_metadata"]
            [PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY]["decisions"],
        json!(["omitted_protocol_context_profile_mismatch"])
    );
    assert_eq!(
        trace.output_payload["provider_route"]["provider_code"],
        "fixture_provider"
    );
    assert_eq!(trace.output_payload["finish_reason"], json!("tool_call"));
    assert!(trace.debug_payload.get("provider_metadata").is_none());
    assert!(trace.debug_payload.get("provider_route").is_none());
}

#[tokio::test]
async fn llm_runtime_sends_rendered_prompt_messages_to_provider() {
    let mut plan = base_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.bindings = BTreeMap::from([(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
            raw_value: json!([
                {
                    "id": "system-1",
                    "role": "system",
                    "content": {
                        "kind": "templated_text",
                        "value": "You are concise."
                    }
                },
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "Question: {{ node-start.query }}"
                    }
                },
                {
                    "id": "assistant-1",
                    "role": "assistant",
                    "content": {
                        "kind": "templated_text",
                        "value": "Prior answer."
                    }
                }
            ]),
        },
    )]);
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = StubProviderInvoker {
        fail: false,
        captured_input: captured_input.clone(),
        final_content: "ok".to_string(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");
    assert_eq!(input.system_text().as_deref(), Some("You are concise."));
    assert_eq!(input.messages.len(), 2);
    assert_eq!(input.messages[0].role, ProviderMessageRole::User);
    assert_eq!(input.messages[0].content, "Question: hello");
    assert_eq!(input.messages[1].role, ProviderMessageRole::Assistant);
    assert_eq!(input.messages[1].content, "Prior answer.");

    let trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert_eq!(
        trace.input_payload["prompt_messages"][1]["content"],
        json!("Question: hello")
    );
}
