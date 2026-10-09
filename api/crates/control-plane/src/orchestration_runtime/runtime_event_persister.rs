use std::{sync::Arc, time::Duration};

use anyhow::Result;
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use tracing::warn;
use uuid::Uuid;

use crate::ports::{
    AppendRuntimeEventInput, OrchestrationRuntimeRepository, RuntimeEventAfterCommitDeliveryStatus,
    RuntimeEventAfterCommitLane, RuntimeEventAfterCommitReceipt, RuntimeEventCloseReason,
    RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventStream,
    RuntimeEventSubscription, RuntimeEventTerminalWriter,
};

const RUNTIME_EVENT_BATCH_MAX_BYTES: usize = 64 * 1024;
pub(crate) const RUNTIME_EVENT_BATCH_MAX_DELAY: Duration = Duration::from_millis(20);

#[derive(Default)]
struct RuntimeEventPersistenceBatch {
    events: Vec<RuntimeEventEnvelope>,
    payload_bytes: usize,
}

impl RuntimeEventPersistenceBatch {
    fn push(&mut self, event: RuntimeEventEnvelope) {
        self.payload_bytes = self
            .payload_bytes
            .saturating_add(event.payload.to_string().len());
        self.events.push(event);
    }

    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    fn reached_byte_limit(&self) -> bool {
        self.payload_bytes >= RUNTIME_EVENT_BATCH_MAX_BYTES
    }

    fn take(&mut self) -> Vec<RuntimeEventEnvelope> {
        self.payload_bytes = 0;
        std::mem::take(&mut self.events)
    }
}

pub async fn persist_runtime_event_payload<R>(
    repository: &R,
    flow_run_id: Uuid,
    event: &RuntimeEventPayload,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let lane = RuntimeEventAfterCommitLane::empty();
    persist_runtime_event_payload_with_after_commit(repository, flow_run_id, event, &lane)
        .await
        .map(|_| ())
}

pub async fn persist_runtime_event_payload_with_after_commit<R>(
    repository: &R,
    flow_run_id: Uuid,
    event: &RuntimeEventPayload,
    after_commit: &RuntimeEventAfterCommitLane,
) -> Result<Vec<RuntimeEventAfterCommitReceipt>>
where
    R: OrchestrationRuntimeRepository,
{
    if !event.persist_required || is_stream_delta_event(&event.event_type) {
        return Ok(Vec::new());
    }
    let node_run_id = event
        .payload
        .get("node_run_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let input = build_runtime_event_input(
        flow_run_id,
        node_run_id,
        event.event_type.clone(),
        event.source,
        event.payload.clone(),
    );
    let record = repository.append_runtime_event(&input).await?;
    if event.durability == RuntimeEventDurability::Ephemeral {
        return Ok(Vec::new());
    }
    let envelope = RuntimeEventEnvelope::new(flow_run_id, record.sequence, event.clone());
    Ok(vec![after_commit.deliver_after_commit(envelope).await])
}

pub async fn persist_runtime_debug_stream_events<R>(
    repository: &R,
    events: Vec<RuntimeEventEnvelope>,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let lane = RuntimeEventAfterCommitLane::empty();
    persist_runtime_debug_stream_events_with_after_commit(repository, events, &lane)
        .await
        .map(|_| ())
}

pub async fn persist_runtime_debug_stream_events_with_after_commit<R>(
    repository: &R,
    events: Vec<RuntimeEventEnvelope>,
    after_commit: &RuntimeEventAfterCommitLane,
) -> Result<Vec<RuntimeEventAfterCommitReceipt>>
where
    R: OrchestrationRuntimeRepository,
{
    // Canonical reducers and invocation snapshots own completed content. The live
    // stream retains its sequence; token fragments never acquire durable rows.
    let events = events
        .into_iter()
        .filter(|event| event.persist_required && !is_stream_delta_event(&event.event_type))
        .collect::<Vec<_>>();
    let runtime_events = events
        .iter()
        .map(|event| {
            build_runtime_event_input(
                event.run_id,
                event.node_run_id,
                event.event_type.clone(),
                event.source,
                payload_with_stream_sequence(event.payload.clone(), event.sequence, event.sequence),
            )
        })
        .collect::<Vec<_>>();
    if !runtime_events.is_empty() {
        repository.append_runtime_events(&runtime_events).await?;
    }
    let after_commit_events = events
        .into_iter()
        .filter(|event| event.durability != RuntimeEventDurability::Ephemeral);

    let mut receipts = Vec::new();
    for event in after_commit_events {
        receipts.push(after_commit.deliver_after_commit(event).await);
    }
    Ok(receipts)
}

pub async fn project_runtime_event_stream_terminal(
    stream: Arc<dyn RuntimeEventStream>,
    flow_run: &domain::FlowRunRecord,
) {
    let Some(terminal_event) =
        super::stream_terminal_recovery::terminal_event_from_flow_run(flow_run)
    else {
        warn!(
            flow_run_id = %flow_run.id,
            durable_status = %flow_run.status.as_str(),
            "runtime event stream fallback has no durable terminal to project"
        );
        return;
    };
    project_runtime_event_stream_terminal_payload(
        stream,
        flow_run.id,
        flow_run.status,
        terminal_event,
    )
    .await;
}

pub(super) async fn project_runtime_event_stream_terminal_payload(
    stream: Arc<dyn RuntimeEventStream>,
    flow_run_id: Uuid,
    durable_status: domain::FlowRunStatus,
    mut terminal_event: RuntimeEventPayload,
) {
    terminal_event.persist_required = false;
    terminal_event.durability = crate::ports::RuntimeEventDurability::Ephemeral;
    if let Err(error) = stream
        .append_terminal_if_missing_and_close(flow_run_id, terminal_event)
        .await
    {
        warn!(
            flow_run_id = %flow_run_id,
            durable_status = %durable_status.as_str(),
            error = %error,
            "failed to project durable terminal to runtime event stream"
        );
    }
}

pub fn spawn_runtime_debug_event_persister<R>(
    repository: R,
    stream: Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
) -> JoinHandle<()>
where
    R: OrchestrationRuntimeRepository + Send + Sync + 'static,
{
    spawn_runtime_debug_event_persister_with_after_commit(
        repository,
        stream,
        run_id,
        RuntimeEventAfterCommitLane::empty(),
    )
}

pub fn spawn_runtime_debug_event_persister_with_after_commit<R>(
    repository: R,
    stream: Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
    after_commit: RuntimeEventAfterCommitLane,
) -> JoinHandle<()>
where
    R: OrchestrationRuntimeRepository + Send + Sync + 'static,
{
    tokio::spawn(async move {
        start_runtime_debug_event_persister_with_after_commit(
            repository,
            stream,
            run_id,
            after_commit,
        )
        .await
        .await
        .unwrap_or_else(
            |error| warn!(flow_run_id = %run_id, %error, "runtime event persister task failed"),
        );
    })
}

/// Install the generation's persistence subscription and scalar durable anchor
/// before starting its producer. No conversation body is copied to the anchor.
pub async fn start_runtime_debug_event_persister<R>(
    repository: R,
    stream: Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
) -> JoinHandle<()>
where
    R: OrchestrationRuntimeRepository + Send + Sync + 'static,
{
    start_runtime_debug_event_persister_with_after_commit(
        repository,
        stream,
        run_id,
        RuntimeEventAfterCommitLane::empty(),
    )
    .await
}

async fn start_runtime_debug_event_persister_with_after_commit<R>(
    repository: R,
    stream: Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
    after_commit: RuntimeEventAfterCommitLane,
) -> JoinHandle<()>
where
    R: OrchestrationRuntimeRepository + Send + Sync + 'static,
{
    let Ok(subscription) = stream.subscribe(run_id, Some(0)).await else {
        warn!(flow_run_id = %run_id, "failed to subscribe runtime debug stream for durable event persistence");
        return tokio::spawn(async {});
    };
    let writer = &subscription.terminal_writer;
    if !writer.claim_persistence_owner() {
        return tokio::spawn(async {});
    }
    if let Some(generation_id) = writer.generation_id() {
        if writer.durable_replay_boundary().is_none() {
            let anchor = build_runtime_event_input(
                run_id,
                None,
                "runtime_stream_opened".to_owned(),
                crate::ports::RuntimeEventSource::System,
                json!({"type": "runtime_stream_opened", "stream_generation_id": generation_id}),
            );
            match repository.append_runtime_event(&anchor).await {
                Ok(record) => {
                    if let Err(error) = writer.set_durable_replay_boundary(record.sequence) {
                        writer.record_persistence_failure();
                        warn!(flow_run_id = %run_id, %error, "failed to bind runtime replay generation anchor");
                    }
                }
                Err(error) => {
                    writer.record_persistence_failure();
                    warn!(flow_run_id = %run_id, %error, "failed to persist runtime replay generation anchor");
                }
            }
        }
    }
    let scope_owner = after_commit.scope_owner();
    tokio::spawn(async move {
        let _scope_owner = scope_owner;
        persist_subscribed_debug_events(repository, run_id, after_commit, subscription).await;
    })
}

async fn persist_subscribed_debug_events<R>(
    repository: R,
    run_id: Uuid,
    after_commit: RuntimeEventAfterCommitLane,
    mut subscription: RuntimeEventSubscription,
) where
    R: OrchestrationRuntimeRepository,
{
    let writer = subscription.terminal_writer.clone();
    let mut batch = RuntimeEventPersistenceBatch::default();
    for event in std::mem::take(&mut subscription.replay) {
        let terminal = is_terminal_runtime_event(&event.event_type);
        let terminal_sequence = event.sequence;
        let terminal_type = event.event_type.clone();
        push_debug_event_for_persistence(
            &repository,
            &after_commit,
            &mut batch,
            run_id,
            event,
            writer.as_ref(),
        )
        .await;
        if terminal {
            confirm_closed_generation(
                &repository,
                run_id,
                terminal_sequence,
                &terminal_type,
                &mut subscription,
            )
            .await;
            return;
        }
    }
    let start = tokio::time::Instant::now() + RUNTIME_EVENT_BATCH_MAX_DELAY;
    let mut flush_interval = tokio::time::interval_at(start, RUNTIME_EVENT_BATCH_MAX_DELAY);
    loop {
        tokio::select! {
            maybe_event = subscription.live_events.recv() => {
                let Some(event) = maybe_event else {
                    flush_or_mark_failure(&repository, &after_commit, &mut batch, run_id, writer.as_ref()).await;
                    // A bare EOF is not evidence of a durable terminal.
                    return;
                };
                let terminal = is_terminal_runtime_event(&event.event_type);
                let terminal_sequence = event.sequence;
                let terminal_type = event.event_type.clone();
                push_debug_event_for_persistence(&repository, &after_commit, &mut batch, run_id, event, writer.as_ref()).await;
                if terminal {
                    confirm_closed_generation(&repository, run_id, terminal_sequence, &terminal_type, &mut subscription).await;
                    return;
                }
            }
            _ = flush_interval.tick(), if !batch.is_empty() => {
                flush_or_mark_failure(&repository, &after_commit, &mut batch, run_id, writer.as_ref()).await;
            }
        }
    }
}

async fn confirm_closed_generation<R: OrchestrationRuntimeRepository>(
    repository: &R,
    run_id: Uuid,
    terminal_sequence: i64,
    terminal_type: &str,
    subscription: &mut RuntimeEventSubscription,
) {
    let writer = subscription.terminal_writer.clone();
    let Some(generation_id) = writer.generation_id() else {
        return;
    };
    let Some(boundary) = writer.durable_replay_boundary() else {
        return;
    };
    // Terminal publication and producer closure are separate facts. Wait for
    // the existing owner to close; this subscriber must never close it itself.
    loop {
        let closure = *subscription.closure.borrow_and_update();
        if let Some(closure) = closure {
            if closure.final_sequence != terminal_sequence {
                return;
            }
            break;
        }
        tokio::select! {
            changed = subscription.closure.changed() => {
                if changed.is_err() { return; }
            }
            event = subscription.live_events.recv() => {
                // Data after the alleged terminal or EOF without closure cannot
                // prove this generation complete. Retain the legacy window.
                if event.is_some() { writer.record_persistence_failure(); }
                return;
            }
        }
    }
    let proof = async {
        let window = repository.get_runtime_event_replay_window(run_id).await?;
        if !window.is_some_and(|window| window.generation_id == Some(generation_id)) {
            anyhow::bail!("runtime replay generation is no longer the durable window");
        }
        if !repository
            .has_runtime_event_terminal_after(run_id, boundary, terminal_type)
            .await?
        {
            anyhow::bail!("runtime stream terminal has no durable fact");
        }
        writer.confirm_terminal_persisted(terminal_sequence).await
    }
    .await;
    if let Err(error) = proof {
        warn!(flow_run_id = %run_id, %error, "runtime replay generation remains in its legacy retention window");
    }
}

pub(super) async fn record_stream_persistence_failure(
    stream: &Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
) {
    if let Ok(writer) = stream.terminal_writer(run_id).await {
        writer.record_persistence_failure();
    }
}

pub async fn wait_for_runtime_debug_event_persister(
    handle: JoinHandle<()>,
    application_id: Uuid,
    run_id: Uuid,
) {
    match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(
                application_id = %application_id,
                flow_run_id = %run_id,
                error = %error,
                "runtime debug stream persister task panicked"
            );
        }
        Err(_) => {
            warn!(
                application_id = %application_id,
                flow_run_id = %run_id,
                "runtime debug stream persister did not finish after terminal event"
            );
        }
    }
}

async fn push_debug_event_for_persistence<R>(
    repository: &R,
    after_commit: &RuntimeEventAfterCommitLane,
    batch: &mut RuntimeEventPersistenceBatch,
    run_id: Uuid,
    mut event: RuntimeEventEnvelope,
    writer: &dyn RuntimeEventTerminalWriter,
) where
    R: OrchestrationRuntimeRepository,
{
    let is_terminal = is_terminal_runtime_event(&event.event_type);
    // Discard nonpersistent fragments before they acquire batch capacity or
    // trigger an otherwise unnecessary SQL flush. Required lanes retain await
    // backpressure while eligible writes are in progress.
    if event.persist_required && !is_stream_delta_event(&event.event_type) {
        if let (Some(generation_id), Some(object)) =
            (writer.generation_id(), event.payload.as_object_mut())
        {
            object.insert("stream_generation_id".into(), json!(generation_id));
        }
        batch.push(event);
    }
    if is_terminal || batch.reached_byte_limit() {
        flush_or_mark_failure(repository, after_commit, batch, run_id, writer).await;
    }
}

async fn flush_or_mark_failure<R: OrchestrationRuntimeRepository>(
    repository: &R,
    after_commit: &RuntimeEventAfterCommitLane,
    batch: &mut RuntimeEventPersistenceBatch,
    run_id: Uuid,
    writer: &dyn RuntimeEventTerminalWriter,
) {
    if let Err(error) = flush_debug_event_batch(repository, after_commit, batch, run_id).await {
        writer.record_persistence_failure();
        warn!(flow_run_id = %run_id, %error, "failed to persist runtime debug stream events");
    }
}

async fn flush_debug_event_batch<R: OrchestrationRuntimeRepository>(
    repository: &R,
    after_commit: &RuntimeEventAfterCommitLane,
    batch: &mut RuntimeEventPersistenceBatch,
    run_id: Uuid,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let events = batch.take();
    let receipts =
        persist_runtime_debug_stream_events_with_after_commit(repository, events, after_commit)
            .await?;
    for subscriber in receipts
        .iter()
        .flat_map(|receipt| &receipt.subscribers)
        .filter(|subscriber| subscriber.status == RuntimeEventAfterCommitDeliveryStatus::Failed)
    {
        warn!(
            flow_run_id = %run_id,
            subscriber_id = %subscriber.subscriber_id.as_str(),
            contribution_id = %subscriber.contribution_id,
            attempts = subscriber.attempts,
            failure_reason = ?subscriber.failure_reason,
            "runtime event after-commit subscriber exhausted its delivery policy"
        );
    }
    Ok(())
}

fn is_stream_delta_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "text_delta" | "reasoning_delta" | "tool_call_delta" | "mcp_call_delta"
    )
}

fn is_terminal_runtime_event(event_type: &str) -> bool {
    RuntimeEventCloseReason::from_terminal_event_type(event_type).is_some()
}

pub(super) fn payload_with_stream_sequence(
    mut payload: Value,
    sequence_start: i64,
    sequence_end: i64,
) -> Value {
    if let Some(object) = payload.as_object_mut() {
        object
            .entry("stream_sequence")
            .or_insert_with(|| json!(sequence_end));
        object
            .entry("sequence_start")
            .or_insert_with(|| json!(sequence_start));
        object
            .entry("sequence_end")
            .or_insert_with(|| json!(sequence_end));
    }
    payload
}

pub(super) fn build_runtime_event_input(
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    event_type: String,
    source: crate::ports::RuntimeEventSource,
    payload: Value,
) -> AppendRuntimeEventInput {
    let (layer, source, trust_level, visibility, durability) = classify_event(&event_type, source);

    AppendRuntimeEventInput {
        flow_run_id,
        node_run_id,
        span_id: None,
        parent_span_id: None,
        event_type,
        layer,
        source,
        trust_level,
        item_id: None,
        ledger_ref: None,
        payload,
        visibility,
        durability,
    }
}

fn classify_event(
    event_type: &str,
    source: crate::ports::RuntimeEventSource,
) -> (
    domain::RuntimeEventLayer,
    domain::RuntimeEventSource,
    domain::RuntimeTrustLevel,
    domain::RuntimeEventVisibility,
    domain::RuntimeEventDurability,
) {
    let layer = match event_type {
        "flow_started" | "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human"
        | "waiting_callback" => domain::RuntimeEventLayer::AgentTransition,
        "tool_call_commit"
        | "tool_result_appended"
        | "capability_call_requested"
        | "capability_call_finished" => domain::RuntimeEventLayer::Capability,
        "usage_snapshot" | "usage_recorded" | "cost_recorded" => domain::RuntimeEventLayer::Ledger,
        "error" | "run_failed" | "llm_turn_failed" => domain::RuntimeEventLayer::Diagnostic,
        _ => domain::RuntimeEventLayer::RuntimeItem,
    };
    let source = match source {
        crate::ports::RuntimeEventSource::Runtime
        | crate::ports::RuntimeEventSource::Provider
        | crate::ports::RuntimeEventSource::Persister
        | crate::ports::RuntimeEventSource::System => domain::RuntimeEventSource::Host,
    };

    (
        layer,
        source,
        domain::RuntimeTrustLevel::HostFact,
        domain::RuntimeEventVisibility::Workspace,
        domain::RuntimeEventDurability::Durable,
    )
}
