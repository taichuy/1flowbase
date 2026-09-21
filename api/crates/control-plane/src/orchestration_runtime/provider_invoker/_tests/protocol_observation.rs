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
