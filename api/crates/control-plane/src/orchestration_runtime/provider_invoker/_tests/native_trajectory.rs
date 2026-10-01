use super::*;
use plugin_framework::provider_contract::{
    ProviderInvocationResult, ProviderRuntimeError, ProviderRuntimeErrorKind, ProviderToolCall,
};

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
                observed: Arc::new(AtomicU64::new(0)),
                dropped: Arc::new(AtomicU64::new(0)),
            }),
            aggregate: Default::default(),
            completion: Some(done),
            started_at: Some(std::time::Instant::now()),
            writer: None,
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
#[tokio::test]
async fn chunks_are_not_steps_and_result_does_not_duplicate_reply_or_tool() {
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
        capture
            .finish(
                Some(&ProviderInvocationResult {
                    final_content: Some("hello".into()),
                    tool_calls: vec![call],
                    ..Default::default()
                }),
                None,
                true,
            )
            .await;
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
#[tokio::test]
async fn result_only_preserves_unknown_metadata_without_promoting_fake_steps_or_credentials() {
    let (capture, receiver, _) = fixture(16);
    capture.finish(Some(&ProviderInvocationResult { final_content: Some("reply".into()), provider_metadata: json!({"future_vendor":{"opaque":[1,{"x":true}],"authorization":"Bearer secret","headers":{"x":"secret"}},"pretend_tool_call":{"id":"not-a-native-call"}}), ..Default::default() }), None, true).await;
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
#[tokio::test]
async fn native_output_items_capture_tools_and_unknown_content_without_raw() {
    let (capture, receiver, _) = fixture(16);
    let observer = capture.observer();
    for (index, item) in [json!({"type":"function_call","id":"item-1","call_id":"call-1","name":"search","arguments":"{}"}),json!({"type":"vendor_future_content","future":{"data":[42]}})].into_iter().enumerate() {
        observer.observe(&ProviderStreamEvent::OutputItem { phase: ProviderOutputItemPhase::Added, output_index: index, item: item.clone() });
        observer.observe(&ProviderStreamEvent::OutputItem { phase: ProviderOutputItemPhase::Done, output_index: index, item });
    }
    capture.finish(None, None, true).await;
    let records = records(receiver);
    assert_eq!(records.len(), 2);
    assert_eq!(
        body(&records[0])["output_items"][0]["future"]["data"][0],
        42
    );
    assert_eq!(records[1]["tool_call_id"], "call-1");
}
#[tokio::test]
async fn unfinished_items_remain_incomplete_but_large_complete_content_survives() {
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
        capture.finish(None, None, true).await;
        assert_eq!(completion.await.unwrap(), oversized);
        let records = records(receiver);
        assert_eq!(
            records
                .iter()
                .any(|record| record["kind"] == "observation_gap"),
            !oversized
        );
        if oversized {
            assert_eq!(
                body(&records[0])["final_content"].as_str().unwrap().len(),
                CAPACITY
            );
        }
    }
}
#[tokio::test]
async fn sidecar_counts_successful_admission_and_marks_queue_loss_cancel_and_write_failure() {
    for (cancel, fail, capacity, expected) in [
        (false, false, 16, "complete"),
        (true, false, 16, "incomplete"),
        (false, true, 16, "incomplete"),
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
        )
        .await;
        if cancel {
            drop(capture);
        } else {
            capture.finish(None, Some("upstream error"), true).await;
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
#[tokio::test]
async fn serialization_preserves_large_actual_values() {
    assert!(snapshot_value(&json!({"huge":"x".repeat(CAPACITY)})).is_some());
    assert!(snapshot_value(&json!({"small":"ok"})).is_some());
}

#[tokio::test]
async fn native_continuation_keeps_sealed_request_and_tool_results_without_auth_context() {
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
    assert!(!record_input(capture.sink.as_ref().unwrap(), &input).await);
    drop(capture);
    let records = records(receiver);
    assert_eq!(records.len(), 1);
    let entry = &records[0]["_context_occurrences"]["entries"][0];
    assert_eq!(entry["metadata"]["kind"], "tool_result");
    assert_eq!(
        entry["metadata"]["body_ref"]["pointer"],
        "/native_request/wire_body/input/0"
    );
    assert_eq!(
        entry["metadata"]["body_ref"]["step_key"],
        records[0]["step_key"]
    );
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
}
#[tokio::test]
async fn error_only_has_complete_capture_without_invented_reply() {
    let (capture, receiver, completion) = fixture(16);
    capture.finish(None, Some("failed"), true).await;
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
        capture.finish(Some(&result), None, true).await;
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

#[tokio::test]
async fn responses_delta_cannot_open_or_reopen_output_items() {
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

#[tokio::test]
async fn preview_uses_native_content_and_tool_name_with_unicode_limit() {
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

#[tokio::test]
async fn large_request_retains_full_input_and_tool_result_occurrence() {
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
    assert!(!record_input(capture.sink.as_ref().unwrap(), &input).await);
    let records = records(receiver);
    assert_eq!(records[0]["status"], "recorded");
    assert_eq!(records.len(), 1);
    assert_eq!(
        body(&records[0])["native_request"]["wire_body"]["input"][0]["output"],
        "kept"
    );
}

#[tokio::test]
async fn different_stream_tool_evidence_is_not_replaced_by_result_with_same_id() {
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
    capture
        .finish(
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
        )
        .await;
    let records = records(receiver);
    let tool = records.iter().find(|r| r["kind"] == "tool_call").unwrap();
    assert!(tool.get("body_ref").is_none());
    assert_eq!(body(tool)["arguments"]["q"], "stream");
    assert_eq!(
        body(&records[0])["result"]["tool_calls"][0]["arguments"]["q"],
        "final"
    );
}

#[tokio::test]
async fn repeated_large_reply_and_tool_are_stored_once_per_snapshot() {
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
    capture.finish(Some(&result), None, true).await;
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

#[tokio::test]
async fn workflow_purpose_uses_actual_operation_and_segment_facts() {
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
        protocol: "openai_responses".into(),
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

#[tokio::test]
async fn workflow_identity_keeps_current_trigger_distinct_from_previous_context_on_all_facts() {
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
    record_input(sink, &ProviderInvocationInput::default()).await;
    capture.finish(None, Some("upstream error"), false).await;
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

#[tokio::test]
async fn compact_observation_preserves_typed_result_and_real_elapsed_duration() {
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
    capture.finish_compact(Some(&result), false).await;
    let facts = records(receiver);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["purpose"], "compact");
    assert!(facts[0]["duration_ms"].as_u64().unwrap() >= 25);
    assert_eq!(body(&facts[0]), serde_json::to_value(result).unwrap());
    assert!(body(&facts[0]).get("final_content").is_none());
}

#[tokio::test]
async fn interrupted_native_output_keeps_each_item_and_call_identity() {
    let (capture, receiver, completion) = fixture(16);
    let observer = capture.observer();
    for (index, id) in [(0, "m0"), (1, "m1")] {
        observer.observe(&ProviderStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: index,
            item: json!({"id":id,"type":"message","future":{"opaque":true}}),
        });
        observer.observe(&ProviderStreamEvent::ResponsesOutputDelta {
            event: json!({"type":"response.output_text.delta","output_index":index,"item_id":id,"content_index":0,"delta":format!("partial-{id}")}),
        });
    }
    for id in ["c0", "c1"] {
        observer.observe(&ProviderStreamEvent::ToolCallDelta {
            call_id: id.into(),
            delta: json!({"arguments":"{\"q\":","name":"lookup"}),
        });
        observer.observe(&ProviderStreamEvent::ToolCallDelta {
            call_id: id.into(),
            delta: json!({"arguments":format!("\"{id}\"")}),
        });
    }
    observer.observe(&ProviderStreamEvent::ReasoningSignatureDelta {
        signature: "signature".into(),
    });
    capture.finish(None, Some("interrupted"), true).await;
    assert!(!completion.await.unwrap());
    let events = records(receiver);
    let reply = body(
        events
            .iter()
            .find(|event| event["kind"] == "model_reply")
            .unwrap(),
    );
    assert_eq!(reply["partial_output_items"].as_array().unwrap().len(), 2);
    assert_eq!(reply["partial_content"][0]["text"], "partial-m0");
    assert_eq!(reply["partial_content"][1]["text"], "partial-m1");
    assert_eq!(reply["partial_tool_calls"][0]["call_id"], "c0");
    assert_eq!(
        reply["partial_tool_calls"][1]["delta"]["arguments"],
        "{\"q\":\"c1\""
    );
    assert_eq!(reply["reasoning_signature"], "signature");
    assert_eq!(reply["partial_output_items"][0]["future"]["opaque"], true);
}

#[tokio::test]
async fn semantic_handoff_waits_for_bounded_queue_instead_of_dropping_large_body() {
    let (capture, mut receiver, _) = fixture(1);
    let sink = capture.sink.as_ref().unwrap();
    sink.step("model_call", "input", "prepared", json!({}), None, false)
        .await;
    let mut handoff = Box::pin(sink.step(
        "model_reply",
        "reply",
        "received",
        json!({"final_content":"x".repeat(CAPACITY + 1)}),
        None,
        false,
    ));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(1), &mut handoff)
            .await
            .is_err()
    );
    receiver.recv().await.unwrap();
    assert!(handoff.await);
    let record = receiver.recv().await.unwrap();
    assert_eq!(
        body(&record.payload.payload)["final_content"]
            .as_str()
            .unwrap()
            .len(),
        CAPACITY + 1
    );
    assert_eq!(sink.dropped.load(Relaxed), 0);
}

#[tokio::test]
async fn signature_only_failure_preserves_actual_reasoning_signature() {
    let (capture, receiver, completion) = fixture(16);
    capture
        .observer()
        .observe(&ProviderStreamEvent::ReasoningSignatureDelta {
            signature: "opaque-signature-only".into(),
        });
    capture
        .finish(None, Some("failed before content"), false)
        .await;
    assert!(!completion.await.unwrap());
    let events = records(receiver);
    let reply = events
        .iter()
        .find(|event| event["kind"] == "model_reply")
        .expect("signature is actual reply evidence even without visible content");
    assert_eq!(body(reply)["reasoning_signature"], "opaque-signature-only");
    assert!(events.iter().any(|event| event["kind"] == "error"));
}

#[tokio::test]
async fn orphan_partial_content_failure_preserves_reply_and_explicit_gap() {
    let (capture, receiver, completion) = fixture(16);
    capture
        .observer()
        .observe(&ProviderStreamEvent::ResponsesOutputDelta {
            event: json!({"type":"response.output_text.delta","output_index":7,
                     "item_id":"orphan-item","content_index":2,"delta":"partial-without-added"}),
        });
    capture
        .finish(None, Some("failed without added item"), false)
        .await;
    assert!(!completion.await.unwrap());
    let events = records(receiver);
    let reply = events
        .iter()
        .find(|event| event["kind"] == "model_reply")
        .expect("partial content cannot disappear when its Added fact is absent");
    assert_eq!(reply["status"], "incomplete");
    let content = &body(reply)["partial_content"][0];
    assert_eq!(content["output_index"], 7);
    assert_eq!(content["content_index"], 2);
    assert_eq!(content["text"], "partial-without-added");
    assert!(events
        .iter()
        .any(|event| event["kind"] == "observation_gap"));
}

#[tokio::test]
async fn slow_admitted_body_completes_without_local_deadline_cancellation() {
    let (mut capture, receiver, completion) = fixture(1);
    let sink = capture.sink.as_ref().unwrap();
    let (id, observed, dropped) = (sink.id.clone(), sink.observed.clone(), sink.dropped.clone());
    let persisted = Arc::new(std::sync::Mutex::new(Vec::new()));
    let written = persisted.clone();
    capture.writer = Some(tokio::spawn(async move {
        write(
            id,
            receiver,
            observed,
            dropped,
            completion,
            move |payload| {
                let written = written.clone();
                async move {
                    if payload.event_type == "provider_semantic_step" {
                        // Exceeds the obsolete two-second local deadline exactly once.
                        tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
                    }
                    written.lock().unwrap().push(payload);
                    true
                }
            },
        )
        .await;
    }));
    capture.observer().observe(&ProviderStreamEvent::TextDelta {
        delta: "slow immutable reply".into(),
    });
    let started = std::time::Instant::now();
    capture.finish(None, None, true).await;
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(2100),
        "finish must wait for the admitted snapshot writer"
    );
    let persisted = persisted.lock().unwrap();
    let reply = persisted
        .iter()
        .find(|event| event.event_type == "provider_semantic_step")
        .expect("a slow repository must not erase an admitted snapshot");
    assert_eq!(
        body(&reply.payload)["final_content"],
        "slow immutable reply"
    );
    let integrity = &persisted.last().unwrap().payload;
    assert_eq!(integrity["status"], "complete");
    assert_eq!(integrity["persist_failed_count"], 0);
    assert_eq!(integrity["dropped_count"], 0);
}

#[tokio::test]
async fn repeated_context_preserves_order_distinct_call_identities_and_result_versions() {
    let inputs = [
        json!([
            {"type":"function_call_output","call_id":"branch-a","output":"same"},
            {"type":"function_call_output","call_id":"branch-b","output":"same"},
            {"role":"user","content":"between"},
            {"type":"function_call_output","call_id":"branch-a","output":"updated"}
        ]),
        json!([
            {"type":"function_call_output","call_id":"branch-b","output":"same"},
            {"type":"function_call_output","call_id":"branch-a","output":"same"}
        ]),
    ];
    for expected in inputs {
        let input = ProviderInvocationInput {
            native_transport: Some(
                plugin_framework::provider_contract::ProviderNativeTransport {
                    protocol: "openai.responses".into(),
                    digest: "d".into(),
                    size_bytes: 0,
                    wire_body: json!({"input":expected}),
                },
            ),
            ..Default::default()
        };
        let (capture, receiver, _) = fixture(16);
        assert!(!record_input(capture.sink.as_ref().unwrap(), &input).await);
        let records = records(receiver);
        assert_eq!(
            records.len(),
            1,
            "one journal owner must retain all context occurrences"
        );
        assert_eq!(
            body(&records[0])["native_request"]["wire_body"]["input"],
            expected
        );
    }
}

#[tokio::test]
async fn upstream_error_observation_preserves_message_and_original_details() {
    let (capture, receiver, _) = fixture(16);
    let details = json!({"upstream_error":{"message":"rejected\nnext line","code":"unknown_vendor_code","type":null,"future":{"authorization":"vendor diagnostic"}},"raw_body":"original body\n","semantic_terminal":true});
    let error = ProviderRuntimeError {
        kind: ProviderRuntimeErrorKind::ProviderUpstreamError,
        message: "rejected\nnext line".into(),
        provider_summary: None,
        provider_details: Some(details.clone()),
    };
    capture
        .observer()
        .observe(&ProviderStreamEvent::Error { error });
    capture.finish(None, None, true).await;
    let records = records(receiver);
    let error_record = records
        .iter()
        .find(|record| record["kind"] == "error")
        .unwrap();
    let observed = body(error_record);
    assert_eq!(observed["message"], "rejected\nnext line");
    assert_eq!(observed["provider_details"], details);
}
