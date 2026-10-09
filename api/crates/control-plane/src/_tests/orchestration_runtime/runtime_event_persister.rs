use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};

use crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository;
use crate::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventClosure, RuntimeEventReceiver,
    RuntimeEventSource,
};

// Only generation authority is controlled here; all writes and durable proofs
// use the real in-memory orchestration repository and production persister.
struct ControlledGenerationWriter {
    generation_id: Uuid,
    boundary: Mutex<Option<i64>>,
    failed: AtomicBool,
    confirmation_attempts: AtomicUsize,
    confirmed: AtomicUsize,
    owner_claimed: AtomicBool,
}

impl ControlledGenerationWriter {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            generation_id: Uuid::now_v7(),
            boundary: Mutex::new(None),
            failed: AtomicBool::new(false),
            confirmation_attempts: AtomicUsize::new(0),
            confirmed: AtomicUsize::new(0),
            owner_claimed: AtomicBool::new(false),
        })
    }
}

#[async_trait::async_trait]
impl RuntimeEventTerminalWriter for ControlledGenerationWriter {
    fn claim_persistence_owner(&self) -> bool {
        !self.owner_claimed.swap(true, Ordering::SeqCst)
    }

    fn generation_id(&self) -> Option<Uuid> {
        Some(self.generation_id)
    }

    fn durable_replay_boundary(&self) -> Option<i64> {
        *self.boundary.lock().unwrap()
    }

    fn set_durable_replay_boundary(&self, sequence: i64) -> Result<()> {
        let mut boundary = self.boundary.lock().unwrap();
        match *boundary {
            Some(existing) if existing != sequence => anyhow::bail!("boundary is immutable"),
            _ => *boundary = Some(sequence),
        }
        Ok(())
    }

    fn record_persistence_failure(&self) {
        self.failed.store(true, Ordering::SeqCst);
    }

    async fn confirm_terminal_persisted(&self, _final_sequence: i64) -> Result<()> {
        self.confirmation_attempts.fetch_add(1, Ordering::SeqCst);
        anyhow::ensure!(
            !self.failed.load(Ordering::SeqCst),
            "generation has a sticky persistence failure"
        );
        self.confirmed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        _event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("persister must not acquire producer terminal authority")
    }
}

fn event(
    run_id: Uuid,
    sequence: i64,
    event_type: &str,
    persist_required: bool,
) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run_id,
        sequence,
        RuntimeEventPayload {
            event_type: event_type.into(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required,
            trace_visible: true,
            payload: json!({"type": event_type}),
        },
    )
}

fn closed_subscription(
    writer: Arc<ControlledGenerationWriter>,
    replay: Vec<RuntimeEventEnvelope>,
    final_sequence: Option<i64>,
) -> RuntimeEventSubscription {
    let (required, diagnostic, live_events) = RuntimeEventReceiver::bounded_lanes(2);
    drop(required);
    drop(diagnostic);
    let (_closure_sender, closure) =
        tokio::sync::watch::channel(final_sequence.map(|sequence| RuntimeEventClosure {
            reason: RuntimeEventCloseReason::Finished,
            final_sequence: sequence,
        }));
    RuntimeEventSubscription {
        terminal_writer: writer,
        replay,
        live_events,
        closure,
    }
}

async fn prepare_durable_generation(
    repository: &InMemoryOrchestrationRuntimeRepository,
    run_id: Uuid,
    writer: &ControlledGenerationWriter,
) {
    let anchor = build_runtime_event_input(
        run_id,
        None,
        "runtime_stream_opened".into(),
        RuntimeEventSource::System,
        json!({"type": "runtime_stream_opened", "stream_generation_id": writer.generation_id}),
    );
    let record = repository.append_runtime_event(&anchor).await.unwrap();
    writer.set_durable_replay_boundary(record.sequence).unwrap();
}

async fn commit_upstream_terminal(
    repository: &InMemoryOrchestrationRuntimeRepository,
    run_id: Uuid,
) {
    let input = build_runtime_event_input(
        run_id,
        None,
        "flow_finished".into(),
        RuntimeEventSource::Runtime,
        json!({"type": "flow_finished", "committed_delivery": true}),
    );
    repository.append_runtime_event(&input).await.unwrap();
}

#[tokio::test]
async fn discarded_large_events_never_enter_batch_or_attempt_repository_write() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    let lane = RuntimeEventAfterCommitLane::empty();
    let mut batch = RuntimeEventPersistenceBatch::default();
    repository.fail_next_runtime_event_append();

    for (event_type, persist_required) in [
        ("text_delta", true),
        ("reasoning_delta", true),
        ("tool_call_delta", true),
        ("mcp_call_delta", true),
        ("usage_snapshot", false),
    ] {
        let mut discarded = event(run_id, 1, event_type, persist_required);
        discarded.payload = json!({"delta": "x".repeat(RUNTIME_EVENT_BATCH_MAX_BYTES * 2)});
        push_debug_event_for_persistence(
            &repository,
            &lane,
            &mut batch,
            run_id,
            discarded,
            writer.as_ref(),
        )
        .await;
        assert!(batch.is_empty());
        assert_eq!(batch.payload_bytes, 0);
    }
    flush_debug_event_batch(&repository, &lane, &mut batch, run_id)
        .await
        .unwrap();
    assert!(!writer.failed.load(Ordering::SeqCst));
    assert!(repository
        .list_runtime_events(run_id, 0)
        .await
        .unwrap()
        .is_empty());

    // The armed repository error must remain unconsumed: empty/discarded
    // batches cannot reach even the first SQL-equivalent append operation.
    push_debug_event_for_persistence(
        &repository,
        &lane,
        &mut batch,
        run_id,
        event(run_id, 2, "usage_snapshot", true),
        writer.as_ref(),
    )
    .await;
    let error = flush_debug_event_batch(&repository, &lane, &mut batch, run_id)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("simulated runtime event append failure"));
}

#[tokio::test]
async fn excluded_terminal_flushes_prior_eligible_batch() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    let lane = RuntimeEventAfterCommitLane::empty();
    let mut batch = RuntimeEventPersistenceBatch::default();
    push_debug_event_for_persistence(
        &repository,
        &lane,
        &mut batch,
        run_id,
        event(run_id, 1, "usage_snapshot", true),
        writer.as_ref(),
    )
    .await;
    assert_eq!(batch.events.len(), 1);
    assert!(repository
        .list_runtime_events(run_id, 0)
        .await
        .unwrap()
        .is_empty());
    push_debug_event_for_persistence(
        &repository,
        &lane,
        &mut batch,
        run_id,
        event(run_id, 2, "flow_finished", false),
        writer.as_ref(),
    )
    .await;
    assert!(batch.is_empty());
    let records = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].event_type, "usage_snapshot");
    assert_eq!(records[0].payload["stream_sequence"], 1);
    assert_eq!(
        records[0].payload["stream_generation_id"],
        json!(writer.generation_id)
    );
    assert!(!writer.failed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn earlier_batch_failure_blocks_confirmation_after_later_terminal_commit() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    prepare_durable_generation(&repository, run_id, writer.as_ref()).await;
    let lane = RuntimeEventAfterCommitLane::empty();
    let mut batch = RuntimeEventPersistenceBatch::default();
    push_debug_event_for_persistence(
        &repository,
        &lane,
        &mut batch,
        run_id,
        event(run_id, 1, "usage_snapshot", true),
        writer.as_ref(),
    )
    .await;
    repository.fail_next_runtime_event_append();
    flush_or_mark_failure(&repository, &lane, &mut batch, run_id, writer.as_ref()).await;
    assert!(writer.failed.load(Ordering::SeqCst));
    assert!(batch.is_empty());

    let subscription = closed_subscription(
        writer.clone(),
        vec![event(run_id, 2, "flow_finished", true)],
        Some(2),
    );
    persist_subscribed_debug_events(repository.clone(), run_id, lane, subscription).await;
    let records = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].event_type, "flow_finished");
    assert_eq!(writer.confirmation_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 0);
    let error = writer.confirm_terminal_persisted(2).await.unwrap_err();
    assert!(error.to_string().contains("sticky persistence failure"));
}

#[tokio::test]
async fn excluded_precommitted_terminal_with_matching_closed_replay_confirms_generation() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    prepare_durable_generation(&repository, run_id, writer.as_ref()).await;
    commit_upstream_terminal(&repository, run_id).await;
    let subscription = closed_subscription(
        writer.clone(),
        vec![event(run_id, 7, "flow_finished", false)],
        Some(7),
    );
    persist_subscribed_debug_events(
        repository.clone(),
        run_id,
        RuntimeEventAfterCommitLane::empty(),
        subscription,
    )
    .await;
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 1);
    assert_eq!(
        repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn live_required_terminal_with_matching_closure_confirms_generation() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    prepare_durable_generation(&repository, run_id, writer.as_ref()).await;
    let (required, diagnostic, live_events) = RuntimeEventReceiver::bounded_lanes(1);
    required
        .send(event(run_id, 1, "flow_finished", true))
        .await
        .unwrap();
    drop(required);
    drop(diagnostic);
    let (_sender, closure) = tokio::sync::watch::channel(Some(RuntimeEventClosure {
        reason: RuntimeEventCloseReason::Finished,
        final_sequence: 1,
    }));
    let subscription = RuntimeEventSubscription {
        terminal_writer: writer.clone(),
        replay: Vec::new(),
        live_events,
        closure,
    };
    persist_subscribed_debug_events(
        repository.clone(),
        run_id,
        RuntimeEventAfterCommitLane::empty(),
        subscription,
    )
    .await;
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 1);
    assert_eq!(
        repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn missing_durable_terminal_or_mismatched_or_absent_closure_never_confirms() {
    for (durable_terminal, closure_sequence) in [(false, Some(3)), (true, Some(4)), (true, None)] {
        let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
        let run_id = Uuid::now_v7();
        let writer = ControlledGenerationWriter::new();
        prepare_durable_generation(&repository, run_id, writer.as_ref()).await;
        if durable_terminal {
            commit_upstream_terminal(&repository, run_id).await;
        }
        let mut subscription = closed_subscription(writer.clone(), Vec::new(), closure_sequence);
        confirm_closed_generation(&repository, run_id, 3, "flow_finished", &mut subscription).await;
        assert_eq!(writer.confirmed.load(Ordering::SeqCst), 0);
        assert_eq!(writer.confirmation_attempts.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn earlier_generation_terminal_does_not_prove_current_generation_closed() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    commit_upstream_terminal(&repository, run_id).await;
    let writer = ControlledGenerationWriter::new();
    prepare_durable_generation(&repository, run_id, writer.as_ref()).await;
    let mut subscription = closed_subscription(writer.clone(), Vec::new(), Some(1));
    confirm_closed_generation(&repository, run_id, 1, "flow_finished", &mut subscription).await;
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 0);
    assert_eq!(writer.confirmation_attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn late_foreign_generation_terminal_never_proves_current_closed_generation() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    prepare_durable_generation(&repository, run_id, writer.as_ref()).await;
    let mut late = build_runtime_event_input(
        run_id,
        None,
        "flow_finished".into(),
        RuntimeEventSource::Runtime,
        json!({"type": "flow_finished", "stream_generation_id": Uuid::now_v7()}),
    );
    repository.append_runtime_event(&late).await.unwrap();
    let mut subscription = closed_subscription(writer.clone(), Vec::new(), Some(7));
    confirm_closed_generation(&repository, run_id, 7, "flow_finished", &mut subscription).await;
    assert_eq!(writer.confirmation_attempts.load(Ordering::SeqCst), 0);
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 0);

    // An explicitly matching fact is accepted; unrelated late rows do not
    // permanently prevent recovery of the correctly committed current round.
    late.payload["stream_generation_id"] = json!(writer.generation_id);
    repository.append_runtime_event(&late).await.unwrap();
    confirm_closed_generation(&repository, run_id, 7, "flow_finished", &mut subscription).await;
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn bound_inline_persistence_preserves_fact_and_marks_only_captured_generation_on_failure() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let captured = ControlledGenerationWriter::new();
    let replacement = ControlledGenerationWriter::new();
    let item = json!({"type": "reasoning", "encrypted_content": "original-opaque", "extension": {"numeric_lexeme": "00"}});
    let fact = RuntimeEventPayload {
        event_type: "provider_output_item_done".into(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({"type": "provider_output_item_done", "output_index": 0, "item": item}),
    };
    persist_runtime_event_payload_for_generation(
        &repository,
        run_id,
        &fact,
        Some(captured.as_ref()),
    )
    .await
    .unwrap();
    let records = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload["item"], item);
    assert_eq!(
        records[0].payload["stream_generation_id"],
        json!(captured.generation_id)
    );

    repository.fail_next_runtime_event_append();
    let error = persist_runtime_event_payload_for_generation(
        &repository,
        run_id,
        &fact,
        Some(captured.as_ref()),
    )
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "simulated runtime event append failure");
    assert!(captured.failed.load(Ordering::SeqCst));
    assert!(!replacement.failed.load(Ordering::SeqCst));
    persist_runtime_event_payload_for_generation(
        &repository,
        run_id,
        &fact,
        Some(replacement.as_ref()),
    )
    .await
    .unwrap();
    assert!(captured.failed.load(Ordering::SeqCst));
    let records = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[1].payload["stream_generation_id"],
        json!(replacement.generation_id)
    );
}

struct PreparedGenerationStream {
    writer: Arc<ControlledGenerationWriter>,
}

#[async_trait::async_trait]
impl RuntimeEventStream for PreparedGenerationStream {
    async fn open_run(
        &self,
        _run_id: Uuid,
        _policy: crate::ports::RuntimeEventStreamPolicy,
    ) -> Result<()> {
        anyhow::bail!("fixture generation is already prepared")
    }

    async fn append(
        &self,
        _run_id: Uuid,
        _event: RuntimeEventPayload,
    ) -> Result<RuntimeEventEnvelope> {
        anyhow::bail!("fixture has no producer")
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        _run_id: Uuid,
        _event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("persister cannot close producer generation")
    }

    async fn subscribe(
        &self,
        _run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> Result<RuntimeEventSubscription> {
        assert_eq!(from_sequence, Some(0));
        Ok(closed_subscription(self.writer.clone(), Vec::new(), None))
    }

    async fn replay(
        &self,
        _run_id: Uuid,
        _from_sequence: Option<i64>,
        _limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        anyhow::bail!("persister must consume the prepared subscription replay")
    }

    async fn close_run(&self, _run_id: Uuid, _reason: RuntimeEventCloseReason) -> Result<()> {
        anyhow::bail!("persister cannot close producer generation")
    }

    async fn trim(
        &self,
        _run_id: Uuid,
        _policy: crate::ports::RuntimeEventTrimPolicy,
    ) -> Result<()> {
        anyhow::bail!("persister cannot trim producer generation")
    }
}

#[tokio::test]
async fn start_prepares_scalar_anchor_once_for_generation_owner() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    let stream: Arc<dyn RuntimeEventStream> = Arc::new(PreparedGenerationStream {
        writer: writer.clone(),
    });
    let handle =
        start_runtime_debug_event_persister(repository.clone(), stream.clone(), run_id).await;
    // Preparation completes before the caller can start the producer.
    assert_eq!(writer.durable_replay_boundary(), Some(1));
    handle.await.unwrap();
    start_runtime_debug_event_persister(repository.clone(), stream, run_id)
        .await
        .await
        .unwrap();
    let records = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].event_type, "runtime_stream_opened");
    assert_eq!(
        records[0].payload,
        json!({
            "type": "runtime_stream_opened", "stream_generation_id": writer.generation_id,
        })
    );
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_anchor_preparation_marks_generation_failed_and_leaves_no_boundary() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let writer = ControlledGenerationWriter::new();
    let stream: Arc<dyn RuntimeEventStream> = Arc::new(PreparedGenerationStream {
        writer: writer.clone(),
    });
    repository.fail_next_runtime_event_append();
    start_runtime_debug_event_persister(repository.clone(), stream, run_id)
        .await
        .await
        .unwrap();
    assert!(writer.failed.load(Ordering::SeqCst));
    assert_eq!(writer.durable_replay_boundary(), None);
    assert_eq!(writer.confirmed.load(Ordering::SeqCst), 0);
    assert!(repository
        .list_runtime_events(run_id, 0)
        .await
        .unwrap()
        .is_empty());
}
