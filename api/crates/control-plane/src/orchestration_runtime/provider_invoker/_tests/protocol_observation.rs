use super::*;

#[test]
fn capture_requires_explicit_end_and_successful_persistence() {
    let id = Uuid::now_v7();
    let mut capture = Capture {
        node: Some(("model".into(), Uuid::now_v7())),
        invocation_id: id,
        provider_attempt_index: 2,
        observed_count: 0,
        persist_failed_count: 0,
        ended: false,
        dropped_count: 0,
        capture_failed: false,
    };
    let run = Uuid::now_v7();
    assert_eq!(
        capture.integrity(run, true).unwrap().payload["status"],
        "unavailable"
    );
    let event = |kind: &str| ProviderStreamEvent::ProtocolObservation {
        protocol: "openai.responses".into(),
        transport: "sse".into(),
        direction: "received".into(),
        kind: kind.into(),
        body: "data: [DONE]\n\n".into(),
        encoding: "utf8".into(),
        status: None,
    };
    let first = capture.observe(Some(run), &event("response_body")).unwrap();
    assert!(first.persist_required);
    assert_eq!(first.payload["invocation_id"], json!(id));
    assert_eq!(first.payload["sequence"], 1);
    assert_eq!(
        capture.integrity(run, true).unwrap().payload["status"],
        "incomplete"
    );
    capture.observe(Some(run), &event("stream_end"));
    assert_eq!(
        capture.integrity(run, true).unwrap().payload["status"],
        "complete"
    );
    assert_eq!(
        capture.integrity(run, false).unwrap().payload["status"],
        "incomplete"
    );
    capture.persist_failed_count += 1;
    assert_eq!(
        capture.integrity(run, true).unwrap().payload["status"],
        "incomplete"
    );
}

fn event(body: String) -> ProviderStreamEvent {
    ProviderStreamEvent::ProtocolObservation {
        protocol: "openai.responses".into(),
        transport: "sse".into(),
        direction: "received".into(),
        kind: "response_body".into(),
        body,
        encoding: "utf8".into(),
        status: None,
    }
}

#[tokio::test]
async fn full_queue_and_byte_budget_drop_without_waiting_for_writer() {
    use runtime_core::runtime_backend::RuntimeProtocolObservationSink;
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    let (sender, mut receiver) = mpsc::channel(1);
    let dropped = Arc::new(AtomicU64::new(0));
    let sink = Sink {
        sender,
        bytes: Arc::new(tokio::sync::Semaphore::new(BYTE_CAPACITY)),
        dropped: dropped.clone(),
        sequence: AtomicU64::new(0),
    };
    sink.observe(event("first".into()));
    sink.observe(event("queue overflow".into()));
    assert_eq!(dropped.load(Relaxed), 1);
    // Simulate a writer holding an observation while its database operation is stalled.
    let in_flight = receiver.recv().await.unwrap();
    sink.observe(event("x".repeat(BYTE_CAPACITY)));
    assert_eq!(dropped.load(Relaxed), 2);
    sink.observe(event("business can keep moving".into()));
    assert_eq!(receiver.recv().await.unwrap().sequence, 4);
    drop(in_flight);
    assert_eq!(sink.bytes.available_permits(), BYTE_CAPACITY);
    drop(receiver);
    sink.observe(event("closed writer".into()));
    assert_eq!(dropped.load(Relaxed), 3);
}

#[tokio::test]
async fn completion_drop_reports_cancel_and_success_is_explicit() {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    drop(Completion(Some(sender)));
    assert!(!receiver.await.unwrap());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    Completion(Some(sender)).finish(true);
    assert!(receiver.await.unwrap());
}

#[test]
fn loss_without_accepted_events_is_incomplete_and_sdk_loss_cannot_be_complete() {
    let run = Uuid::now_v7();
    let mut capture = Capture {
        node: Some(("model".into(), Uuid::now_v7())),
        invocation_id: Uuid::now_v7(),
        provider_attempt_index: 0,
        observed_count: 0,
        persist_failed_count: 0,
        ended: false,
        dropped_count: 1,
        capture_failed: false,
    };
    assert_eq!(
        capture.integrity(run, true).unwrap().payload["status"],
        "incomplete"
    );
    let mut loss = event(r#"{"dropped_count":7,"reason":"observation_capacity_exceeded"}"#.into());
    if let ProviderStreamEvent::ProtocolObservation { kind, .. } = &mut loss {
        *kind = "capture_integrity".into();
    }
    capture.observe(Some(run), &loss);
    capture.ended = true;
    let integrity = capture.integrity(run, true).unwrap();
    assert_eq!(integrity.payload["status"], "incomplete");
    assert_eq!(integrity.payload["dropped_count"], 8);
}

#[tokio::test]
async fn stalled_persistence_does_not_block_ingress_or_completion_and_writer_exits() {
    use runtime_core::runtime_backend::RuntimeProtocolObservationSink;
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    let run = Uuid::now_v7();
    let capture = Capture {
        node: Some(("model".into(), Uuid::now_v7())),
        invocation_id: Uuid::now_v7(),
        provider_attempt_index: 0,
        observed_count: 0,
        persist_failed_count: 0,
        ended: false,
        dropped_count: 0,
        capture_failed: false,
    };
    let (sender, receiver) = mpsc::channel(1);
    let dropped = Arc::new(AtomicU64::new(0));
    let sink = Sink {
        sender,
        bytes: Arc::new(tokio::sync::Semaphore::new(BYTE_CAPACITY)),
        dropped: dropped.clone(),
        sequence: AtomicU64::new(0),
    };
    let (done, completion) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(write(
        run,
        capture,
        receiver,
        dropped.clone(),
        completion,
        |_| async { std::future::pending::<bool>().await },
    ));
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        sink.observe(event("accepted".into()));
        sink.observe(event("overflow".into()));
        Completion(Some(done)).finish(true);
    })
    .await
    .expect("business completion must not wait for persistence");
    assert_eq!(dropped.load(Relaxed), 1);
    tokio::time::timeout(std::time::Duration::from_secs(12), task)
        .await
        .expect("writer must release its task and bytes after bounded drain")
        .unwrap();
    assert_eq!(sink.bytes.available_permits(), BYTE_CAPACITY);
}

#[tokio::test]
async fn writer_final_integrity_distinguishes_success_cancel_and_persist_failure() {
    use runtime_core::runtime_backend::RuntimeProtocolObservationSink;
    use std::sync::atomic::AtomicU64;
    for (success, fail_raw, expected) in [
        (true, false, "complete"),
        (false, false, "incomplete"),
        (true, true, "incomplete"),
    ] {
        let run = Uuid::now_v7();
        let capture = Capture {
            node: Some(("model".into(), Uuid::now_v7())),
            invocation_id: Uuid::now_v7(),
            provider_attempt_index: 0,
            observed_count: 0,
            persist_failed_count: 0,
            ended: false,
            dropped_count: 0,
            capture_failed: false,
        };
        let (sender, receiver) = mpsc::channel(2);
        let dropped = Arc::new(AtomicU64::new(0));
        let sink = Sink {
            sender,
            bytes: Arc::new(tokio::sync::Semaphore::new(BYTE_CAPACITY)),
            dropped: dropped.clone(),
            sequence: AtomicU64::new(0),
        };
        let mut end = event(String::new());
        if let ProviderStreamEvent::ProtocolObservation { kind, .. } = &mut end {
            *kind = "stream_end".into();
        }
        sink.observe(end);
        let (done, completion) = tokio::sync::oneshot::channel();
        if success {
            Completion(Some(done)).finish(true);
        } else {
            drop(Completion(Some(done)));
        }
        let written = Arc::new(Mutex::new(Vec::new()));
        let written_for_task = written.clone();
        write(
            run,
            capture,
            receiver,
            dropped,
            completion,
            move |payload| {
                let written = written_for_task.clone();
                async move {
                    if fail_raw && payload.event_type == "provider_protocol_observation" {
                        return false;
                    }
                    written.lock().unwrap().push(payload);
                    true
                }
            },
        )
        .await;
        let written = written.lock().unwrap();
        let final_integrity = written
            .iter()
            .rev()
            .find(|event| event.event_type == "provider_protocol_integrity")
            .unwrap();
        assert_eq!(final_integrity.payload["status"], expected);
        if fail_raw {
            assert_eq!(final_integrity.payload["persist_failed_count"], 1);
        }
        assert_eq!(sink.bytes.available_permits(), BYTE_CAPACITY);
    }
}
