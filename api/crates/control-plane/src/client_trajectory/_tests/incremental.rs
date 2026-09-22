use super::*;
use serde_json::Value;

fn long_request() -> Vec<u8> {
    serde_json::to_vec(&json!({"model":"gpt-5.6-luna","reasoning":{"effort":"max"},
        "input":(0..600).map(|index|json!({"role":"user","content":format!("{index}:{}", "x".repeat(2048))})).collect::<Vec<_>>()
    })).unwrap()
}

#[tokio::test]
async fn large_valid_history_is_complete_beyond_old_aggregate_and_item_limits() {
    let request = long_request();
    assert!(request.len() > 1024 * 1024);
    assert!(request.len() < decode::AGGREGATE_BYTES);
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    // The whole accepted request can be admitted before the worker is scheduled.
    recorder.record(ClientTrajectoryFrameKind::Request, &request);
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(
        ClientTrajectoryFrameKind::ResponseJson,
        b"{\"object\":\"response\",\"output\":[]}",
    );
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    assert_eq!(raw(&records, "submitted"), request);
    let steps = records
        .iter()
        .filter(|r| matches!(&r.fact, ClientTrajectoryFact::Step {step} if step.category=="user"))
        .count();
    assert_eq!(steps, 600);
    let results: Vec<_> = records
        .iter()
        .filter_map(|r| match &r.fact {
            ClientTrajectoryFact::Section { section, value, .. } if section == "result" => {
                Some(value)
            }
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 600);
    for (index, value) in results.iter().enumerate() {
        assert_eq!(
            value.as_str().unwrap(),
            format!("{index}:{}", "x".repeat(2048))
        );
    }
}

struct PausedSink {
    count: Arc<AtomicU64>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}
#[async_trait::async_trait]
impl classify::FactSink for PausedSink {
    async fn push(&mut self, _fact: ClientTrajectoryFact) {
        if self.count.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }
}
#[tokio::test]
async fn classifier_waits_for_sink_instead_of_buffering_all_history_facts() {
    let count = Arc::new(AtomicU64::new(0));
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut sink = PausedSink {
        count: count.clone(),
        entered: entered.clone(),
        release: release.clone(),
    };
    let task = tokio::spawn(async move {
        let mut classifier = classify::Classifier::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            None,
            ClientTrajectoryTransport::Http,
        );
        classifier
            .observe_into(
                ClientTrajectoryFrameKind::Request,
                serde_json::from_slice::<Value>(&long_request()).unwrap(),
                "2026-09-22T00:00:00Z",
                &mut sink,
            )
            .await;
        assert!(!classifier.incomplete);
    });
    entered.notified().await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(!task.is_finished());
    release.notify_one();
    task.await.unwrap();
    // Each message: step + original overview + extracted result + timing.
    assert_eq!(count.load(Ordering::SeqCst), 3 + 600 * 4);
}

#[tokio::test]
async fn maximum_request_fits_admission_and_oversized_decode_is_incomplete() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    let request = format!(
        "{{\"input\":\"{}\"}}",
        "x".repeat(decode::AGGREGATE_BYTES - 12)
    );
    assert_eq!(request.len(), decode::AGGREGATE_BYTES);
    recorder.record(ClientTrajectoryFrameKind::Request, request.as_bytes());
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(
        ClientTrajectoryFrameKind::ResponseJson,
        b"{\"object\":\"response\",\"output\":[]}",
    );
    recorder.finish();
    recorder.wait_finished().await;
    assert!(complete(&writer.records.lock().unwrap()));
    let mut decoder = decode::Decoder::default();
    assert!(decoder
        .feed(
            ClientTrajectoryFrameKind::Request,
            &vec![b' '; decode::AGGREGATE_BYTES + 1]
        )
        .is_empty());
    assert!(decoder.incomplete);
}

#[tokio::test]
async fn observed_nodes_are_deduplicated_without_reassigning_the_client_capture() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    let flow = Uuid::now_v7();
    let nodes = [Uuid::now_v7(), Uuid::now_v7()];
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
    for node in [nodes[0], nodes[1], nodes[0]] {
        recorder.link_llm_node(flow, node);
    }
    recorder.record(
        ClientTrajectoryFrameKind::ResponseJson,
        b"{\"object\":\"response\",\"output\":[]}",
    );
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    let links: BTreeSet<_> = records
        .iter()
        .filter_map(|r| match r.fact {
            ClientTrajectoryFact::NodeLink { node_run_id } => Some(node_run_id),
            _ => None,
        })
        .collect();
    assert_eq!(links, BTreeSet::from(nodes));
    assert_eq!(
        records
            .iter()
            .filter(|r| matches!(r.fact, ClientTrajectoryFact::NodeLink { .. }))
            .count(),
        2
    );
    assert!(records.iter().all(|r| r.node_run_id.is_none()));
    assert_eq!(raw(&records, "submitted"), b"{}");
}
