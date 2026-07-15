use std::{convert::Infallible, sync::Arc};

use axum::response::sse::Event;
use control_plane::ports::{
    OrchestrationRuntimeRepository, RuntimeEventEnvelope, RuntimeEventStream,
};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use tokio::sync::mpsc;
use uuid::Uuid;

pub type DebugRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;
const DURABLE_BACKFILL_PAGE_SIZE: usize = 1_000;

#[async_trait::async_trait]
pub trait RuntimeEventBackfillSource: Send + Sync {
    async fn list_runtime_event_backfill_page(
        &self,
        run_id: Uuid,
        after_stream_sequence: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>>;
}

#[async_trait::async_trait]
impl<T> RuntimeEventBackfillSource for T
where
    T: OrchestrationRuntimeRepository + Send + Sync,
{
    async fn list_runtime_event_backfill_page(
        &self,
        run_id: Uuid,
        after_stream_sequence: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
        OrchestrationRuntimeRepository::list_runtime_event_backfill_page(
            self,
            run_id,
            after_stream_sequence,
            limit,
        )
        .await
    }
}

#[derive(Debug, Serialize)]
pub struct RuntimeEventStreamEnvelopeResponse {
    pub event_id: String,
    pub run_id: String,
    pub node_run_id: Option<String>,
    pub event_type: String,
    pub sequence: i64,
    pub created_at: String,
    pub payload: serde_json::Value,
    pub delta_index: Option<i64>,
    pub content_type: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeEventReplayExpiredResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub event_id: Option<String>,
    pub run_id: String,
    pub from_sequence: Option<i64>,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RuntimeEventDurableBackfillResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub run_id: String,
    pub from_sequence: Option<i64>,
    pub first_sequence: i64,
    pub last_sequence: i64,
    pub event_count: usize,
    pub has_more: bool,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RuntimeEventReplayGapResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub run_id: String,
    pub from_sequence: Option<i64>,
    pub reason: &'static str,
}

fn to_runtime_event_stream_envelope_response(
    envelope: RuntimeEventEnvelope,
) -> RuntimeEventStreamEnvelopeResponse {
    RuntimeEventStreamEnvelopeResponse {
        event_id: envelope.event_id,
        run_id: envelope.run_id.to_string(),
        node_run_id: envelope.node_run_id.map(|value| value.to_string()),
        event_type: envelope.event_type,
        sequence: envelope.sequence,
        created_at: envelope
            .occurred_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| envelope.occurred_at.to_string()),
        payload: envelope.payload,
        delta_index: envelope.delta_index,
        content_type: envelope.content_type,
        text: envelope.text,
    }
}

fn payload_i64(payload: &serde_json::Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

fn payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn durable_event_stream_sequence(event: &domain::RuntimeEventRecord) -> i64 {
    payload_i64(&event.payload, "sequence_end")
        .or_else(|| payload_i64(&event.payload, "stream_sequence"))
        .unwrap_or(event.sequence)
}

fn to_runtime_event_record_response(
    event: domain::RuntimeEventRecord,
) -> RuntimeEventStreamEnvelopeResponse {
    let sequence = durable_event_stream_sequence(&event);
    let delta_index = payload_i64(&event.payload, "delta_index")
        .or_else(|| payload_i64(&event.payload, "sequence_start"));
    let content_type = payload_string(&event.payload, "content_type");
    let text =
        payload_string(&event.payload, "text").or_else(|| payload_string(&event.payload, "delta"));
    RuntimeEventStreamEnvelopeResponse {
        event_id: format!("{}:{sequence}", event.flow_run_id),
        run_id: event.flow_run_id.to_string(),
        node_run_id: event.node_run_id.map(|value| value.to_string()),
        event_type: event.event_type,
        sequence,
        created_at: event
            .created_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| event.created_at.to_string()),
        payload: event.payload,
        delta_index,
        content_type,
        text,
    }
}

pub fn runtime_event_to_sse(envelope: RuntimeEventEnvelope) -> Result<Event, Infallible> {
    let event_id = envelope.event_id.clone();
    let event_type = envelope.event_type.clone();

    Ok(Event::default()
        .id(event_id)
        .event(event_type)
        .json_data(to_runtime_event_stream_envelope_response(envelope))
        .expect("runtime event envelope should serialize"))
}

pub fn runtime_event_record_to_sse(event: domain::RuntimeEventRecord) -> Result<Event, Infallible> {
    let event_type = event.event_type.clone();
    let response = to_runtime_event_record_response(event);
    Ok(Event::default()
        .id(response.event_id.clone())
        .event(event_type)
        .json_data(response)
        .expect("runtime event record should serialize"))
}

fn to_replay_expired_response(
    run_id: Uuid,
    from_sequence: Option<i64>,
) -> RuntimeEventReplayExpiredResponse {
    RuntimeEventReplayExpiredResponse {
        response_type: "replay_expired",
        event_id: from_sequence.map(|sequence| format!("{run_id}:{sequence}")),
        run_id: run_id.to_string(),
        from_sequence,
        reason: "cursor_expired",
    }
}

pub fn replay_expired_to_sse(
    run_id: Uuid,
    from_sequence: Option<i64>,
) -> Result<Event, Infallible> {
    let payload = to_replay_expired_response(run_id, from_sequence);
    let mut event = Event::default().event("replay_expired");
    if let Some(event_id) = &payload.event_id {
        event = event.id(event_id.clone());
    }

    Ok(event
        .json_data(payload)
        .expect("replay_expired payload should serialize"))
}

fn durable_backfill_to_sse(
    run_id: Uuid,
    from_sequence: Option<i64>,
    events: &[domain::RuntimeEventRecord],
    has_more: bool,
) -> Result<Event, Infallible> {
    let first_sequence = events
        .first()
        .map(durable_event_stream_sequence)
        .unwrap_or_else(|| from_sequence.unwrap_or(0));
    let last_sequence = events
        .last()
        .map(durable_event_stream_sequence)
        .unwrap_or(first_sequence);
    let payload = RuntimeEventDurableBackfillResponse {
        response_type: "durable_backfill",
        run_id: run_id.to_string(),
        from_sequence,
        first_sequence,
        last_sequence,
        event_count: events.len(),
        has_more,
        reason: "cursor_expired",
    };
    Ok(Event::default()
        .event("durable_backfill")
        .json_data(payload)
        .expect("durable_backfill payload should serialize"))
}

fn replay_gap_to_sse(
    run_id: Uuid,
    from_sequence: Option<i64>,
    reason: &'static str,
) -> Result<Event, Infallible> {
    let payload = RuntimeEventReplayGapResponse {
        response_type: "replay_gap",
        run_id: run_id.to_string(),
        from_sequence,
        reason,
    };
    Ok(Event::default()
        .event("replay_gap")
        .json_data(payload)
        .expect("replay_gap payload should serialize"))
}

fn is_terminal_runtime_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human" | "waiting_callback"
    )
}

pub async fn send_runtime_event_stream(
    stream: Arc<dyn RuntimeEventStream>,
    backfill_source: Arc<dyn RuntimeEventBackfillSource>,
    run_id: Uuid,
    from_sequence: Option<i64>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
) {
    let Ok(mut subscription) = stream.subscribe(run_id, from_sequence).await else {
        send_durable_backfill(backfill_source, run_id, from_sequence, sender).await;
        return;
    };

    for event in subscription.replay {
        let is_terminal = is_terminal_runtime_event(&event.event_type);
        if sender.send(runtime_event_to_sse(event)).await.is_err() {
            return;
        }
        if is_terminal {
            return;
        }
    }

    while let Some(event) = subscription.live_events.recv().await {
        let is_terminal = is_terminal_runtime_event(&event.event_type);
        if sender.send(runtime_event_to_sse(event)).await.is_err() {
            return;
        }
        if is_terminal {
            return;
        }
    }
}

async fn send_durable_backfill(
    backfill_source: Arc<dyn RuntimeEventBackfillSource>,
    run_id: Uuid,
    from_sequence: Option<i64>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
) {
    let after_sequence = from_sequence.unwrap_or(0);
    let mut events = match backfill_source
        .list_runtime_event_backfill_page(run_id, after_sequence, DURABLE_BACKFILL_PAGE_SIZE + 1)
        .await
    {
        Ok(events) => events,
        Err(_) => {
            let _ = sender
                .send(replay_expired_to_sse(run_id, from_sequence))
                .await;
            let _ = sender
                .send(replay_gap_to_sse(
                    run_id,
                    from_sequence,
                    "durable_backfill_failed",
                ))
                .await;
            return;
        }
    };
    let has_more = events.len() > DURABLE_BACKFILL_PAGE_SIZE;
    if has_more {
        events.truncate(DURABLE_BACKFILL_PAGE_SIZE);
    }
    if events.is_empty() {
        let _ = sender
            .send(replay_expired_to_sse(run_id, from_sequence))
            .await;
        let _ = sender
            .send(replay_gap_to_sse(
                run_id,
                from_sequence,
                "durable_history_unavailable",
            ))
            .await;
        return;
    }

    if sender
        .send(durable_backfill_to_sse(
            run_id,
            from_sequence,
            &events,
            has_more,
        ))
        .await
        .is_err()
    {
        return;
    }
    for event in events {
        if sender
            .send(runtime_event_record_to_sse(event))
            .await
            .is_err()
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use control_plane::ports::{
        RuntimeEventDurability, RuntimeEventPayload, RuntimeEventSource, RuntimeEventStreamPolicy,
        RuntimeEventTrimPolicy,
    };
    use serde_json::json;
    use time::OffsetDateTime;
    use tokio::time::{timeout, Duration};

    use crate::host_infrastructure::LocalRuntimeEventStream;

    fn runtime_event(event_type: &str) -> RuntimeEventPayload {
        RuntimeEventPayload {
            event_type: event_type.to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({ "type": event_type }),
        }
    }

    fn durable_runtime_event(
        run_id: Uuid,
        event_type: &str,
        sequence: i64,
    ) -> domain::RuntimeEventRecord {
        domain::RuntimeEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: run_id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            sequence,
            event_type: event_type.to_string(),
            layer: domain::RuntimeEventLayer::RuntimeItem,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload: json!({
                "type": event_type,
                "sequence_start": sequence,
                "sequence_end": sequence
            }),
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    #[derive(Default)]
    struct RecordingBackfillSource {
        calls: AtomicUsize,
        events: std::sync::Mutex<Vec<domain::RuntimeEventRecord>>,
    }

    #[async_trait::async_trait]
    impl RuntimeEventBackfillSource for RecordingBackfillSource {
        async fn list_runtime_event_backfill_page(
            &self,
            _run_id: Uuid,
            _after_stream_sequence: i64,
            limit: usize,
        ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .events
                .lock()
                .expect("backfill events lock should be available")
                .iter()
                .take(limit)
                .cloned()
                .collect())
        }
    }

    #[test]
    fn replay_expired_response_includes_cursor_contract() {
        let run_id = Uuid::now_v7();
        let payload = to_replay_expired_response(run_id, Some(42));
        let expected_event_id = format!("{run_id}:42");

        assert_eq!(payload.response_type, "replay_expired");
        assert_eq!(
            payload.event_id.as_deref(),
            Some(expected_event_id.as_str())
        );
        assert_eq!(payload.run_id, run_id.to_string());
        assert_eq!(payload.from_sequence, Some(42));
        assert_eq!(payload.reason, "cursor_expired");
    }

    #[tokio::test]
    async fn send_runtime_event_stream_returns_after_terminal_event() {
        let stream = Arc::new(LocalRuntimeEventStream::new());
        let run_id = Uuid::now_v7();
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let (sender, mut receiver) = mpsc::channel(8);
        let backfill = Arc::new(RecordingBackfillSource::default());

        tokio::spawn(send_runtime_event_stream(
            stream.clone(),
            backfill,
            run_id,
            None,
            sender,
        ));
        stream
            .append(run_id, runtime_event("flow_finished"))
            .await
            .unwrap();

        let _ = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("terminal event should be sent")
            .expect("terminal event should be available")
            .expect("sse event should be valid");

        let closed = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(
            matches!(closed, Ok(None)),
            "sender should close after terminal event"
        );
    }

    #[tokio::test]
    async fn send_runtime_event_stream_returns_after_flow_cancelled_terminal_event() {
        let stream = Arc::new(LocalRuntimeEventStream::new());
        let run_id = Uuid::now_v7();
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let (sender, mut receiver) = mpsc::channel(8);
        let backfill = Arc::new(RecordingBackfillSource::default());

        tokio::spawn(send_runtime_event_stream(
            stream.clone(),
            backfill,
            run_id,
            None,
            sender,
        ));
        stream
            .append(run_id, runtime_event("flow_cancelled"))
            .await
            .unwrap();

        let _ = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("cancelled terminal event should be sent")
            .expect("cancelled terminal event should be available")
            .expect("sse event should be valid");

        let closed = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(
            matches!(closed, Ok(None)),
            "sender should close after flow_cancelled terminal event"
        );
    }

    #[tokio::test]
    async fn send_runtime_event_stream_uses_single_durable_backfill_when_replay_expired() {
        let stream = Arc::new(LocalRuntimeEventStream::new());
        let backfill = Arc::new(RecordingBackfillSource::default());
        let run_id = Uuid::now_v7();

        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        stream
            .append(run_id, runtime_event("flow_started"))
            .await
            .unwrap();
        stream
            .trim(
                run_id,
                RuntimeEventTrimPolicy {
                    before_sequence: Some(2),
                    keep_required: false,
                },
            )
            .await
            .unwrap();
        backfill
            .events
            .lock()
            .expect("backfill events lock should be available")
            .push(durable_runtime_event(run_id, "flow_started", 1));
        let (sender, mut receiver) = mpsc::channel(8);

        send_runtime_event_stream(stream, backfill.clone(), run_id, Some(0), sender).await;

        let first = receiver
            .recv()
            .await
            .expect("backfill marker should be sent");
        let second = receiver.recv().await.expect("durable event should be sent");
        assert!(first.is_ok());
        assert!(second.is_ok());
        assert!(receiver.recv().await.is_none());
        assert_eq!(backfill.calls.load(Ordering::SeqCst), 1);
    }
}
