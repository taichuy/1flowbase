use super::*;

#[tokio::test]
async fn start_flow_debug_run_waits_for_human_input() {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": { "query": "请总结退款政策" }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::WaitingHuman(ref wait) => {
            assert_eq!(wait.node_id, "node-human");
            assert!(wait.prompt.contains("请审核"));
        }
        other => panic!("expected waiting_human, got {other:?}"),
    }

    assert_eq!(outcome.node_traces.len(), 3);
    assert_eq!(outcome.node_traces[1].node_id, "node-llm");
    assert_eq!(
        outcome.node_traces[1].output_payload["text"],
        "echo:gpt-5.4-mini"
    );
    assert_eq!(outcome.node_traces[1].provider_events.len(), 3);
}

#[tokio::test]
async fn resume_flow_debug_run_completes_answer_after_human_input() {
    let waiting = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": { "query": "退款政策" },
            "sys": { "workflow_run_id": "run-1", "conversation_id": "conversation-1" },
            "env": { "ApiBaseUrl": "https://api.example.com" }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    let checkpoint = waiting.checkpoint_snapshot.clone().unwrap();
    let resumed = resume_flow_debug_run(
        &base_plan(),
        &checkpoint,
        "node-human",
        &json!({ "input": "已审核，可继续" }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        resumed.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        resumed.variable_pool["node-answer"]["answer"],
        json!("已审核，可继续")
    );
    let answer_trace = resumed
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-answer")
        .expect("answer trace should exist");
    assert_eq!(
        answer_trace.output_payload["sys"]["workflow_run_id"],
        json!("run-1")
    );
    assert_eq!(
        answer_trace.output_payload["env"]["ApiBaseUrl"],
        json!("https://api.example.com")
    );
}

#[tokio::test]
async fn resume_flow_debug_run_rejects_non_public_resume_output_keys() {
    let waiting = start_flow_debug_run(
        &base_plan(),
        &json!({ "node-start": { "query": "退款政策" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    let checkpoint = waiting.checkpoint_snapshot.clone().unwrap();
    let error = resume_flow_debug_run(
        &base_plan(),
        &checkpoint,
        "node-human",
        &json!({
            "input": "已审核，可继续",
            "node-llm": { "text": "polluted" }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("resume payload key node-llm is not a public output for node-human"));
}

#[tokio::test]
async fn tool_node_emits_waiting_callback_stop_reason() {
    let mut plan = base_plan();
    plan.topological_order = vec!["node-start".to_string(), "node-tool".to_string()];
    plan.nodes.remove("node-llm");
    plan.nodes.remove("node-human");
    plan.nodes.remove("node-answer");
    plan.nodes.insert(
        "node-tool".to_string(),
        CompiledNode {
            node_id: "node-tool".to_string(),
            node_type: "tool".to_string(),
            alias: "Tool".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "result".to_string(),
                title: "工具输出".to_string(),
                value_type: "json".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({ "tool_name": "lookup_order" }),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "order_123" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-tool");
            assert_eq!(pending.callback_kind, "tool");
        }
        other => panic!("expected waiting_callback, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_tool_calls_pause_current_llm_and_skip_downstream_answer() {
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![tool_call_response(vec![
        ProviderToolCall {
            id: "call_weather".to_string(),
            name: "lookup_weather".to_string(),
            arguments: json!({ "city": "Shanghai" }),
            provider_metadata: json!({
                "gemini": {
                    "thought_signature": "real-gemini-tool-signature"
                }
            }),
        },
        ProviderToolCall {
            id: "call_unit".to_string(),
            name: "lookup_unit".to_string(),
            arguments: json!({ "scale": "celsius" }),
            provider_metadata: json!({}),
        },
    ])]);

    let outcome = start_flow_debug_run(
        &llm_answer_plan(),
        &json!({ "node-start": { "query": "weather?" } }),
        &invoker,
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-llm");
            assert_eq!(pending.callback_kind, "llm_tool_calls");
            assert_eq!(
                pending.request_payload["tool_calls"][0]["id"],
                json!("call_weather")
            );
            assert_eq!(
                pending.request_payload["tool_calls"][0]["call_usage"]["input_tokens"],
                json!(11)
            );
            assert_eq!(
                pending.request_payload["tool_calls"][0]["call_usage"]["input_cache_hit_tokens"],
                json!(5)
            );
            assert_eq!(
                pending.request_payload["tool_calls"][0]["call_usage"]["output_tokens"],
                json!(3)
            );
            assert_eq!(
                pending.request_payload["tool_calls"][0]["call_usage"]["total_tokens"],
                json!(14)
            );
            assert!(pending.request_payload["tool_calls"][0]
                .get("call_output_tokens")
                .is_none());
            assert!(pending.request_payload["tool_calls"][0]
                .get("result_input_tokens")
                .is_none());
            assert!(pending.request_payload["tool_calls"][0]
                .get("token_count_method")
                .is_none());
            assert_eq!(
                pending.request_payload["tool_calls"][1]["call_usage"]["total_tokens"],
                json!(14)
            );
            assert!(pending.request_payload["tool_calls"][0]
                .get("input_cache_hit_tokens")
                .is_none());
            assert_eq!(
                pending.request_payload["tool_calls"][0]["provider_metadata"]["gemini"]
                    ["thought_signature"],
                json!("real-gemini-tool-signature")
            );
            assert_eq!(pending.request_payload["finish_reason"], json!("tool_call"));
            assert_eq!(pending.request_payload["text"], json!("need tools"));
            assert_eq!(
                pending.request_payload["provider_route"]["provider_code"],
                json!("fixture_provider")
            );
            assert_eq!(pending.request_payload["usage"]["total_tokens"], json!(14));
            assert_eq!(pending.request_payload["history"][0]["role"], json!("user"));
            assert_eq!(
                pending.request_payload["history"][0]["content"],
                json!("weather?")
            );
            assert_eq!(
                pending.request_payload["history"][1]["tool_calls"][0]["id"],
                json!("call_weather")
            );
        }
        other => panic!("expected llm tool callback wait, got {other:?}"),
    }

    assert_eq!(outcome.node_traces.len(), 2);
    assert!(outcome
        .node_traces
        .iter()
        .all(|trace| trace.node_id != "node-answer"));
    assert!(outcome.variable_pool.get("node-answer").is_none());

    let llm_trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["content"],
        json!("need tools")
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["id"],
        json!("call_weather")
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["call_usage"]
            ["input_tokens"],
        json!(11)
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["call_usage"]
            ["input_cache_hit_tokens"],
        json!(5)
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["call_usage"]
            ["output_tokens"],
        json!(3)
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["usage"]["input_tokens"],
        json!(11)
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["usage"]["input_cache_hit_tokens"],
        json!(5)
    );
    assert!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]
            .get("call_output_tokens")
            .is_none()
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["finish_reason"],
        json!("tool_call")
    );
}

#[tokio::test]
async fn llm_tool_call_finish_without_tool_calls_exposes_error_text_to_answer() {
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![tool_call_response(Vec::new())]);

    let outcome = start_flow_debug_run(
        &llm_answer_plan(),
        &json!({ "node-start": { "query": "weather?" } }),
        &invoker,
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
            assert!(failure.error_payload["message"]
                .as_str()
                .expect("failure message should be a string")
                .contains("finish_reason=tool_call"));
            assert_eq!(
                outcome.variable_pool["node-answer"]["answer"],
                failure.error_payload["message"]
            );
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }

    assert!(outcome
        .node_traces
        .iter()
        .any(|trace| trace.node_id == "node-answer"));
}

#[tokio::test]
async fn resume_llm_tool_results_recalls_same_llm_then_enters_downstream() {
    let (waiting_invoker, _waiting_inputs) =
        sequential_tool_invoker(vec![tool_call_response(vec![ProviderToolCall {
            id: "call_weather".to_string(),
            name: "lookup_weather".to_string(),
            arguments: json!({ "city": "Shanghai" }),
            provider_metadata: json!({}),
        }])]);
    let plan = llm_answer_plan();

    let waiting = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "weather?" } }),
        &waiting_invoker,
    )
    .await
    .unwrap();
    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("llm tool wait should have checkpoint");

    let (resume_invoker, resumed_inputs) =
        sequential_tool_invoker(vec![final_llm_response("weather is clear")]);
    let resumed = resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        resumed.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        resumed.variable_pool["node-answer"]["answer"],
        json!("weather is clear")
    );
    assert_eq!(resumed.node_traces[0].node_id, "node-llm");
    assert!(resumed
        .node_traces
        .iter()
        .any(|trace| trace.node_id == "node-answer"));

    let captured = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 1);
    let messages = serde_json::to_value(&captured[0].messages).expect("messages serialize");
    assert_eq!(messages[0]["role"], json!("user"));
    assert_eq!(messages[0]["content"], json!("weather?"));
    assert_eq!(messages[1]["role"], json!("assistant"));
    assert_eq!(messages[1]["tool_calls"][0]["id"], json!("call_weather"));
    assert!(messages[1]["tool_calls"][0]
        .get("call_input_tokens")
        .is_none());
    assert!(messages[1]["tool_calls"][0].get("call_usage").is_none());
    assert!(messages[1]["tool_calls"][0]
        .get("call_cached_input_tokens")
        .is_none());
    assert!(messages[1]["tool_calls"][0]
        .get("call_output_tokens")
        .is_none());
    assert_eq!(messages[2]["role"], json!("tool"));
    assert_eq!(messages[2]["tool_call_id"], json!("call_weather"));
    assert_eq!(messages[2]["name"], json!("lookup_weather"));

    let resumed_llm_trace = resumed
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("resumed llm trace should exist");
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["id"],
        json!("call_weather")
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]["tool_call_id"],
        json!("call_weather")
    );
    assert!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]
            .get("result_input_tokens")
            .is_none()
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]["result_context_usage"]
            ["input_tokens"],
        json!(20)
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]["result_context_usage"]
            ["input_cache_hit_tokens"],
        json!(8)
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]["result_context_usage"]
            ["total_tokens"],
        json!(24)
    );
    assert!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]
            .get("call_output_tokens")
            .is_none()
    );
    assert!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]
            .get("token_count_method")
            .is_none()
    );
    assert!(
        resumed_llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]
            .get("input_cache_hit_tokens")
            .is_none()
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][1]["assistant"]["content"],
        json!("weather is clear")
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][1]["usage"]["input_tokens"],
        json!(20)
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][1]["usage"]["input_cache_hit_tokens"],
        json!(8)
    );
    assert_eq!(
        resumed_llm_trace.debug_payload["llm_rounds"][1]["finish_reason"],
        json!("stop")
    );
}

#[tokio::test]
async fn resume_llm_tool_results_passes_native_response_cursor_system_and_delta_messages() {
    let mut waiting_response = tool_call_response(vec![ProviderToolCall {
        id: "call_weather".to_string(),
        name: "lookup_weather".to_string(),
        arguments: json!({ "city": "Shanghai" }),
        provider_metadata: json!({}),
    }]);
    waiting_response.response_id = Some("resp_previous".to_string());
    waiting_response.provider_metadata = json!({ "transport": "responses_websocket" });
    let (waiting_invoker, _waiting_inputs) = sequential_tool_invoker(vec![waiting_response]);
    let mut plan = llm_answer_plan();
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.bindings.insert(
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
                        "value": "Always answer with current tool evidence."
                    }
                },
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "{{node-start.query}}"
                    }
                }
            ]),
        },
    );

    let waiting = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "weather?" } }),
        &waiting_invoker,
    )
    .await
    .unwrap();

    match waiting.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(
                pending.request_payload["response_id"],
                json!("resp_previous")
            );
        }
        other => panic!("expected llm tool callback wait, got {other:?}"),
    }

    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("llm tool wait should have checkpoint");
    let (resume_invoker, resumed_inputs) =
        sequential_tool_invoker(vec![final_llm_response("weather is clear")]);

    resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    let captured = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].previous_response_id.as_deref(),
        Some("resp_previous")
    );
    assert_eq!(
        captured[0].system_text().as_deref(),
        Some("Always answer with current tool evidence.")
    );
    assert_eq!(captured[0].messages.len(), 1);
    assert_eq!(captured[0].messages[0].role, ProviderMessageRole::Tool);
    assert_eq!(
        captured[0].messages[0].tool_call_id.as_deref(),
        Some("call_weather")
    );
    assert_eq!(
        captured[0].messages[0].name.as_deref(),
        Some("lookup_weather")
    );
}

#[tokio::test]
async fn resume_llm_tool_results_replays_full_history_after_http_sse_response_cursor() {
    let mut waiting_response = tool_call_response(vec![ProviderToolCall {
        id: "call_weather".to_string(),
        name: "lookup_weather".to_string(),
        arguments: json!({ "city": "Shanghai" }),
        provider_metadata: json!({}),
    }]);
    waiting_response.response_id = Some("resp_from_http_sse".to_string());
    waiting_response.provider_metadata = json!({ "transport": "http_sse" });
    let (waiting_invoker, _waiting_inputs) = sequential_tool_invoker(vec![waiting_response]);
    let plan = llm_answer_plan();

    let waiting = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "weather?" } }),
        &waiting_invoker,
    )
    .await
    .unwrap();

    match waiting.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(
                pending.request_payload["response_id"],
                json!("resp_from_http_sse")
            );
            assert_eq!(
                pending.request_payload["provider_metadata"]["transport"],
                json!("http_sse")
            );
        }
        other => panic!("expected llm tool callback wait, got {other:?}"),
    }

    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("llm tool wait should have checkpoint");
    let (resume_invoker, resumed_inputs) =
        sequential_tool_invoker(vec![final_llm_response("weather is clear")]);

    resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    let captured = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].previous_response_id, None);
    assert_eq!(captured[0].messages.len(), 3);
    assert_eq!(captured[0].messages[0].role, ProviderMessageRole::User);
    assert_eq!(captured[0].messages[0].content, "weather?");
    assert_eq!(captured[0].messages[1].role, ProviderMessageRole::Assistant);
    assert_eq!(
        captured[0].messages[1].tool_calls.as_ref().unwrap()[0]["id"],
        json!("call_weather")
    );
    assert_eq!(captured[0].messages[2].role, ProviderMessageRole::Tool);
    assert_eq!(
        captured[0].messages[2].tool_call_id.as_deref(),
        Some("call_weather")
    );
}

#[tokio::test]
async fn resume_llm_tool_results_preserves_media_content_blocks() {
    let mut waiting_response = tool_call_response(vec![ProviderToolCall {
        id: "call_read_image".to_string(),
        name: "Read".to_string(),
        arguments: json!({ "file_path": "uploads/agent-flow-preview-debug.png" }),
        provider_metadata: json!({}),
    }]);
    waiting_response.response_id = Some("resp_from_http_sse".to_string());
    waiting_response.provider_metadata = json!({ "transport": "http_sse" });
    let (waiting_invoker, _waiting_inputs) = sequential_tool_invoker(vec![waiting_response]);
    let plan = llm_answer_plan();

    let waiting = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "describe image" } }),
        &waiting_invoker,
    )
    .await
    .unwrap();
    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("llm tool wait should have checkpoint");
    let (resume_invoker, resumed_inputs) =
        sequential_tool_invoker(vec![final_llm_response("image described")]);

    resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_read_image",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }
                    ]
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap();

    let captured = resumed_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].messages.len(), 3);
    assert_eq!(captured[0].messages[2].role, ProviderMessageRole::Tool);
    assert_eq!(
        captured[0].messages[2].tool_call_id.as_deref(),
        Some("call_read_image")
    );
    assert_eq!(
        captured[0].messages[2].content_blocks.as_ref().unwrap()[0]["type"],
        json!("image")
    );
}

mod visible_internal_llm_tool_claude_code;
mod visible_internal_llm_tool_errors;
mod visible_internal_llm_tool_fixtures;
mod visible_internal_llm_tool_media;
mod visible_internal_llm_tool_resume;
mod visible_internal_llm_tools;
#[tokio::test]
async fn multi_round_llm_tool_callbacks_keep_previous_round_debug_evidence() {
    let first_call = ProviderToolCall {
        id: "call_weather".to_string(),
        name: "lookup_weather".to_string(),
        arguments: json!({ "city": "Shanghai" }),
        provider_metadata: json!({}),
    };
    let second_call = ProviderToolCall {
        id: "call_time".to_string(),
        name: "lookup_time".to_string(),
        arguments: json!({ "city": "Shanghai" }),
        provider_metadata: json!({}),
    };
    let plan = llm_answer_plan();
    let (waiting_invoker, _waiting_inputs) =
        sequential_tool_invoker(vec![tool_call_response(vec![first_call])]);

    let waiting = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "weather and time?" } }),
        &waiting_invoker,
    )
    .await
    .unwrap();
    let first_checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("first tool wait should have checkpoint");

    let (second_wait_invoker, _second_inputs) =
        sequential_tool_invoker(vec![tool_call_response(vec![second_call])]);
    let second_wait = resume_flow_debug_run(
        &plan,
        &first_checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        }),
        &second_wait_invoker,
    )
    .await
    .unwrap();

    match second_wait.stop_reason {
        ExecutionStopReason::WaitingCallback(ref pending) => {
            assert_eq!(pending.node_id, "node-llm");
            assert_eq!(
                pending.request_payload["tool_calls"][0]["id"],
                json!("call_time")
            );
        }
        other => panic!("expected second llm tool callback wait, got {other:?}"),
    }

    let llm_trace = second_wait
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("second wait llm trace should exist");
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["id"],
        json!("call_weather")
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][0]["tool_results"][0]["tool_call_id"],
        json!("call_weather")
    );
    assert_eq!(
        llm_trace.debug_payload["llm_rounds"][1]["assistant"]["tool_calls"][0]["id"],
        json!("call_time")
    );
}

#[tokio::test]
async fn resume_llm_tool_results_rejects_missing_tool_results() {
    let (invoker, _captured_inputs) = sequential_tool_invoker(vec![tool_call_response(vec![
        ProviderToolCall {
            id: "call_weather".to_string(),
            name: "lookup_weather".to_string(),
            arguments: json!({ "city": "Shanghai" }),
            provider_metadata: json!({}),
        },
        ProviderToolCall {
            id: "call_time".to_string(),
            name: "lookup_time".to_string(),
            arguments: json!({ "city": "Shanghai" }),
            provider_metadata: json!({}),
        },
    ])]);
    let plan = llm_answer_plan();

    let waiting = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "weather and time?" } }),
        &invoker,
    )
    .await
    .unwrap();
    let checkpoint = waiting
        .checkpoint_snapshot
        .clone()
        .expect("llm tool wait should have checkpoint");

    let (resume_invoker, _resume_inputs) =
        sequential_tool_invoker(vec![final_llm_response("should not be called")]);
    let error = resume_flow_debug_run(
        &plan,
        &checkpoint,
        "node-llm",
        &json!({
            "tool_results": [
                {
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        }),
        &resume_invoker,
    )
    .await
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("missing tool result for call_time"));
}
