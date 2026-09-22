use super::*;
#[derive(Default)]
struct MemoryWriter {
    records: Mutex<Vec<AppendClientTrajectoryInput>>,
    timeout_once: AtomicBool,
    fail_once: AtomicBool,
}
#[async_trait::async_trait]
impl FactWriter for MemoryWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        if self.timeout_once.swap(false, Ordering::AcqRel) {
            tokio::time::sleep(WRITE_TIMEOUT + Duration::from_millis(100)).await;
        }
        if self.fail_once.swap(false, Ordering::AcqRel) {
            anyhow::bail!("fixture failed persist");
        }
        self.records.lock().unwrap().push(input.clone());
        Ok(())
    }
}
fn capture(writer: Arc<MemoryWriter>) -> ClientTrajectoryRecorder {
    ClientTrajectoryRecorder::with_writer(writer, ClientTrajectoryTransport::Http)
}
fn raw(records: &[AppendClientTrajectoryInput], direction: &str) -> Vec<u8> {
    records
        .iter()
        .filter_map(|record| match &record.fact {
            ClientTrajectoryFact::Section { section, value, .. }
                if section == "raw" && value["direction"] == direction =>
            {
                Some(if value["encoding"] == "base64" {
                    base64::engine::general_purpose::STANDARD
                        .decode(value["body"].as_str().unwrap())
                        .unwrap()
                } else {
                    value["body"].as_str().unwrap().as_bytes().to_vec()
                })
            }
            _ => None,
        })
        .flatten()
        .collect()
}
fn complete(records: &[AppendClientTrajectoryInput]) -> bool {
    matches!(&records.last().unwrap().fact,ClientTrajectoryFact::Integrity {status,dropped_count:0,persist_failed_count:0} if status=="complete")
}

#[tokio::test]
async fn exact_raw_unicode_whitespace_and_user_credential_named_fields_survive() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    let request=" {\n \"model\": \"x\", \"instructions\":\"system\", \"input\":[{\"role\":\"user\",\"content\":\"你好 🌍\",\"token\":\"user token\",\"password\":\"user password\"}] } \n";
    // Queue before a run is bound; arbitrary UTF-8 splits must remain lossless.
    for chunk in request.as_bytes().chunks(7) {
        recorder.record(ClientTrajectoryFrameKind::Request, chunk);
    }
    recorder.bind_run(Uuid::now_v7(), None);
    let response=b" {\"object\":\"response\",\"id\":\"resp-1\",\"output\":[],\"usage\":{\"input_tokens\":1}} ";
    recorder.record(ClientTrajectoryFrameKind::ResponseJson, response);
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert_eq!(raw(&records, "submitted"), request.as_bytes());
    assert_eq!(raw(&records, "emitted"), response);
    assert!(complete(&records));
    assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Section {section,value,..} if section=="overview"&&value["token"]=="user token"&&value["password"]=="user password")));
}

#[tokio::test]
async fn sse_chunks_and_completed_output_deduplicate_real_calls_and_keep_schema_and_submitted_results(
) {
    let mut decoder = decode::Decoder::default();
    let mut classifier = classify::Classifier::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        ClientTrajectoryTransport::Http,
    );
    let at = "2026-09-22T00:00:00Z";
    let mut facts = classifier.begin_request(at).await;
    facts.extend(classifier.observe(ClientTrajectoryFrameKind::Request,json!({"instructions":"Be helpful","tools":[{"type":"function","name":"weather","parameters":{"properties":{"token":{"type":"string"}}}}],"input":[{"type":"function_call","id":"old-call","call_id":"call-old","name":"weather","arguments":"{\"city\":\"Paris\"}"},{"type":"function_call_output","call_id":"call-old","output":"sunny"},{"role":"user","content":"Now 北京"}]}),at).await);
    let item = json!({"type":"function_call","id":"item-1","call_id":"call-1","name":"weather","arguments":"{\"city\":\"北京\"}"});
    let done = json!({"type":"response.output_item.done","output_index":0,"item":item});
    let completed = json!({"type":"response.completed","response":{"id":"resp-1","output":[item],"usage":{"output_tokens":4}}});
    let wire=format!(": heartbeat\r\nevent: response.output_item.done\r\ndata: {done}\r\n\r\nevent: response.completed\ndata: {completed}\n\ndata: [DONE]\n\n");
    for byte in wire.as_bytes() {
        for value in decoder.feed(ClientTrajectoryFrameKind::ResponseSse, &[*byte]) {
            facts.extend(
                classifier
                    .observe(ClientTrajectoryFrameKind::ResponseSse, value, at)
                    .await,
            );
        }
    }
    decoder.finish();
    assert!(!decoder.incomplete);
    assert!(classifier.completed);
    assert!(!classifier.incomplete);
    let steps: Vec<_> = facts
        .iter()
        .filter_map(|f| {
            if let ClientTrajectoryFact::Step { step } = f {
                Some(step)
            } else {
                None
            }
        })
        .collect();
    let emitted: Vec<_> = steps
        .iter()
        .filter(|s| s.origin == "emitted" && s.category == "tool_call")
        .collect();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].call_id.as_deref(), Some("call-1"));
    assert!(emitted[0].available_sections.iter().any(|s| s == "schema"));
    let submitted = steps
        .iter()
        .find(|s| s.result_preview.as_deref() == Some("sunny"))
        .unwrap();
    assert_eq!(submitted.origin, "submitted");
    assert_eq!(submitted.status, "submitted");
    assert!(submitted.related_step_id.is_some());
    assert!(steps.iter().any(|s| s.category == "system"));
    assert!(steps.iter().any(|s| s.category == "usage"));
    for fact in facts {
        if let ClientTrajectoryFact::Section { section, value, .. } = fact {
            if section == "timing" {
                assert_eq!(value, json!({"observed_at":at}));
            }
        }
    }
}

#[tokio::test]
async fn overflow_unbound_discard_drop_and_persist_timeout_are_nonblocking_and_incomplete() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder.record(
        ClientTrajectoryFrameKind::Request,
        &vec![b' '; QUEUE_BYTES + FRAME_BYTES],
    );
    assert!(recorder.owner.state.dropped.load(Ordering::Relaxed) > 0);
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.finish();
    recorder.wait_finished().await;
    assert!(!complete(&writer.records.lock().unwrap()));
    let unbound = Arc::new(MemoryWriter::default());
    let recorder = capture(unbound.clone());
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
    recorder.finish();
    recorder.wait_finished().await;
    assert!(unbound.records.lock().unwrap().is_empty());
    let dropped = Arc::new(MemoryWriter::default());
    let recorder = capture(dropped.clone());
    recorder.bind_run(Uuid::now_v7(), None);
    let state = recorder.owner.state.clone();
    drop(recorder);
    loop {
        let wake = state.stopped_notify.notified();
        tokio::pin!(wake);
        wake.as_mut().enable();
        if state.stopped.load(Ordering::Acquire) {
            break;
        }
        wake.await;
    }
    assert!(!complete(&dropped.records.lock().unwrap()));
    let slow = Arc::new(MemoryWriter::default());
    slow.timeout_once.store(true, Ordering::Release);
    let recorder = capture(slow.clone());
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
    recorder.record(
        ClientTrajectoryFrameKind::ResponseJson,
        b"{\"object\":\"response\",\"output\":[]}",
    );
    recorder.finish();
    recorder.wait_finished().await;
    assert!(!complete(&slow.records.lock().unwrap()));
}

#[tokio::test]
async fn long_stream_releases_queue_budget_and_json_chunks_are_incremental() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
    // Total wire size exceeds the queue budget, but steady draining must not truncate it.
    let chunk = format!(":{}\n\n", "x".repeat(FRAME_BYTES / 2));
    for _ in 0..80 {
        recorder.record(ClientTrajectoryFrameKind::ResponseSse, chunk.as_bytes());
        while recorder.owner.bytes.available_permits() < QUEUE_BYTES {
            tokio::task::yield_now().await;
        }
    }
    let tail = "data: {\"type\":\"response.completed\",\"response\":{\"output\":[]}}\n\n";
    recorder.record(ClientTrajectoryFrameKind::ResponseSse, tail.as_bytes());
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    assert_eq!(
        raw(&records, "emitted").len(),
        chunk.len() * 80 + tail.len()
    );
    let mut decoder = decode::Decoder::default();
    let mut values = vec![];
    for byte in "{\"object\":\"response\",\"text\":\"🌍\"}".as_bytes() {
        values.extend(decoder.feed(ClientTrajectoryFrameKind::ResponseJson, &[*byte]));
    }
    decoder.finish();
    assert!(!decoder.incomplete);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["text"], "🌍");
}

#[tokio::test]
async fn classifications_preserve_unknown_and_actual_request_labels() {
    let mut classifier = classify::Classifier::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        ClientTrajectoryTransport::Websocket,
    );
    classifier.begin_request("2026-09-22T00:00:00Z").await;
    let facts=classifier.observe(ClientTrajectoryFrameKind::Request,json!({"model":"actual-model","reasoning":{"effort":"high"},"stream":true,"input":[{"type":"future_item","data":"opaque"}]}),"2026-09-22T00:00:01Z").await;
    let steps: Vec<_> = facts
        .iter()
        .filter_map(|fact| {
            if let ClientTrajectoryFact::Step { step } = fact {
                Some(step)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        steps[0].preview,
        "actual-model · reasoning.effort=high · stream=true"
    );
    assert_eq!(steps[0].created_at, "2026-09-22T00:00:00Z");
    assert_eq!(steps[1].category, "unknown");
    assert!(steps[1].turn_id.is_none());
    assert!(steps[1].node_run_id.is_none());
}

#[tokio::test]
async fn codex_custom_tools_preserve_freeform_input_content_parts_and_actual_schema() {
    let mut classifier = classify::Classifier::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        ClientTrajectoryTransport::Http,
    );
    let at = "2026-09-22T00:00:00Z";
    classifier.begin_request(at).await;
    let patch = "*** Begin Patch\n*** Add File: 北京.txt\n+hello\n*** End Patch";
    let schema = json!({"type":"custom","name":"apply_patch","format":{"type":"grammar","syntax":"lark","definition":"start: /.+/"}});
    let output = json!([{"type":"input_text","text":"Success. Updated 北京.txt"}]);
    let facts = classifier.observe(ClientTrajectoryFrameKind::Request, json!({
        "tools":[schema], "input":[
            {"type":"custom_tool_call","id":"ctc-original","call_id":"call-patch","name":"apply_patch","input":patch},
            {"type":"custom_tool_call_output","call_id":"call-patch","output":output}
        ]
    }), at).await;
    let steps: Vec<_> = facts
        .iter()
        .filter_map(|fact| match fact {
            ClientTrajectoryFact::Step { step } => Some(step),
            _ => None,
        })
        .collect();
    let call = steps
        .iter()
        .find(|step| step.category == "tool_call")
        .unwrap();
    let result = steps
        .iter()
        .find(|step| step.category == "tool_result")
        .unwrap();
    assert_eq!(call.call_id.as_deref(), Some("call-patch"));
    assert_eq!(call.item_id.as_deref(), Some("ctc-original"));
    assert_eq!(call.origin, "submitted");
    assert_eq!(result.origin, "submitted");
    assert_eq!(result.related_step_id, Some(call.id));
    for (id, section, expected) in [
        (call.id, "parameters", json!(patch)),
        (call.id, "schema", schema),
        (result.id, "result", output),
    ] {
        assert!(facts.iter().any(|fact|matches!(fact,ClientTrajectoryFact::Section {step_id,section:actual,value} if *step_id==id && actual==section && *value==expected)));
    }
}

mod namespaces;

mod admission;

#[async_trait::async_trait]
impl classify::FactSink for Vec<ClientTrajectoryFact> {
    async fn push(&mut self, fact: ClientTrajectoryFact) {
        Vec::push(self, fact);
    }
}
impl classify::Classifier {
    async fn begin_request(&mut self, at: &str) -> Vec<ClientTrajectoryFact> {
        let mut facts = Vec::new();
        self.begin_request_into(at, &mut facts).await;
        facts
    }
    async fn observe(
        &mut self,
        kind: ClientTrajectoryFrameKind,
        value: serde_json::Value,
        at: &str,
    ) -> Vec<ClientTrajectoryFact> {
        let mut facts = Vec::new();
        self.observe_into(kind, value, at, &mut facts).await;
        facts
    }
}
mod incremental;

#[tokio::test]
async fn empty_prewarm_keeps_response_identity_without_fabricating_output() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(
        ClientTrajectoryFrameKind::Request,
        br#"{"generate":false,"input":[]}"#,
    );
    recorder.record(
        ClientTrajectoryFrameKind::ResponseJson,
        br#"{"id":"resp-prewarm","object":"response","status":"completed","output":[]}"#,
    );
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::ResponseLink{response_id} if response_id=="resp-prewarm")));
    assert!(!records
        .iter()
        .any(|r| matches!(&r.fact,ClientTrajectoryFact::Step{step} if step.origin=="emitted")));
}
