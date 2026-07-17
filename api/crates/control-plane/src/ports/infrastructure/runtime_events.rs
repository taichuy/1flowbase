use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventSource {
    Runtime,
    Provider,
    Persister,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventDurability {
    Ephemeral,
    DurableRequired,
    AuditRequired,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEventPayload {
    pub event_type: String,
    pub source: RuntimeEventSource,
    pub durability: RuntimeEventDurability,
    pub persist_required: bool,
    pub trace_visible: bool,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEventEnvelope {
    pub run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub sequence: i64,
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: time::OffsetDateTime,
    pub delta_index: Option<i64>,
    pub content_type: Option<String>,
    pub text: Option<String>,
    pub source: RuntimeEventSource,
    pub durability: RuntimeEventDurability,
    pub persist_required: bool,
    pub trace_visible: bool,
    pub payload: serde_json::Value,
}

impl RuntimeEventEnvelope {
    pub fn new(run_id: Uuid, sequence: i64, event: RuntimeEventPayload) -> Self {
        let node_run_id = event
            .payload
            .get("node_run_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        let text = event
            .payload
            .get("text")
            .or_else(|| event.payload.get("delta"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        let (delta_index, content_type) = match event.event_type.as_str() {
            "text_delta" => (
                Some(
                    event
                        .payload
                        .get("delta_index")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(sequence),
                ),
                Some("text".to_string()),
            ),
            "reasoning_delta" => (
                Some(
                    event
                        .payload
                        .get("delta_index")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(sequence),
                ),
                Some("reasoning".to_string()),
            ),
            _ => (None, None),
        };

        Self {
            run_id,
            node_run_id,
            sequence,
            event_id: format!("{run_id}:{sequence}"),
            event_type: event.event_type,
            occurred_at: time::OffsetDateTime::now_utc(),
            delta_index,
            content_type,
            text,
            source: event.source,
            durability: event.durability,
            persist_required: event.persist_required,
            trace_visible: event.trace_visible,
            payload: event.payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventOverflowBehavior {
    DropOldEphemeralKeepRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEventStreamPolicy {
    pub ttl: time::Duration,
    pub max_events: usize,
    pub max_bytes: usize,
    pub overflow_behavior: RuntimeEventOverflowBehavior,
}

impl RuntimeEventStreamPolicy {
    pub fn debug_default() -> Self {
        Self {
            ttl: time::Duration::hours(2),
            max_events: 20_000,
            max_bytes: 16 * 1024 * 1024,
            overflow_behavior: RuntimeEventOverflowBehavior::DropOldEphemeralKeepRequired,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventCloseReason {
    Finished,
    Incomplete,
    Failed,
    Cancelled,
    WaitingHuman,
    WaitingCallback,
    Expired,
}

impl RuntimeEventCloseReason {
    /// Maps the canonical terminal event to its only valid stream closure meaning.
    pub fn from_terminal_event_type(event_type: &str) -> Option<Self> {
        match event_type {
            "flow_finished" => Some(Self::Finished),
            "flow_incomplete" => Some(Self::Incomplete),
            "flow_failed" => Some(Self::Failed),
            "flow_cancelled" => Some(Self::Cancelled),
            "waiting_human" => Some(Self::WaitingHuman),
            "waiting_callback" => Some(Self::WaitingCallback),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendTerminalIfMissingAndCloseOutcome {
    Appended,
    ExistingTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEventClosure {
    pub reason: RuntimeEventCloseReason,
    pub final_sequence: i64,
}

pub struct RuntimeEventSubscription {
    pub replay: Vec<RuntimeEventEnvelope>,
    pub live_events: mpsc::UnboundedReceiver<RuntimeEventEnvelope>,
    pub closure: watch::Receiver<Option<RuntimeEventClosure>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEventTrimPolicy {
    pub before_sequence: Option<i64>,
    pub keep_required: bool,
}

#[async_trait]
pub trait RuntimeEventStream: Send + Sync {
    async fn open_run(&self, run_id: Uuid, policy: RuntimeEventStreamPolicy) -> anyhow::Result<()>;

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope>;

    /// Atomically claims the sole terminal event for one open runtime stream and closes it.
    /// Implementations must not compose this from independent replay, append, and close calls.
    async fn append_terminal_if_missing_and_close(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome>;

    async fn subscribe(
        &self,
        run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription>;

    async fn replay(
        &self,
        run_id: Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>>;

    async fn close_run(&self, run_id: Uuid, reason: RuntimeEventCloseReason) -> anyhow::Result<()>;

    async fn trim(&self, run_id: Uuid, policy: RuntimeEventTrimPolicy) -> anyhow::Result<()>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}
