use axum::response::sse::Event;
use control_plane::ports::{
    OrchestrationRuntimeRepository, RuntimeEventReplayWindow, RuntimeEventStream,
};
use serde::Serialize;
use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;
pub type DebugRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;

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

pub fn payload_i64(payload: &serde_json::Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}
pub fn payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}
pub fn durable_event_stream_sequence(event: &domain::RuntimeEventRecord) -> i64 {
    payload_i64(&event.payload, "sequence_end")
        .or_else(|| payload_i64(&event.payload, "stream_sequence"))
        .unwrap_or(event.sequence)
}
pub fn to_runtime_event_record_response(
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
pub trait RuntimeEventSsePort: Send + Sync {
    fn websocket_send(
        self: Arc<Self>,
        stream: Arc<dyn RuntimeEventStream>,
        backfill: Arc<dyn RuntimeEventBackfillSource>,
        run_id: Uuid,
        from_sequence: Option<i64>,
        sender: tokio::sync::mpsc::Sender<serde_json::Value>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
    fn send(
        self: Arc<Self>,
        stream: Arc<dyn RuntimeEventStream>,
        backfill: Arc<dyn RuntimeEventBackfillSource>,
        run_id: Uuid,
        from_sequence: Option<i64>,
        sender: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
}
