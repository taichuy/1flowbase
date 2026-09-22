use super::*;
use plugin_framework::provider_contract::{ProviderInvocationResult, ProviderToolCall};

fn fixture(
    capacity: usize,
) -> (
    Capture,
    mpsc::Receiver<Record>,
    tokio::sync::oneshot::Receiver<bool>,
) {
    let (sender, receiver) = mpsc::channel(capacity);
    let (done, completion) = tokio::sync::oneshot::channel();
    let id = Identity {
        run: Uuid::now_v7(),
        node: "model".into(),
        node_run: Uuid::now_v7(),
        invocation: Uuid::now_v7(),
        attempt: 0,
        observation_context: None,
        purpose: "generate",
        duration_ms: Arc::new(AtomicU64::new(u64::MAX)),
    };
    (
        Capture {
            sink: Some(Sink {
                id,
                sender,
                bytes: Arc::new(tokio::sync::Semaphore::new(CAPACITY)),
                observed: Arc::new(AtomicU64::new(0)),
                dropped: Arc::new(AtomicU64::new(0)),
            }),
            aggregate: Default::default(),
            completion: Some(done),
            started_at: Some(std::time::Instant::now()),
        },
        receiver,
        completion,
    )
}
fn records(mut receiver: mpsc::Receiver<Record>) -> Vec<Value> {
    let mut values = Vec::new();
    while let Ok(record) = receiver.try_recv() {
        values.push(record.payload.payload);
    }
    values
}
fn body(record: &Value) -> Value {
    serde_json::from_str(record["body"].as_str().unwrap()).unwrap()
}
#[test]
fn chunks_are_not_steps_and_result_does_not_duplicate_reply_or_tool() {
    for chunks in [vec!["hello"], vec!["h", "e", "llo"]] {
        let (capture, receiver, _) = fixture(16);
        let observer = capture.observer();
        for chunk in chunks {
            observer.observe(&ProviderStreamEvent::TextDelta {
                delta: chunk.into(),
            });
        }
        observer.observe(&ProviderStreamEvent::ReasoningDelta {
            delta: "reason".into(),
        });
        let call = ProviderToolCall {
            id: "call-1".into(),
            name: "search".into(),
            arguments: json!({"q":"x"}),
            provider_metadata: json!({}),
        };
        observer.observe(&ProviderStreamEvent::ToolCallCommit { call: call.clone() });
        capture.finish(
            Some(&ProviderInvocationResult {
                final_content: Some("hello".into()),
                tool_calls: vec![call],
                ..Default::default()
            }),
            None,
            true,
        );
        let records = records(receiver);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["kind"], "model_reply");
        assert_eq!(body(&records[0])["final_content"], "hello");
        assert_eq!(body(&records[0])["reasoning"], "reason");
        assert_eq!(records[1]["tool_call_id"], "call-1");
        assert_eq!(records[1]["status"], "recorded");
        assert!(body(&records[0])["result"].get("final_content").is_none());
        assert_eq!(records[0]["body_format"], "native_reply_v2");
        assert!(records[1].get("body").is_none());
        assert_eq!(records[1]["body_ref"]["step_key"], records[0]["step_key"]);
        assert_eq!(records[1]["body_ref"]["pointer"], "/result/tool_calls/0");
    }
}
#[test]
fn result_only_preserves_unknown_metadata_without_promoting_fake_steps_or_credentials() {
    let (capture, receiver, _) = fixture(16);
    capture.finish(Some(&ProviderInvocationResult { final_content: Some("reply".into()), provider_metadata: json!({"future_vendor":{"opaque":[1,{"x":true}],"authorization":"Bearer secret","headers":{"x":"secret"}},"pretend_tool_call":{"id":"not-a-native-call"}}), ..Default::default() }), None, true);
    let records = records(receiver);
    assert_eq!(records.len(), 1);
    let detail = body(&records[0]);
    assert_eq!(
        detail["result"]["provider_metadata"]["future_vendor"]["opaque"][1]["x"],
        true
    );
    assert!(detail.to_string().find("secret").is_none());
    assert_eq!(records[0]["source"], "ai_native");
}
#[test]
fn native_output_items_capture_tools_and_unknown_content_without_raw() {
    let (capture, receiver, _) = fixture(16);
    let observer = capture.observer();
    for (index, item) in [json!({"type":"function_call","id":"item-1","call_id":"call-1","name":"search","arguments":"{}"}),json!({"type":"vendor_future_content","future":{"data":[42]}})].into_iter().enumerate() {
        observer.observe(&ProviderStreamEvent::OutputItem { phase: ProviderOutputItemPhase::Added, output_index: index, item: item.clone() });
        observer.observe(&ProviderStreamEvent::OutputItem { phase: ProviderOutputItemPhase::Done, output_index: index, item });
    }
    capture.finish(None, None, true);
    let records = records(receiver);
    assert_eq!(records.len(), 2);
    assert_eq!(
        body(&records[0])["output_items"][0]["future"]["data"][0],
        42
    );
    assert_eq!(records[1]["tool_call_id"], "call-1");
}
#[tokio::test]
async fn unfinished_native_items_and_capacity_truncation_are_incomplete() {
    for oversized in [false, true] {
        let (capture, receiver, completion) = fixture(16);
        if oversized {
            capture.observer().observe(&ProviderStreamEvent::TextDelta {
                delta: "x".repeat(CAPACITY),
            });
        } else {
            capture
                .observer()
                .observe(&ProviderStreamEvent::OutputItem {
                    phase: ProviderOutputItemPhase::Added,
                    output_index: 0,
                    item: json!({"type":"message","id":"m"}),
                });
        }
        capture.finish(None, None, true);
        assert!(!completion.await.unwrap());
        assert!(records(receiver)
            .iter()
            .any(|record| record["kind"] == "observation_gap"));
    }
}
#[tokio::test]
async fn sidecar_counts_successful_admission_and_marks_queue_loss_cancel_and_write_failure() {
    for (cancel, fail, capacity, expected) in [
        (false, false, 16, "complete"),
        (true, false, 16, "incomplete"),
        (false, true, 16, "incomplete"),
        (false, false, 1, "incomplete"),
    ] {
        let (capture, receiver, completion) = fixture(capacity);
        let sink = capture.sink.as_ref().unwrap();
        let (id, observed, dropped) =
            (sink.id.clone(), sink.observed.clone(), sink.dropped.clone());
        sink.step(
            "model_call",
            "request",
            "prepared",
            json!({"messages":[]}),
            None,
            false,
        );
        if cancel {
            drop(capture);
        } else {
            capture.finish(None, Some("upstream error"), true);
        }
        let written = Arc::new(std::sync::Mutex::new(Vec::new()));
        let output = written.clone();
        write(
            id,
            receiver,
            observed.clone(),
            dropped.clone(),
            completion,
            move |payload| {
                let output = output.clone();
                async move {
                    if fail && payload.event_type == "provider_semantic_step" {
                        return false;
                    }
                    output.lock().unwrap().push(payload);
                    true
                }
            },
        )
        .await;
        let written = written.lock().unwrap();
        let last = &written.last().unwrap().payload;
        assert_eq!(last["status"], expected);
        assert_eq!(last["observed_count"], observed.load(Relaxed));
        if capacity == 1 {
            assert!(last["dropped_count"].as_u64().unwrap() > 0);
        }
        if fail {
            assert!(last["persist_failed_count"].as_u64().unwrap() > 0);
        }
    }
}
#[test]
fn serialization_budget_is_checked_before_retaining_large_output() {
    assert!(bounded(&json!({"huge":"x".repeat(CAPACITY)})).is_none());
    assert!(bounded(&json!({"small":"ok"})).is_some());
}

#[test]
fn native_continuation_keeps_sealed_request_and_tool_results_without_auth_context() {
    let input = ProviderInvocationInput {
        provider_config: json!({"api_key":"secret"}),
        run_context: BTreeMap::from([("authentication".into(), json!("secret"))]),
        native_transport: Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: "openai.responses".into(),
                digest: "digest".into(),
                size_bytes: 100,
                wire_body: json!({"input":[{"type":"function_call_output","call_id":"call-1","output":"answer"}],"tools":[{"properties":{"password":{"type":"string"},"authorship":{"type":"string"}}}]}),
            },
        ),
        tools: vec![
            json!({"properties":{"password":{"type":"string"},"authorship":{"type":"string"}}}),
        ],
        ..Default::default()
    };
    let (capture, receiver, _) = fixture(16);
    assert!(!record_input(capture.sink.as_ref().unwrap(), &input));
    drop(capture);
    let records = records(receiver);
    assert_eq!(records.len(), 2);
    let input = body(&records[0]);
    assert!(input.get("provider_config").is_none());
    assert!(input.get("run_context").is_none());
    assert_eq!(
        input["native_request"]["wire_body"]["input"][0]["output"],
        "answer"
    );
    assert_eq!(
        input["tools"][0]["properties"]["password"]["type"],
        "string"
    );
    assert_eq!(
        input["tools"][0]["properties"]["authorship"]["type"],
        "string"
    );
    assert_eq!(records[1]["kind"], "tool_result");
    assert_eq!(records[1]["direction"], "prepared");
    assert_eq!(records[1]["tool_call_id"], "call-1");
    assert!(records[1].get("body").is_none());
    assert_eq!(records[1]["body_ref"]["step_key"], records[0]["step_key"]);
    assert_eq!(
        records[1]["body_ref"]["pointer"],
        "/native_request/wire_body/input/0"
    );
}
#[tokio::test]
async fn error_only_has_complete_capture_without_invented_reply() {
    let (capture, receiver, completion) = fixture(16);
    capture.finish(None, Some("failed"), true);
    assert!(completion.await.unwrap());
    let records = records(receiver);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["kind"], "error");
}

#[tokio::test]
async fn native_tool_output_done_closes_argument_stream_without_tool_commit_or_result_calls() {
    for done in [true, false] {
        let (capture, receiver, completion) = fixture(16);
        let observer = capture.observer();
        let item = json!({
            "type":"function_call", "id":"fc-1", "call_id":"call-1",
            "name":"echo", "arguments":"{\"text\":\"trace-ok\"}"
        });
        observer.observe(&ProviderStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: 0,
            item: item.clone(),
        });
        // Actual Native passthrough shape: delta identity is the output-item id,
        // while Done also carries a different canonical tool call_id.
        observer.observe(&ProviderStreamEvent::ToolCallDelta {
            call_id: "fc-1".into(),
            delta: json!({"arguments":"{\"text\":\"trace-ok\"}"}),
        });
        observer.observe(&ProviderStreamEvent::ResponsesOutputDelta {
            event: json!({"type":"response.function_call_arguments.delta", "output_index":0,
                "item_id":"fc-1", "delta":"{\"text\":\"trace-ok\"}"}),
        });
        if done {
            observer.observe(&ProviderStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Done,
                output_index: 0,
                item,
            });
        }
        let result = ProviderInvocationResult {
            finish_reason: Some(
                plugin_framework::provider_contract::ProviderFinishReason::ToolCall,
            ),
            ..Default::default()
        };
        assert!(result.tool_calls.is_empty());
        capture.finish(Some(&result), None, true);
        assert_eq!(completion.await.unwrap(), done);
        let records = records(receiver);
        assert_eq!(
            records
                .iter()
                .filter(|record| record["kind"] == "tool_call")
                .count(),
            usize::from(done)
        );
        assert_eq!(
            records
                .iter()
                .any(|record| record["kind"] == "observation_gap"),
            !done
        );
        let reply = records
            .iter()
            .find(|record| record["kind"] == "model_reply")
            .unwrap();
        assert_eq!(
            reply["status"],
            if done { "recorded" } else { "incomplete" }
        );
        if done {
            let tool = records
                .iter()
                .find(|record| record["kind"] == "tool_call")
                .unwrap();
            assert_eq!(tool["tool_call_id"], "call-1");
            assert_eq!(body(tool)["arguments"], "{\"text\":\"trace-ok\"}");
        }
    }
}

#[test]
fn responses_delta_cannot_open_or_reopen_output_items() {
    for previously_done in [false, true] {
        let mut aggregate = Aggregate::default();
        if previously_done {
            for phase in [
                ProviderOutputItemPhase::Added,
                ProviderOutputItemPhase::Done,
            ] {
                aggregate.observe(&ProviderStreamEvent::OutputItem {
                    phase,
                    output_index: 0,
                    item: json!({"type":"message","id":"m-1","content":[]}),
                });
            }
        }
        aggregate.observe(&ProviderStreamEvent::ResponsesOutputDelta {
            event: json!({"type":"response.output_text.delta", "output_index":0,
                "item_id":"m-1", "content_index":0, "delta":"late"}),
        });
        assert!(aggregate.open_items.is_empty());
        assert!(aggregate.gap, "invalid delta ordering must stay incomplete");
    }
}

#[test]
fn preview_uses_native_content_and_tool_name_with_unicode_limit() {
    assert_eq!(
        native_preview(
            "model_reply",
            &json!({"final_content":"Hello there", "reasoning":"hidden", "result":{"metadata":"not content"}})
        ),
        "Hello there"
    );
    assert_eq!(
        native_preview(
            "model_call",
            &json!({"messages":[{"role":"user","content":[{"type":"text","text":"What is the weather?"}]}]})
        ),
        "What is the weather?"
    );
    assert_eq!(
        native_preview(
            "tool_call",
            &json!({"id":"call-1","name":"weather_lookup","arguments":{"city":"Paris"}})
        ),
        "weather_lookup"
    );
    assert_eq!(
        native_preview("tool_result", &json!({"content":"Sunny"})),
        "Sunny"
    );
    assert_eq!(
        native_preview("model_reply", &json!({"final_content":"界".repeat(500)}))
            .chars()
            .count(),
        240
    );
    assert_eq!(
        native_preview(
            "model_reply",
            &json!({"result":{"provider_metadata":{"text":"must not display"}}})
        ),
        ""
    );
}

#[test]
fn oversized_request_retains_available_tool_result_without_dangling_reference() {
    let input = ProviderInvocationInput {
        tools: vec![json!({"description":"x".repeat(CAPACITY)})],
        native_transport: Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: "openai.responses".into(),
                digest: "d".into(),
                size_bytes: 100,
                wire_body: json!({"input":[{"type":"function_call_output","call_id":"c","output":"kept"}]}),
            },
        ),
        ..Default::default()
    };
    let (capture, receiver, _) = fixture(16);
    assert!(record_input(capture.sink.as_ref().unwrap(), &input));
    let records = records(receiver);
    assert_eq!(records[0]["status"], "incomplete");
    assert!(records[1].get("body_ref").is_none());
    assert_eq!(body(&records[1])["output"], "kept");
}

#[test]
fn different_stream_tool_evidence_is_not_replaced_by_result_with_same_id() {
    let (capture, receiver, _) = fixture(16);
    let observed = ProviderToolCall {
        id: "c".into(),
        name: "search".into(),
        arguments: json!({"q":"stream"}),
        provider_metadata: json!({}),
    };
    capture
        .observer()
        .observe(&ProviderStreamEvent::ToolCallCommit { call: observed });
    capture.finish(
        Some(&ProviderInvocationResult {
            tool_calls: vec![ProviderToolCall {
                id: "c".into(),
                name: "search".into(),
                arguments: json!({"q":"final"}),
                provider_metadata: json!({}),
            }],
            ..Default::default()
        }),
        None,
        true,
    );
    let records = records(receiver);
    let tool = records.iter().find(|r| r["kind"] == "tool_call").unwrap();
    assert!(tool.get("body_ref").is_none());
    assert_eq!(body(tool)["arguments"]["q"], "stream");
    assert_eq!(
        body(&records[0])["result"]["tool_calls"][0]["arguments"]["q"],
        "final"
    );
}

#[test]
fn repeated_large_reply_and_tool_are_stored_once_per_snapshot() {
    let text = "unique-reply-".repeat(4000);
    let arguments = json!({"data":"unique-tool-".repeat(4000)});
    let result = ProviderInvocationResult {
        final_content: Some(text.clone()),
        tool_calls: vec![ProviderToolCall {
            id: "c".into(),
            name: "search".into(),
            arguments,
            provider_metadata: json!({}),
        }],
        ..Default::default()
    };
    let (capture, receiver, _) = fixture(16);
    capture.finish(Some(&result), None, true);
    let records = records(receiver);
    let size: usize = records.iter().map(|r| r.to_string().len()).sum();
    let legacy = json!({"final_content":text,"reasoning":"","reasoning_signature":"","output_items":[],"result":result}).to_string().len()
        + serde_json::to_string(&result.tool_calls[0]).unwrap().len();
    eprintln!("Native snapshot fixture: compact={size} bytes; previous={legacy} bytes");
    assert!(
        size * 100 < legacy * 65,
        "compact={size}, duplicated={legacy}"
    );
    assert_eq!(records.len(), 2);
    assert!(records[1].get("body").is_none());
}

#[test]
fn workflow_purpose_uses_actual_operation_and_segment_facts() {
    use plugin_framework::provider_contract::{ProviderNativeTransport, ProviderWireOperation};
    let context = control_plane_contracts::ports::WorkflowObservationContext {
        client_request_id: Uuid::now_v7(),
        context_flow_run_id: Some(Uuid::now_v7()),
        context_response_id: Some("resp-A".into()),
        is_resume: true,
    };
    let mut input = ProviderInvocationInput::default();
    input
        .run_context
        .insert("request_kind".into(), json!("prewarm"));
    assert_eq!(invocation_purpose(&input, None), "generate");
    assert_eq!(invocation_purpose(&input, Some(&context)), "tool_resume");
    input.native_transport = Some(ProviderNativeTransport {
        protocol: "openai.responses".into(),
        digest: "digest".into(),
        size_bytes: 20,
        wire_body: json!({"generate":false}),
    });
    assert_eq!(invocation_purpose(&input, None), "prewarm");
    input.native_transport.as_mut().unwrap().wire_body = json!({
        "input":[{"type":"function_call_output", "call_id":"history", "output":"old"}],
        "client_metadata":{"request_kind":"prewarm"}
    });
    assert_eq!(invocation_purpose(&input, None), "generate");
    input.operation = ProviderWireOperation::Compact;
    assert_eq!(invocation_purpose(&input, Some(&context)), "compact");
    input.operation = ProviderWireOperation::CountTokens;
    assert_eq!(invocation_purpose(&input, Some(&context)), "unknown");
}

#[test]
fn workflow_identity_keeps_current_trigger_distinct_from_previous_context_on_all_facts() {
    let (mut capture, receiver, _) = fixture(16);
    let trigger = Uuid::now_v7();
    let previous_flow = Uuid::now_v7();
    let sink = capture.sink.as_mut().unwrap();
    sink.id.observation_context =
        Some(control_plane_contracts::ports::WorkflowObservationContext {
            client_request_id: trigger,
            context_flow_run_id: Some(previous_flow),
            context_response_id: Some("resp-A".into()),
            is_resume: true,
        });
    sink.id.purpose = "tool_resume";
    let id = sink.id.clone();
    record_input(sink, &ProviderInvocationInput::default());
    capture.finish(None, Some("upstream error"), false);
    let mut facts = records(receiver);
    facts.push(
        id.event(
            "native_trajectory_integrity",
            json!({"status":"incomplete"}),
        )
        .payload,
    );
    assert!(facts.iter().any(|fact| fact["kind"] == "error"));
    for fact in facts {
        assert_eq!(fact["trigger_request_id"], json!(trigger));
        assert_eq!(fact["context_flow_run_id"], json!(previous_flow));
        assert_eq!(fact["context_response_id"], "resp-A");
        assert_eq!(fact["purpose"], "tool_resume");
    }
    let (capture, _, _) = fixture(1);
    let event = capture.sink.as_ref().unwrap().id.event("test", json!({}));
    assert!(event.payload["trigger_request_id"].is_null());
    assert!(event.payload["context_flow_run_id"].is_null());
}

#[test]
fn compact_observation_preserves_typed_result_and_real_elapsed_duration() {
    use plugin_framework::provider_contract::{
        ProviderCompactProfile, ProviderCompactResult, ProviderWireOperation,
    };
    let (mut capture, receiver, _) = fixture(8);
    capture.started_at = Some(std::time::Instant::now() - std::time::Duration::from_millis(25));
    capture.sink.as_mut().unwrap().id.purpose = "compact";
    let result = ProviderCompactResult::ResponseItems {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompact,
        response_items: vec![json!({"type":"compaction", "encrypted_content":"opaque"})],
    };
    capture.finish_compact(Some(&result), false);
    let facts = records(receiver);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["purpose"], "compact");
    assert!(facts[0]["duration_ms"].as_u64().unwrap() >= 25);
    assert_eq!(body(&facts[0]), serde_json::to_value(result).unwrap());
    assert!(body(&facts[0]).get("final_content").is_none());
}
