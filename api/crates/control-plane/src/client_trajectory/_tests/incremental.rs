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
    assert!(request.len() < (2 * 1024 * 1024));
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    // The whole accepted request can be admitted before the worker is scheduled.
    recorder
        .record(ClientTrajectoryFrameKind::Request, &request)
        .await
        .unwrap();
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
    recorder.bind_run(Uuid::now_v7(), None);
    recorder
        .record(
            ClientTrajectoryFrameKind::ResponseJson,
            b"{\"object\":\"response\",\"output\":[]}",
        )
        .await
        .unwrap();
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
async fn large_request_fits_admission_and_decode_has_no_observer_size_cutoff() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    let request = format!("{{\"input\":\"{}\"}}", "x".repeat((2 * 1024 * 1024) - 12));
    assert_eq!(request.len(), (2 * 1024 * 1024));
    recorder
        .record(ClientTrajectoryFrameKind::Request, request.as_bytes())
        .await
        .unwrap();
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
    recorder.bind_run(Uuid::now_v7(), None);
    recorder
        .record(
            ClientTrajectoryFrameKind::ResponseJson,
            b"{\"object\":\"response\",\"output\":[]}",
        )
        .await
        .unwrap();
    recorder.finish();
    recorder.wait_finished().await;
    assert!(complete(&writer.records.lock().unwrap()));
    let mut decoder = decode::Decoder::default();
    assert!(decoder
        .feed(
            ClientTrajectoryFrameKind::Request,
            &vec![b' '; (2 * 1024 * 1024) + 1]
        )
        .is_empty());
    assert!(!decoder.incomplete);
}

#[tokio::test]
async fn observed_nodes_are_deduplicated_without_reassigning_the_client_capture() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    let flow = Uuid::now_v7();
    let nodes = [Uuid::now_v7(), Uuid::now_v7()];
    recorder
        .record(ClientTrajectoryFrameKind::Request, b"{}")
        .await
        .unwrap();
    for node in [nodes[0], nodes[1], nodes[0]] {
        recorder.link_llm_node(flow, node);
    }
    recorder
        .record(
            ClientTrajectoryFrameKind::ResponseJson,
            b"{\"object\":\"response\",\"output\":[]}",
        )
        .await
        .unwrap();
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

#[tokio::test]
async fn owned_request_overview_preserves_original_content_allocation_and_full_text() {
    let content = "owned-content-".to_owned() + &"x".repeat(2 * 1024 * 1024);
    let request = json!({"input":[{"role":"user","content":content}]});
    let original_pointer = request["input"][0]["content"].as_str().unwrap().as_ptr() as usize;
    let mut classifier = classify::Classifier::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        ClientTrajectoryTransport::Http,
    );
    let mut facts = Vec::new();
    classifier
        .observe_into(
            ClientTrajectoryFrameKind::Request,
            request,
            "2026-10-30T00:00:00Z",
            &mut facts,
        )
        .await;
    let overview = facts
        .iter()
        .find_map(|fact| match fact {
            ClientTrajectoryFact::Section { section, value, .. }
                if section == "overview" && value["role"] == "user" =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    let actual = overview["content"].as_str().unwrap();
    assert_eq!(actual, content);
    assert_eq!(
        actual.as_ptr() as usize,
        original_pointer,
        "the owned item must move into its overview, without cloning the original allocation"
    );
    let result = facts
        .iter()
        .find_map(|fact| match fact {
            ClientTrajectoryFact::Section { section, value, .. } if section == "result" => {
                value.as_str()
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(result, content);
    let step = facts
        .iter()
        .find_map(|fact| match fact {
            ClientTrajectoryFact::Step { step } if step.category == "user" => Some(step),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        step.available_sections,
        ["overview", "result", "timing", "raw"]
    );
    assert!(!classifier.incomplete);
}

#[tokio::test]
async fn duplicate_outputs_keep_first_id_or_index_and_terminal_usage_link() {
    let mut classifier = classify::Classifier::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        None,
        ClientTrajectoryTransport::Http,
    );
    let at = "2026-10-30T00:00:00Z";
    let mut facts = Vec::new();
    let accepted = [
        json!({"id":"A","role":"assistant","content":"first-A"}),
        json!({"id":"B","role":"assistant","content":"first-B"}),
        json!({"id":"","role":"assistant","content":"empty-id-indexed"}),
        json!({"id":7,"role":"assistant","content":"nonstring-id-indexed"}),
    ];
    for (item, index) in [
        (accepted[0].clone(), Some(0)),
        (
            json!({"id":"A","content":"id-conflict-does-not-reserve-index-1"}),
            Some(1),
        ),
        (
            json!({"id":"B","content":"index-conflict-does-not-reserve-id-B"}),
            Some(0),
        ),
        (accepted[1].clone(), Some(1)),
        (accepted[2].clone(), Some(2)),
        (accepted[3].clone(), Some(3)),
        (json!({"id":"","content":"empty-id-conflict"}), None),
        (json!({"id":7,"content":"no-string-identity"}), None),
        (json!({"content":"no-id-or-index"}), None),
    ] {
        let mut event = json!({"type":"response.output_item.done", "item":item});
        if let Some(index) = index {
            event["output_index"] = json!(index);
        }
        classifier
            .observe_into(
                ClientTrajectoryFrameKind::ResponseSse,
                event,
                at,
                &mut facts,
            )
            .await;
    }
    classifier.observe_into(ClientTrajectoryFrameKind::ResponseSse, json!({"type":"response.completed","response":{"id":"response-link-kept","output":accepted,"usage":{"output_tokens":4}}}), at, &mut facts).await;
    let overview: Vec<_> = facts
        .iter()
        .filter_map(|fact| match fact {
            ClientTrajectoryFact::Section { section, value, .. } if section == "overview" => {
                Some(value.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(overview, accepted);
    assert_eq!(facts.iter().filter(|fact| matches!(fact, ClientTrajectoryFact::Step { step } if step.category == "assistant")).count(), 4);
    assert!(facts.iter().any(|fact| matches!(fact, ClientTrajectoryFact::Section { section, value, .. } if section == "usage" && value["output_tokens"] == 4)));
    assert!(facts.iter().any(|fact| matches!(fact, ClientTrajectoryFact::ResponseLink { response_id } if response_id == "response-link-kept")));
    assert!(
        classifier.incomplete,
        "unidentified output retains the original incomplete semantics"
    );
    assert!(classifier.completed);
}
