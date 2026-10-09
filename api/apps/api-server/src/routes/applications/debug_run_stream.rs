use std::{convert::Infallible, sync::Arc};

use axum::response::sse::Event;
use control_plane::ports::{
    OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventEnvelope,
    RuntimeEventReplayWindow, RuntimeEventStream,
};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use tokio::sync::mpsc;
use uuid::Uuid;

pub type DebugRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;
const DURABLE_BACKFILL_PAGE_SIZE: usize = 1_000;

#[async_trait::async_trait]
pub trait RuntimeEventBackfillSource: Send + Sync {
    async fn get_runtime_event_replay_window(
        &self,
        run_id: Uuid,
    ) -> anyhow::Result<Option<RuntimeEventReplayWindow>>;

    async fn list_runtime_event_durable_page(
        &self,
        run_id: Uuid,
        after_sequence: i64,
        through_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>>;
}

#[async_trait::async_trait]
impl<T> RuntimeEventBackfillSource for T
where
    T: OrchestrationRuntimeRepository + Send + Sync,
{
    async fn get_runtime_event_replay_window(
        &self,
        run_id: Uuid,
    ) -> anyhow::Result<Option<RuntimeEventReplayWindow>> {
        OrchestrationRuntimeRepository::get_runtime_event_replay_window(self, run_id).await
    }
    async fn list_runtime_event_durable_page(
        &self,
        run_id: Uuid,
        after_sequence: i64,
        through_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
        OrchestrationRuntimeRepository::list_runtime_event_durable_page(
            self,
            run_id,
            after_sequence,
            through_sequence,
            limit,
        )
        .await
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RuntimeEventReplayExpiredResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub event_id: Option<String>,
    pub run_id: String,
    pub from_sequence: Option<i64>,
    pub reason: &'static str,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RuntimeEventReplayGapResponse {
    #[serde(rename = "type")]
    pub response_type: &'static str,
    pub run_id: String,
    pub from_sequence: Option<i64>,
    pub reason: &'static str,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum RuntimeDebugSseEventResponse {
    Event(RuntimeEventStreamEnvelopeResponse),
    ReplayExpired(RuntimeEventReplayExpiredResponse),
    DurableBackfill(RuntimeEventDurableBackfillResponse),
    ReplayGap(RuntimeEventReplayGapResponse),
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

pub(crate) fn runtime_event_to_websocket_value(
    envelope: RuntimeEventEnvelope,
) -> serde_json::Value {
    let response = to_runtime_event_stream_envelope_response(envelope);
    let mut value =
        serde_json::to_value(&response).expect("runtime event WebSocket envelope should serialize");
    value["type"] = serde_json::Value::String(response.event_type);
    value
}

pub(crate) async fn send_runtime_event_websocket_stream(
    stream: Arc<dyn RuntimeEventStream>,
    backfill_source: Arc<dyn RuntimeEventBackfillSource>,
    run_id: Uuid,
    from_sequence: Option<i64>,
    sender: mpsc::Sender<serde_json::Value>,
) {
    send_debug_stream(
        stream,
        backfill_source,
        run_id,
        from_sequence,
        DebugSender::WebSocket(sender),
    )
    .await;
}

/// Both transports use the same finite-page replay and await downstream capacity.
enum DebugSender {
    Sse(mpsc::Sender<Result<Event, Infallible>>),
    WebSocket(mpsc::Sender<serde_json::Value>),
}

impl DebugSender {
    async fn closed(&self) {
        match self {
            Self::Sse(sender) => sender.closed().await,
            Self::WebSocket(sender) => sender.closed().await,
        }
    }
    async fn envelope(&self, event: RuntimeEventEnvelope) -> bool {
        match self {
            Self::Sse(sender) => sender.send(runtime_event_to_sse(event)).await.is_ok(),
            Self::WebSocket(sender) => sender
                .send(runtime_event_to_websocket_value(event))
                .await
                .is_ok(),
        }
    }
    async fn record(&self, event: domain::RuntimeEventRecord) -> bool {
        match self {
            Self::Sse(sender) => sender
                .send(runtime_event_record_to_sse(event))
                .await
                .is_ok(),
            Self::WebSocket(sender) => {
                let response = to_runtime_event_record_response(event);
                let mut value =
                    serde_json::to_value(&response).expect("durable envelope should serialize");
                value["type"] = serde_json::Value::String(response.event_type);
                sender.send(value).await.is_ok()
            }
        }
    }
    async fn marker<T: Serialize>(&self, kind: &str, response: T) -> bool {
        match self {
            Self::Sse(sender) => sender
                .send(Ok(Event::default()
                    .event(kind)
                    .json_data(response)
                    .expect("debug marker should serialize")))
                .await
                .is_ok(),
            Self::WebSocket(sender) => sender
                .send(serde_json::to_value(response).expect("debug marker should serialize"))
                .await
                .is_ok(),
        }
    }
    async fn gap(&self, run_id: Uuid, from_sequence: Option<i64>, reason: &'static str) {
        self.marker(
            "replay_gap",
            RuntimeEventReplayGapResponse {
                response_type: "replay_gap",
                run_id: run_id.to_string(),
                from_sequence,
                reason,
            },
        )
        .await;
    }
    async fn unavailable(&self, run_id: Uuid, from_sequence: Option<i64>, reason: &'static str) {
        if self
            .marker(
                "replay_expired",
                to_replay_expired_response(run_id, from_sequence),
            )
            .await
        {
            self.gap(run_id, from_sequence, reason).await;
        }
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

pub(crate) fn to_runtime_event_record_response(
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

fn is_terminal_runtime_event(event_type: &str) -> bool {
    RuntimeEventCloseReason::from_terminal_event_type(event_type).is_some()
}

pub async fn send_runtime_event_stream(
    stream: Arc<dyn RuntimeEventStream>,
    backfill_source: Arc<dyn RuntimeEventBackfillSource>,
    run_id: Uuid,
    from_sequence: Option<i64>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
) {
    send_debug_stream(
        stream,
        backfill_source,
        run_id,
        from_sequence,
        DebugSender::Sse(sender),
    )
    .await;
}

async fn send_debug_stream(
    stream: Arc<dyn RuntimeEventStream>,
    backfill_source: Arc<dyn RuntimeEventBackfillSource>,
    run_id: Uuid,
    from_sequence: Option<i64>,
    sender: DebugSender,
) {
    let subscription = tokio::select! {
        _ = sender.closed() => return,
        result = stream.subscribe(run_id, from_sequence) => result,
    };
    let mut subscription = match subscription {
        Ok(subscription) => subscription,
        Err(_) => {
            // Read scalar generation authority without allocating a cursorless
            // hot replay. An open generation cannot safely be joined from a
            // semantic DB reconstruction alone.
            let writer = tokio::select! {
                _ = sender.closed() => return,
                result = stream.terminal_writer(run_id) => result.ok(),
            };
            let completed =
                send_durable_backfill(backfill_source, run_id, from_sequence, &sender).await;
            if completed
                && writer
                    .as_ref()
                    .is_some_and(|writer| writer.closure().is_none())
            {
                sender
                    .gap(run_id, from_sequence, "unresolved_live_gap")
                    .await;
            }
            return;
        }
    };
    for event in subscription.replay {
        let terminal = is_terminal_runtime_event(&event.event_type);
        if !sender.envelope(event).await || terminal {
            return;
        }
    }
    loop {
        let event = tokio::select! {
            _ = sender.closed() => return,
            event = subscription.live_events.recv() => event,
        };
        let Some(event) = event else {
            sender
                .gap(run_id, from_sequence, "live_stream_closed_without_terminal")
                .await;
            return;
        };
        let terminal = is_terminal_runtime_event(&event.event_type);
        if !sender.envelope(event).await || terminal {
            return;
        }
    }
}

async fn send_durable_backfill(
    backfill_source: Arc<dyn RuntimeEventBackfillSource>,
    run_id: Uuid,
    from_sequence: Option<i64>,
    sender: &DebugSender,
) -> bool {
    let window = tokio::select! {
        _ = sender.closed() => return false,
        result = backfill_source.get_runtime_event_replay_window(run_id) => result,
    };
    let window = match window {
        Ok(Some(window)) => window,
        Ok(None) => {
            sender
                .unavailable(run_id, from_sequence, "durable_history_unavailable")
                .await;
            return false;
        }
        Err(_) => {
            sender
                .unavailable(run_id, from_sequence, "durable_backfill_failed")
                .await;
            return false;
        }
    };
    let mut durable_cursor = window.after_sequence;
    let mut read_any = false;
    while durable_cursor < window.through_sequence {
        let page = tokio::select! {
            _ = sender.closed() => return false,
            result = backfill_source.list_runtime_event_durable_page(run_id, durable_cursor, Some(window.through_sequence), DURABLE_BACKFILL_PAGE_SIZE) => result,
        };
        let mut events = match page {
            Ok(events) => events,
            Err(_) => {
                sender
                    .unavailable(run_id, from_sequence, "durable_backfill_failed")
                    .await;
                return false;
            }
        };
        if events.is_empty() {
            break;
        }
        let next = events.last().expect("nonempty durable page").sequence;
        if next <= durable_cursor {
            sender
                .unavailable(run_id, from_sequence, "durable_backfill_failed")
                .await;
            return false;
        }
        durable_cursor = next;
        read_any = true;
        // Local cursors only apply to explicit local sequence metadata. Durable
        // facts without it remain necessary for semantic reconstruction.
        events.retain(|event| {
            if event.event_type == "runtime_stream_opened" {
                return false;
            }
            if let (Some(generation), Some(tag)) = (
                window.generation_id,
                event
                    .payload
                    .get("stream_generation_id")
                    .and_then(serde_json::Value::as_str),
            ) {
                if Uuid::parse_str(tag).ok() != Some(generation) {
                    return false;
                }
            }
            let local = payload_i64(&event.payload, "sequence_end")
                .or_else(|| payload_i64(&event.payload, "stream_sequence"));
            from_sequence.is_none_or(|cursor| local.is_none_or(|sequence| sequence > cursor))
        });
        let marker = RuntimeEventDurableBackfillResponse {
            response_type: "durable_backfill",
            run_id: run_id.to_string(),
            from_sequence,
            first_sequence: events
                .first()
                .map(durable_event_stream_sequence)
                .unwrap_or(from_sequence.unwrap_or(0)),
            last_sequence: events
                .last()
                .map(durable_event_stream_sequence)
                .unwrap_or(from_sequence.unwrap_or(0)),
            event_count: events.len(),
            has_more: durable_cursor < window.through_sequence,
            reason: if window.generation_id.is_some() {
                "durable_semantic_reconstruction"
            } else {
                "legacy_durable_semantic_reconstruction"
            },
        };
        if !sender.marker("durable_backfill", marker).await {
            return false;
        }
        for event in events {
            if !sender.record(event).await {
                return false;
            }
        }
    }
    if !read_any {
        sender
            .unavailable(run_id, from_sequence, "durable_history_unavailable")
            .await;
    }
    read_any
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
        window: std::sync::Mutex<Option<RuntimeEventReplayWindow>>,
        fail_page: std::sync::atomic::AtomicBool,
        stall_page: std::sync::atomic::AtomicBool,
    }

    #[async_trait::async_trait]
    impl RuntimeEventBackfillSource for RecordingBackfillSource {
        async fn get_runtime_event_replay_window(
            &self,
            _run_id: Uuid,
        ) -> anyhow::Result<Option<RuntimeEventReplayWindow>> {
            if let Some(window) = *self.window.lock().unwrap() {
                return Ok(Some(window));
            }
            Ok(self
                .events
                .lock()
                .unwrap()
                .last()
                .map(|event| RuntimeEventReplayWindow {
                    after_sequence: 0,
                    through_sequence: event.sequence,
                    generation_id: None,
                }))
        }
        async fn list_runtime_event_durable_page(
            &self,
            _run_id: Uuid,
            after_sequence: i64,
            through_sequence: Option<i64>,
            limit: usize,
        ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_page.load(Ordering::SeqCst) {
                anyhow::bail!("fixture DB failure");
            }
            if self.stall_page.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            Ok(self
                .events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| {
                    event.sequence > after_sequence
                        && through_sequence.is_none_or(|end| event.sequence <= end)
                })
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
    async fn issue_1601_websocket_stream_reports_live_close_without_terminal() {
        let stream = Arc::new(LocalRuntimeEventStream::new());
        let run_id = Uuid::now_v7();
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let (sender, mut receiver) = mpsc::channel(8);
        let handle = tokio::spawn(send_runtime_event_websocket_stream(
            stream.clone(),
            Arc::new(RecordingBackfillSource::default()),
            run_id,
            None,
            sender,
        ));

        stream
            .close_run(run_id, RuntimeEventCloseReason::Finished)
            .await
            .unwrap();

        let gap = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("WebSocket stream should report the missing terminal")
            .expect("replay gap should be emitted");
        assert_eq!(gap["type"], "replay_gap");
        assert_eq!(gap["reason"], "live_stream_closed_without_terminal");
        handle.await.unwrap();
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
    async fn send_runtime_event_stream_returns_after_flow_incomplete_terminal_event() {
        let stream = Arc::new(LocalRuntimeEventStream::new());
        let run_id = Uuid::now_v7();
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let (sender, mut receiver) = mpsc::channel(8);
        let backfill = Arc::new(RecordingBackfillSource::default());

        let handle = tokio::spawn(send_runtime_event_stream(
            stream.clone(),
            backfill,
            run_id,
            None,
            sender,
        ));
        stream
            .append(run_id, runtime_event("flow_incomplete"))
            .await
            .unwrap();

        let _ = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("incomplete terminal event should be sent")
            .expect("incomplete terminal event should be available")
            .expect("sse event should be valid");

        let closed = timeout(Duration::from_millis(100), receiver.recv()).await;
        if !matches!(closed, Ok(None)) {
            handle.abort();
        }
        assert!(
            matches!(closed, Ok(None)),
            "sender should close after flow_incomplete terminal event"
        );
        handle
            .await
            .expect("incomplete terminal stream task should finish");
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
        assert!(receiver
            .recv()
            .await
            .expect("active cold gap should be explicit")
            .is_ok());
        assert!(receiver.recv().await.is_none());
        assert_eq!(backfill.calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn cold_websocket_replays_more_than_one_durable_page() {
        let run_id = Uuid::now_v7();
        let backfill = Arc::new(RecordingBackfillSource::default());
        *backfill.events.lock().unwrap() = (1..=1005)
            .map(|sequence| {
                durable_runtime_event(
                    run_id,
                    if sequence == 1005 {
                        "flow_finished"
                    } else {
                        "opaque_item"
                    },
                    sequence,
                )
            })
            .collect();
        let (sender, mut receiver) = mpsc::channel(2);
        let handle = tokio::spawn(send_runtime_event_websocket_stream(
            Arc::new(LocalRuntimeEventStream::new()),
            backfill.clone(),
            run_id,
            None,
            sender,
        ));
        let mut records = 0;
        let mut markers = Vec::new();
        while let Some(event) = receiver.recv().await {
            match event["type"].as_str().unwrap() {
                "durable_backfill" => markers.push(event),
                "opaque_item" | "flow_finished" => records += 1,
                other => panic!("successful cold read must not warn: {other}"),
            }
        }
        handle.await.unwrap();
        assert_eq!(records, 1005);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0]["has_more"], true);
        assert_eq!(markers[1]["has_more"], false);
        assert_eq!(backfill.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cold_generation_uses_db_anchor_independently_of_local_cursor() {
        let run_id = Uuid::now_v7();
        let backfill = Arc::new(RecordingBackfillSource::default());
        *backfill.window.lock().unwrap() = Some(RuntimeEventReplayWindow {
            after_sequence: 5000,
            through_sequence: 5003,
            generation_id: Some(Uuid::now_v7()),
        });
        let mut acknowledged = durable_runtime_event(run_id, "opaque_item", 5001);
        acknowledged.payload = json!({"stream_sequence": 1});
        let mut fact = durable_runtime_event(run_id, "opaque_item", 5002);
        fact.payload = json!({"opaque": {"untouched": true}});
        let mut terminal = durable_runtime_event(run_id, "flow_failed", 5003);
        terminal.payload = json!({"stream_sequence": 3, "error": {"message": "upstream-original"}});
        *backfill.events.lock().unwrap() = vec![acknowledged, fact.clone(), terminal.clone()];
        let (sender, mut receiver) = mpsc::channel(8);
        send_runtime_event_websocket_stream(
            Arc::new(LocalRuntimeEventStream::new()),
            backfill,
            run_id,
            Some(1),
            sender,
        )
        .await;
        let marker = receiver.recv().await.unwrap();
        assert_eq!(marker["event_count"], 2);
        assert_eq!(marker["reason"], "durable_semantic_reconstruction");
        assert_eq!(receiver.recv().await.unwrap()["payload"], fact.payload);
        let failure = receiver.recv().await.unwrap();
        assert_eq!(failure["sequence"], 3);
        assert_eq!(failure["payload"], terminal.payload);
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn cold_db_failure_keeps_existing_failure_messages() {
        let run_id = Uuid::now_v7();
        let backfill = Arc::new(RecordingBackfillSource::default());
        backfill
            .events
            .lock()
            .unwrap()
            .push(durable_runtime_event(run_id, "flow_finished", 1));
        backfill.fail_page.store(true, Ordering::SeqCst);
        let (sender, mut receiver) = mpsc::channel(8);
        send_runtime_event_websocket_stream(
            Arc::new(LocalRuntimeEventStream::new()),
            backfill,
            run_id,
            None,
            sender,
        )
        .await;
        assert_eq!(receiver.recv().await.unwrap()["type"], "replay_expired");
        let gap = receiver.recv().await.unwrap();
        assert_eq!(gap["type"], "replay_gap");
        assert_eq!(gap["reason"], "durable_backfill_failed");
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn receiver_drop_cancels_inflight_cold_page_read() {
        let run_id = Uuid::now_v7();
        let backfill = Arc::new(RecordingBackfillSource::default());
        backfill
            .events
            .lock()
            .unwrap()
            .push(durable_runtime_event(run_id, "flow_finished", 1));
        backfill.stall_page.store(true, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel(1);
        let handle = tokio::spawn(send_runtime_event_websocket_stream(
            Arc::new(LocalRuntimeEventStream::new()),
            backfill.clone(),
            run_id,
            None,
            sender,
        ));
        timeout(Duration::from_secs(1), async {
            while backfill.calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(receiver);
        timeout(Duration::from_secs(1), handle)
            .await
            .expect("dropped receiver must cancel DB wait")
            .unwrap();
    }

    #[tokio::test]
    async fn receiver_drop_cancels_idle_hot_stream() {
        let run_id = Uuid::now_v7();
        let stream = Arc::new(LocalRuntimeEventStream::new());
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let (sender, receiver) = mpsc::channel(1);
        let handle = tokio::spawn(send_runtime_event_websocket_stream(
            stream,
            Arc::new(RecordingBackfillSource::default()),
            run_id,
            None,
            sender,
        ));
        drop(receiver);
        timeout(Duration::from_secs(1), handle)
            .await
            .expect("idle forwarder must observe receiver closure")
            .unwrap();
    }
    #[tokio::test]
    async fn closed_hot_cursor_miss_uses_full_cold_sse_without_failure_markers() {
        let run_id = Uuid::now_v7();
        let stream = Arc::new(LocalRuntimeEventStream::new());
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        stream
            .append(run_id, runtime_event("flow_finished"))
            .await
            .unwrap();
        stream
            .close_run(run_id, RuntimeEventCloseReason::Finished)
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
        let backfill = Arc::new(RecordingBackfillSource::default());
        *backfill.events.lock().unwrap() = (1..=1005)
            .map(|sequence| {
                durable_runtime_event(
                    run_id,
                    if sequence == 1005 {
                        "flow_finished"
                    } else {
                        "opaque_item"
                    },
                    sequence,
                )
            })
            .collect();
        let (sender, mut receiver) = mpsc::channel(2);
        let handle = tokio::spawn(send_runtime_event_stream(
            stream,
            backfill.clone(),
            run_id,
            Some(0),
            sender,
        ));
        let mut count = 0;
        while let Some(event) = receiver.recv().await {
            assert!(event.is_ok());
            count += 1;
        }
        handle.await.unwrap();
        // Exactly two page markers and every durable fact; replay_expired,
        // replay_gap or a truncated first page would change this count.
        assert_eq!(count, 1007);
        assert_eq!(backfill.calls.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn cold_generation_excludes_prior_generation_late_rows_and_anchors() {
        let run_id = Uuid::now_v7();
        let generation = Uuid::now_v7();
        let backfill = Arc::new(RecordingBackfillSource::default());
        *backfill.window.lock().unwrap() = Some(RuntimeEventReplayWindow {
            after_sequence: 5000,
            through_sequence: 5004,
            generation_id: Some(generation),
        });
        let mut late = durable_runtime_event(run_id, "flow_failed", 5001);
        late.payload = json!({"stream_generation_id": Uuid::now_v7(), "stream_sequence": 9, "error": "old-generation"});
        let mut anchor = durable_runtime_event(run_id, "runtime_stream_opened", 5002);
        anchor.payload = json!({"stream_generation_id": generation});
        let mut fact = durable_runtime_event(run_id, "opaque_item", 5003);
        fact.payload = json!({"business_fact": "untagged-must-survive"});
        let mut terminal = durable_runtime_event(run_id, "flow_finished", 5004);
        terminal.payload = json!({"stream_generation_id": generation, "stream_sequence": 2});
        *backfill.events.lock().unwrap() = vec![late, anchor, fact.clone(), terminal.clone()];
        let (sender, mut receiver) = mpsc::channel(8);
        send_runtime_event_websocket_stream(
            Arc::new(LocalRuntimeEventStream::new()),
            backfill,
            run_id,
            None,
            sender,
        )
        .await;
        assert_eq!(receiver.recv().await.unwrap()["event_count"], 2);
        assert_eq!(receiver.recv().await.unwrap()["payload"], fact.payload);
        assert_eq!(receiver.recv().await.unwrap()["payload"], terminal.payload);
        assert!(receiver.recv().await.is_none());
    }
}
