use super::*;

#[tokio::test]
async fn runtime_internal_tool_executes_inline_and_continues_llm() {
    let plan = llm_answer_plan();
    let (provider, captured_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_mcp".to_string(),
            name: "catalog_mcp_call".to_string(),
            arguments: json!({"tool_id": "lookup", "arguments": {"id": 1}}),
            provider_metadata: json!({}),
        }]),
        final_llm_response("done"),
    ]);
    let internal = Arc::new(StubRuntimeInternalToolInvoker::default());
    let lifecycle = RecordingExecutionLifecycle::default();
    let input = json!({"node-start": {"query": "lookup"}});
    let variable_pool = input.as_object().unwrap().clone();
    let context = ExecutionRuntimeContext::from_plan_input(&plan, &variable_pool)
        .unwrap()
        .with_runtime_internal_tool_invoker(internal.clone());

    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan, &input, context, &provider, &lifecycle,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(internal.calls.lock().unwrap().len(), 1);
    let inputs = captured_inputs.lock().unwrap();
    assert_eq!(inputs.len(), 2);
    let registered_tools = inputs[0]
        .tools
        .iter()
        .filter(|tool| tool["function"]["name"] == json!("catalog_mcp_call"))
        .count();
    assert_eq!(registered_tools, 1);
    let start_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-start")
        .expect("start trace should freeze run-level tool registrations");
    assert_eq!(
        start_trace.input_payload["tools"][0]["function"]["name"],
        json!("catalog_mcp_call")
    );
    assert_eq!(start_trace.input_payload["tools"], json!(inputs[0].tools));
    assert_eq!(
        start_trace.input_payload["tool_registrations"][0]["execution_kind"],
        json!("host_internal")
    );
    assert_eq!(
        start_trace.input_payload["tool_registrations"][0]["owner"]["source"]["kind"],
        json!("run")
    );
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should record the internal tool lifecycle");
    assert_eq!(llm_trace.input_payload["tools"], json!(inputs[0].tools));
    assert_eq!(
        llm_trace.input_payload["tool_registrations"][0]["provider_name"],
        json!("catalog_mcp_call")
    );
    assert_eq!(
        llm_trace.input_payload["tool_registrations"][0]["execution_kind"],
        json!("host_internal")
    );
    assert_eq!(
        llm_trace.input_payload["tool_registrations"][0]["owner"]["instance_id"],
        json!("catalog")
    );
    assert_eq!(
        llm_trace.metrics_payload["internal_tool_call_count"],
        json!(1)
    );
    assert_eq!(
        llm_trace.debug_payload["runtime_internal_tool_events"][0]["tool_call_id"],
        json!("call_mcp")
    );
    assert_eq!(
        llm_trace.debug_payload["runtime_internal_tool_events"][0]["execution_status"],
        json!("succeeded")
    );
    assert_eq!(
        llm_trace.debug_payload["runtime_internal_tool_events"][0]["owner"]["instance_id"],
        json!("catalog")
    );
    assert_eq!(
        *lifecycle.tool_events.lock().unwrap(),
        vec![
            json!({
                "phase": "started",
                "node_id": "node-llm",
                "tool_call_id": "call_mcp",
                "name": "catalog_mcp_call",
                "call_usage": {
                    "input_tokens": 11,
                    "input_cache_hit_tokens": 5,
                    "input_cache_miss_tokens": null,
                    "output_tokens": 3,
                    "reasoning_tokens": null,
                    "cache_read_tokens": null,
                    "cache_write_tokens": null,
                    "total_tokens": 14
                }
            }),
            json!({
                "phase": "finished",
                "node_id": "node-llm",
                "tool_call_id": "call_mcp",
                "name": "catalog_mcp_call",
                "call_usage": {
                    "input_tokens": 11,
                    "input_cache_hit_tokens": 5,
                    "input_cache_miss_tokens": null,
                    "output_tokens": 3,
                    "reasoning_tokens": null,
                    "cache_read_tokens": null,
                    "cache_write_tokens": null,
                    "total_tokens": 14
                },
                "is_error": false
            })
        ]
    );
}

#[tokio::test]
async fn runtime_internal_tool_continues_after_eight_rounds() {
    let plan = llm_answer_plan();
    let mut responses = (0..9)
        .map(|index| {
            tool_call_response(vec![ProviderToolCall {
                id: format!("call_mcp_{index}"),
                name: "catalog_mcp_call".to_string(),
                arguments: json!({"tool_id": "lookup", "arguments": {"index": index}}),
                provider_metadata: json!({}),
            }])
        })
        .collect::<Vec<_>>();
    responses.push(final_llm_response("done after nine tool rounds"));
    let (provider, captured_inputs) = sequential_tool_invoker(responses);
    let internal = Arc::new(StubRuntimeInternalToolInvoker::default());
    let input = json!({"node-start": {"query": "complete a long tool task"}});
    let variable_pool = input.as_object().unwrap().clone();
    let context = ExecutionRuntimeContext::from_plan_input(&plan, &variable_pool)
        .unwrap()
        .with_runtime_internal_tool_invoker(internal.clone());

    let outcome = start_flow_debug_run_with_runtime_context(&plan, &input, context, &provider)
        .await
        .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(internal.calls.lock().unwrap().len(), 9);
    assert_eq!(captured_inputs.lock().unwrap().len(), 10);
    assert_eq!(
        outcome.variable_pool["node-answer"]["answer"],
        json!("done after nine tool rounds")
    );
}

#[tokio::test]
async fn runtime_internal_tool_lifecycle_persists_the_frozen_llm_catalog() {
    let plan = llm_answer_plan();
    let (provider, captured_inputs) = sequential_tool_invoker(vec![final_llm_response("done")]);
    let internal = Arc::new(StubRuntimeInternalToolInvoker::default());
    let lifecycle = RecordingExecutionLifecycle::default();
    let input = json!({"node-start": {"query": "lookup"}});
    let variable_pool = input.as_object().unwrap().clone();
    let context = ExecutionRuntimeContext::from_plan_input(&plan, &variable_pool)
        .unwrap()
        .with_runtime_internal_tool_invoker(internal);

    start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan, &input, context, &provider, &lifecycle,
    )
    .await
    .unwrap();

    let provider_tools = captured_inputs.lock().unwrap()[0].tools.clone();
    let lifecycle_inputs = lifecycle.inputs.lock().unwrap();
    assert_eq!(lifecycle_inputs["node-llm"]["tools"], json!(provider_tools));
    assert_eq!(
        lifecycle_inputs["node-llm"]["tool_registrations"][0]["provider_name"],
        json!("catalog_mcp_call")
    );
}

#[tokio::test]
async fn runtime_internal_tool_snapshot_preserves_wire_name_without_duplicate_registration() {
    let plan = llm_answer_plan();
    let frozen_name = "catalog_mcp_call_frozen";
    let input = json!({
        "node-start": {
            "query": "lookup",
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": frozen_name,
                        "description": "call catalog",
                        "parameters": {"type": "object"}
                    }
                }
            ],
            "tool_registrations": [
                {
                    "registration_id": "catalog|call",
                    "provider_name": frozen_name,
                    "execution_kind": "host_internal",
                    "owner": {
                        "kind": "mcp_instance",
                        "instance_id": "catalog",
                        "operation": "call",
                        "source": {"kind": "run", "key": "0"}
                    },
                    "node_ids": ["node-llm"]
                }
            ]
        }
    });
    let (provider, captured_inputs) = sequential_tool_invoker(vec![
        tool_call_response(vec![ProviderToolCall {
            id: "call_frozen".to_string(),
            name: frozen_name.to_string(),
            arguments: json!({"tool_id": "lookup", "arguments": {}}),
            provider_metadata: json!({}),
        }]),
        final_llm_response("done"),
    ]);
    let internal = Arc::new(StubRuntimeInternalToolInvoker::default());
    let context =
        ExecutionRuntimeContext::from_plan_input(&plan, &input.as_object().unwrap().clone())
            .unwrap()
            .with_runtime_internal_tool_invoker(internal.clone());

    let outcome = start_flow_debug_run_with_runtime_context(&plan, &input, context, &provider)
        .await
        .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(internal.calls.lock().unwrap().len(), 1);
    let inputs = captured_inputs.lock().unwrap();
    assert_eq!(
        inputs[0]
            .tools
            .iter()
            .filter(|tool| tool["function"]["name"] == json!(frozen_name))
            .count(),
        1
    );
    assert_eq!(json!(inputs[0].tools), input["node-start"]["tools"]);
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should preserve the frozen provider wire name");
    assert_eq!(llm_trace.input_payload["tools"], json!(inputs[0].tools));
    assert_eq!(
        llm_trace.input_payload["tool_registrations"][0]["provider_name"],
        json!(frozen_name)
    );
}

#[tokio::test]
async fn runtime_internal_tool_snapshot_is_not_exposed_without_an_active_invoker_registration() {
    let plan = llm_answer_plan();
    let input = json!({
        "node-start": {
            "query": "lookup",
            "tools": [{
                "type": "function",
                "function": {
                    "name": "catalog_mcp_call",
                    "parameters": {"type": "object"}
                }
            }],
            "tool_registrations": [{
                "registration_id": "catalog|call",
                "provider_name": "catalog_mcp_call",
                "execution_kind": "host_internal",
                "owner": {
                    "kind": "mcp_instance",
                    "instance_id": "catalog",
                    "operation": "call",
                    "source": {"kind": "run", "key": "0"}
                },
                "node_ids": ["node-llm"]
            }]
        }
    });
    let (provider, captured_inputs) =
        sequential_tool_invoker(vec![final_llm_response("no tool available")]);

    let outcome = start_flow_debug_run(&plan, &input, &provider)
        .await
        .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    let inputs = captured_inputs.lock().unwrap();
    assert!(inputs[0].tools.is_empty());
    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should record the effective empty tool input");
    assert_eq!(llm_trace.input_payload["tools"], json!([]));
    assert_eq!(llm_trace.input_payload["tool_registrations"], json!([]));
}

#[tokio::test]
async fn runtime_internal_tool_mixed_round_exposes_only_external_callback() {
    let plan = llm_answer_plan();
    let (provider, _) = sequential_tool_invoker(vec![tool_call_response(vec![
        ProviderToolCall {
            id: "call_mcp".to_string(),
            name: "catalog_mcp_call".to_string(),
            arguments: json!({"tool_id": "lookup", "arguments": {}}),
            provider_metadata: json!({}),
        },
        ProviderToolCall {
            id: "call_external".to_string(),
            name: "external_weather".to_string(),
            arguments: json!({"city": "Shanghai"}),
            provider_metadata: json!({}),
        },
    ])]);
    let internal = Arc::new(StubRuntimeInternalToolInvoker::default());
    let input = json!({"node-start": {"query": "mixed"}});
    let context =
        ExecutionRuntimeContext::from_plan_input(&plan, &input.as_object().unwrap().clone())
            .unwrap()
            .with_runtime_internal_tool_invoker(internal.clone());

    let outcome = start_flow_debug_run_with_runtime_context(&plan, &input, context, &provider)
        .await
        .unwrap();

    let ExecutionStopReason::WaitingCallback(pending) = outcome.stop_reason else {
        panic!("mixed round should wait for external callback");
    };
    let calls = pending.request_payload["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["id"], json!("call_external"));
    assert_eq!(internal.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn canonical_run_tools_reach_provider_when_business_inputs_are_nested() {
    let plan = llm_answer_plan();
    let tools: Vec<_> = (0..24).map(|index|json!({"name":format!("tool_{index}"),"input_schema":{"type":"object","properties":{"query":{"type":"string"}}},"source":"client"})).collect();
    use extension_contracts::provider_contract::NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY;
    let mut input = json!({
        "tools":[{"name":"legacy_should_not_win","input_schema":{"type":"object"}}],
        "tool_choice":{"type":"auto"},
        "node-start":{"query":"Use a client tool","inputs":{"tools":tools,"tool_choice":{"type":"required"}},"tool_choice":{"type":"auto"}}
    });
    input[NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY] =
        json!({"tools":tools,"tool_choice":{"type":"required"}});
    let (provider, captured) = sequential_tool_invoker(vec![final_llm_response("done")]);
    let context =
        ExecutionRuntimeContext::from_plan_input(&plan, input.as_object().unwrap()).unwrap();
    start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context,
        &provider,
        &RecordingExecutionLifecycle::default(),
    )
    .await
    .unwrap();
    let calls = captured.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tools.len(), 24);
    for (index, tool) in calls[0].tools.iter().enumerate() {
        assert_eq!(tool["function"]["name"], tools[index]["name"]);
        assert_eq!(tool["function"]["parameters"], tools[index]["input_schema"]);
    }
    assert_eq!(
        calls[0].model_parameters["tool_choice"],
        json!({"type":"required"})
    );
}

#[tokio::test]
async fn empty_canonical_run_tools_do_not_restore_legacy_run_catalog() {
    use extension_contracts::provider_contract::NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY;
    let plan = llm_answer_plan();
    let mut input = json!({"tools":[{"name":"legacy","input_schema":{"type":"object"}}],"node-start":{"query":"hello"}});
    input[NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY] = json!({"tools":[]});
    let (provider, captured) = sequential_tool_invoker(vec![final_llm_response("done")]);
    let context =
        ExecutionRuntimeContext::from_plan_input(&plan, input.as_object().unwrap()).unwrap();
    start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context,
        &provider,
        &RecordingExecutionLifecycle::default(),
    )
    .await
    .unwrap();
    assert!(captured.lock().unwrap()[0].tools.is_empty());
}
