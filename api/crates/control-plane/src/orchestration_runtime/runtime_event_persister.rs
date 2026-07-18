use std::{sync::Arc, time::Duration};

use anyhow::Result;
use serde_json::{json, Value};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::warn;
use uuid::Uuid;

use crate::ports::{
    AppendRuntimeEventInput, OrchestrationRuntimeRepository, RuntimeEventCloseReason,
    RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventStream,
};

pub(super) const RUNTIME_EVENT_PERSISTENCE_QUEUE_CAPACITY: usize = 64;
const RUNTIME_EVENT_BATCH_MAX_BYTES: usize = 64 * 1024;
const RUNTIME_EVENT_BATCH_MAX_DELAY: Duration = Duration::from_millis(20);

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

    fn contains_terminal(&self) -> bool {
        self.events
            .iter()
            .any(|event| is_terminal_runtime_event(&event.event_type))
    }

    fn take(&mut self) -> Vec<RuntimeEventEnvelope> {
        self.payload_bytes = 0;
        std::mem::take(&mut self.events)
    }
}

pub(super) fn spawn_batched_runtime_event_persister<R>(
    repository: R,
    mut receiver: mpsc::Receiver<RuntimeEventEnvelope>,
) -> JoinHandle<Result<()>>
where
    R: OrchestrationRuntimeRepository + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let start = tokio::time::Instant::now() + RUNTIME_EVENT_BATCH_MAX_DELAY;
        let mut flush_interval = tokio::time::interval_at(start, RUNTIME_EVENT_BATCH_MAX_DELAY);
        let mut batch = RuntimeEventPersistenceBatch::default();

        loop {
            tokio::select! {
                maybe_event = receiver.recv() => {
                    let Some(event) = maybe_event else {
                        persist_runtime_event_batch(&repository, &mut batch).await?;
                        return Ok(());
                    };
                    batch.push(event);
                    if batch.reached_byte_limit() {
                        persist_runtime_event_batch(&repository, &mut batch).await?;
                    }
                }
                _ = flush_interval.tick(), if !batch.is_empty() => {
                    persist_runtime_event_batch(&repository, &mut batch).await?;
                }
            }
        }
    })
}

async fn persist_runtime_event_batch<R>(
    repository: &R,
    batch: &mut RuntimeEventPersistenceBatch,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    if batch.is_empty() {
        return Ok(());
    }
    persist_runtime_debug_stream_events(repository, batch.take()).await
}

pub async fn persist_runtime_event_payload<R>(
    repository: &R,
    flow_run_id: Uuid,
    event: &RuntimeEventPayload,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    if !event.persist_required {
        return Ok(());
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
    repository.append_runtime_event(&input).await?;
    Ok(())
}

pub async fn persist_runtime_debug_stream_events<R>(
    repository: &R,
    events: Vec<RuntimeEventEnvelope>,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let mut runtime_events = Vec::new();
    let mut pending_delta: Option<PendingStreamDelta> = None;

    for event in events {
        if !event.persist_required {
            continue;
        }

        if is_stream_delta_event(&event.event_type) {
            let node_run_id = event.node_run_id;
            let text = event
                .text
                .as_deref()
                .or_else(|| event.payload.get("text").and_then(Value::as_str))
                .or_else(|| event.payload.get("delta").and_then(Value::as_str))
                .unwrap_or_default();

            match &mut pending_delta {
                Some(pending)
                    if pending.run_id == event.run_id
                        && pending.node_run_id == node_run_id
                        && pending.event_type == event.event_type
                        && pending.has_same_projection_identity(&event) =>
                {
                    pending.text.push_str(text);
                    pending.sequence_end = event.sequence;
                    pending.merge_metadata(&event);
                    pending.event_ids.push(event.event_id);
                }
                _ => {
                    flush_pending_delta(&mut runtime_events, pending_delta.take());
                    let mut next_pending = PendingStreamDelta {
                        run_id: event.run_id,
                        node_run_id,
                        sequence_start: event.sequence,
                        sequence_end: event.sequence,
                        event_ids: vec![event.event_id.clone()],
                        event_type: event.event_type.clone(),
                        content_type: event.content_type.clone(),
                        source: event.source,
                        node_id: event.payload.get("node_id").cloned(),
                        presentation: event.payload.get("presentation").cloned(),
                        text: text.to_string(),
                        content_refs: Vec::new(),
                        artifact_refs: Vec::new(),
                        truncated: false,
                        truncation_reason: None,
                        original_bytes: None,
                    };
                    next_pending.merge_metadata(&event);
                    pending_delta = Some(next_pending);
                }
            }
            continue;
        }

        flush_pending_delta(&mut runtime_events, pending_delta.take());
        runtime_events.push(build_runtime_event_input(
            event.run_id,
            event.node_run_id,
            event.event_type,
            event.source,
            payload_with_stream_sequence(event.payload, event.sequence, event.sequence),
        ));
    }

    flush_pending_delta(&mut runtime_events, pending_delta.take());
    if !runtime_events.is_empty() {
        repository.append_runtime_events(&runtime_events).await?;
    }

    Ok(())
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
    tokio::spawn(async move {
        let Ok(mut subscription) = stream.subscribe(run_id, Some(0)).await else {
            warn!(
                flow_run_id = %run_id,
                "failed to subscribe runtime debug stream for durable event persistence"
            );
            return;
        };

        let mut batch = RuntimeEventPersistenceBatch::default();
        for event in subscription.replay {
            if push_debug_event_for_persistence(&repository, &mut batch, run_id, event).await {
                return;
            }
        }

        let start = tokio::time::Instant::now() + RUNTIME_EVENT_BATCH_MAX_DELAY;
        let mut flush_interval = tokio::time::interval_at(start, RUNTIME_EVENT_BATCH_MAX_DELAY);
        loop {
            tokio::select! {
                maybe_event = subscription.live_events.recv() => {
                    let Some(event) = maybe_event else {
                        let _ = flush_debug_event_batch(&repository, &mut batch, run_id).await;
                        return;
                    };
                    if push_debug_event_for_persistence(&repository, &mut batch, run_id, event).await {
                        return;
                    }
                }
                _ = flush_interval.tick(), if !batch.is_empty() => {
                    if flush_debug_event_batch(&repository, &mut batch, run_id).await {
                        return;
                    }
                }
            }
        }
    })
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
    batch: &mut RuntimeEventPersistenceBatch,
    run_id: Uuid,
    event: RuntimeEventEnvelope,
) -> bool
where
    R: OrchestrationRuntimeRepository,
{
    let is_terminal = is_terminal_runtime_event(&event.event_type);
    batch.push(event);
    if is_terminal || batch.reached_byte_limit() {
        return flush_debug_event_batch(repository, batch, run_id).await || is_terminal;
    }
    false
}

async fn flush_debug_event_batch<R>(
    repository: &R,
    batch: &mut RuntimeEventPersistenceBatch,
    run_id: Uuid,
) -> bool
where
    R: OrchestrationRuntimeRepository,
{
    if batch.is_empty() {
        return false;
    }

    let has_terminal = batch.contains_terminal();
    let events = batch.take();
    if let Err(error) = persist_runtime_debug_stream_events(repository, events).await {
        warn!(
            flow_run_id = %run_id,
            error = %error,
            "failed to persist runtime debug stream events"
        );
    }

    has_terminal
}

fn is_stream_delta_event(event_type: &str) -> bool {
    event_type == "text_delta" || event_type == "reasoning_delta"
}

fn is_terminal_runtime_event(event_type: &str) -> bool {
    RuntimeEventCloseReason::from_terminal_event_type(event_type).is_some()
}

struct PendingStreamDelta {
    run_id: Uuid,
    node_run_id: Option<Uuid>,
    sequence_start: i64,
    sequence_end: i64,
    event_ids: Vec<String>,
    event_type: String,
    content_type: Option<String>,
    source: crate::ports::RuntimeEventSource,
    node_id: Option<Value>,
    presentation: Option<Value>,
    text: String,
    content_refs: Vec<String>,
    artifact_refs: Vec<String>,
    truncated: bool,
    truncation_reason: Option<String>,
    original_bytes: Option<i64>,
}

impl PendingStreamDelta {
    fn has_same_projection_identity(&self, event: &RuntimeEventEnvelope) -> bool {
        self.node_id.as_ref() == event.payload.get("node_id")
            && self.presentation.as_ref() == event.payload.get("presentation")
    }

    fn merge_metadata(&mut self, event: &RuntimeEventEnvelope) {
        collect_string_field(&event.payload, "text_ref", &mut self.content_refs);
        collect_string_field(&event.payload, "content_ref", &mut self.content_refs);
        collect_string_field(&event.payload, "payload_ref", &mut self.content_refs);
        collect_string_array_field(&event.payload, "content_refs", &mut self.content_refs);
        collect_string_array_field(&event.payload, "payload_refs", &mut self.content_refs);
        collect_string_field(&event.payload, "artifact_ref", &mut self.artifact_refs);
        collect_string_array_field(&event.payload, "artifact_refs", &mut self.artifact_refs);
        for content_ref in self.content_refs.clone() {
            if content_ref.starts_with("runtime_artifact:") {
                push_unique(&mut self.artifact_refs, content_ref);
            }
        }

        let truncation = event.payload.get("truncation").and_then(Value::as_object);
        let event_truncated = event
            .payload
            .get("truncated")
            .or_else(|| event.payload.get("is_truncated"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || truncation
                .and_then(|value| value.get("truncated"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        self.truncated = self.truncated || event_truncated;

        if self.truncation_reason.is_none() {
            self.truncation_reason = event
                .payload
                .get("truncation_reason")
                .and_then(Value::as_str)
                .or_else(|| {
                    truncation
                        .and_then(|value| value.get("reason"))
                        .and_then(Value::as_str)
                })
                .map(ToString::to_string);
        }

        let original_bytes = event
            .payload
            .get("original_bytes")
            .and_then(Value::as_i64)
            .or_else(|| {
                truncation
                    .and_then(|value| value.get("original_bytes"))
                    .and_then(Value::as_i64)
            });
        if let Some(original_bytes) = original_bytes {
            self.original_bytes = Some(self.original_bytes.unwrap_or(0).max(original_bytes));
        }
    }
}

fn flush_pending_delta(
    runtime_events: &mut Vec<AppendRuntimeEventInput>,
    pending_delta: Option<PendingStreamDelta>,
) {
    let Some(pending) = pending_delta else {
        return;
    };
    let event_type = pending.event_type;
    let node_run_id = pending.node_run_id.map(|value| value.to_string());
    let stored_bytes = pending.text.len() as i64;
    let original_bytes = pending.original_bytes.unwrap_or(stored_bytes);
    let content_refs = pending.content_refs;
    let artifact_refs = pending.artifact_refs;
    let mut payload = json!({
        "type": event_type.clone(),
        "event_type": event_type.clone(),
        "node_run_id": node_run_id,
        "text": pending.text,
        "content_type": pending.content_type,
        "stream_sequence": pending.sequence_end,
        "sequence_start": pending.sequence_start,
        "sequence_end": pending.sequence_end,
        "event_ids": pending.event_ids,
        "truncated": pending.truncated,
        "truncation": {
            "truncated": pending.truncated,
            "reason": pending.truncation_reason,
            "original_bytes": original_bytes,
            "stored_bytes": stored_bytes,
        },
        "content_refs": content_refs.clone(),
        "artifact_refs": artifact_refs.clone(),
        "refs": {
            "content": content_refs,
            "artifacts": artifact_refs,
        },
    });
    if let Some(object) = payload.as_object_mut() {
        if let Some(node_id) = pending.node_id {
            object.insert("node_id".to_string(), node_id);
        }
        if let Some(presentation) = pending.presentation {
            object.insert("presentation".to_string(), presentation);
        }
    }

    runtime_events.push(build_runtime_event_input(
        pending.run_id,
        pending.node_run_id,
        event_type,
        pending.source,
        payload,
    ));
}

fn payload_with_stream_sequence(
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

fn build_runtime_event_input(
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

fn collect_string_field(payload: &Value, key: &str, output: &mut Vec<String>) {
    if let Some(value) = payload.get(key).and_then(Value::as_str) {
        push_unique(output, value.to_string());
    }
}

fn collect_string_array_field(payload: &Value, key: &str, output: &mut Vec<String>) {
    let Some(values) = payload.get(key).and_then(Value::as_array) else {
        return;
    };

    for value in values.iter().filter_map(Value::as_str) {
        push_unique(output, value.to_string());
    }
}

fn push_unique(output: &mut Vec<String>, value: String) {
    if !output.iter().any(|existing| existing == &value) {
        output.push(value);
    }
}
