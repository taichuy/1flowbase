use anyhow::{bail, Result};
use observability::{DeltaCoalescer, RuntimeBusEvent, RuntimeEventBus};
use plugin_framework::provider_contract::ProviderStreamEvent;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::{
    AppendRunEventInput, AppendRuntimeEventInput, AppendRuntimeItemInput, AppendRuntimeSpanInput,
    OrchestrationRuntimeRepository,
};

pub mod debug_read_model;
pub mod items;
pub mod projection;

pub use items::item_kind_for_event;

pub const PROVIDER_DELTA_COALESCE_MAX_BYTES: usize = 4096;
pub const PROVIDER_DELTA_COALESCE_MAX_DELAY_MS: u64 = 250;

pub struct AppendHostSpanInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub kind: domain::RuntimeSpanKind,
    pub name: String,
    pub started_at: OffsetDateTime,
    pub metadata: Value,
}

pub fn audit_row_hash(
    prev_hash: Option<&str>,
    fact_table: &str,
    fact_id: Uuid,
    payload: &serde_json::Value,
) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    if let Some(prev) = prev_hash {
        hasher.update(prev.as_bytes());
    }
    hasher.update(fact_table.as_bytes());
    hasher.update(fact_id.as_bytes());
    hasher.update(serde_json::to_vec(payload).unwrap_or_default());
    format!("sha256:{:x}", hasher.finalize())
}

pub async fn append_host_span<R>(
    repository: &R,
    input: AppendHostSpanInput,
) -> Result<domain::RuntimeSpanRecord>
where
    R: OrchestrationRuntimeRepository,
{
    let AppendHostSpanInput {
        flow_run_id,
        node_run_id,
        parent_span_id,
        kind,
        name,
        started_at,
        metadata,
    } = input;

    repository
        .append_runtime_span(&AppendRuntimeSpanInput {
            flow_run_id,
            node_run_id,
            parent_span_id,
            kind,
            name,
            status: domain::RuntimeSpanStatus::Running,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata,
            started_at,
            finished_at: None,
        })
        .await
}

pub async fn append_host_event<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    event_type: impl Into<String>,
    layer: domain::RuntimeEventLayer,
    payload: Value,
) -> Result<domain::RuntimeEventRecord>
where
    R: OrchestrationRuntimeRepository,
{
    let event_type = event_type.into();
    let event = repository
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id,
            node_run_id,
            span_id,
            parent_span_id: None,
            event_type: event_type.clone(),
            layer,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await?;

    if let Some(kind) = item_kind_for_event(&event_type) {
        repository
            .append_runtime_item(&AppendRuntimeItemInput {
                flow_run_id,
                span_id,
                kind,
                status: domain::RuntimeItemStatus::Created,
                source_event_id: Some(event.id),
                input_ref: None,
                output_ref: None,
                usage_ledger_id: None,
                trust_level: domain::RuntimeTrustLevel::HostFact,
            })
            .await?;
    }

    Ok(event)
}

pub async fn mark_external_opaque_boundary<R>(
    repository: &R,
    flow_run_id: Uuid,
    payload: Value,
) -> Result<domain::RuntimeEventRecord>
where
    R: OrchestrationRuntimeRepository,
{
    repository
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            event_type: "external_agent_opaque_boundary_marked".into(),
            layer: domain::RuntimeEventLayer::Diagnostic,
            source: domain::RuntimeEventSource::ExternalAgent,
            trust_level: domain::RuntimeTrustLevel::ExternalOpaque,
            item_id: None,
            ledger_ref: None,
            payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await
}

pub fn coalesce_provider_stream_events(
    bus: &RuntimeEventBus,
    events: &[ProviderStreamEvent],
    max_bytes: usize,
) -> Result<Vec<ProviderStreamEvent>> {
    let mut coalescer = DeltaCoalescer::new(max_bytes);
    let mut coalesced = Vec::with_capacity(events.len());

    for event in events {
        match event {
            ProviderStreamEvent::NativeEvent { .. } => {
                flush_text_delta(bus, &mut coalesced, &mut coalescer);
                flush_reasoning_delta(bus, &mut coalesced, &mut coalescer);
            }
            ProviderStreamEvent::TextDelta { delta } => {
                flush_reasoning_delta(bus, &mut coalesced, &mut coalescer);
                push_runtime_bus_delta(bus, &mut coalesced, coalescer.push_text(delta));
            }
            ProviderStreamEvent::ReasoningDelta { delta } => {
                flush_text_delta(bus, &mut coalesced, &mut coalescer);
                push_runtime_bus_delta(bus, &mut coalesced, coalescer.push_reasoning(delta));
            }
            other => {
                flush_text_delta(bus, &mut coalesced, &mut coalescer);
                flush_reasoning_delta(bus, &mut coalesced, &mut coalescer);
                bus.publish(RuntimeBusEvent::RuntimeEvent {
                    event_type: provider_stream_event_type(other).to_string(),
                    payload: serde_json::to_value(other)?,
                });
                coalesced.push(other.clone());
            }
        }
    }

    flush_text_delta(bus, &mut coalesced, &mut coalescer);
    flush_reasoning_delta(bus, &mut coalesced, &mut coalescer);

    Ok(coalesced)
}

pub async fn append_provider_stream_event<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    event: &ProviderStreamEvent,
) -> Result<domain::RunEventRecord>
where
    R: OrchestrationRuntimeRepository,
{
    if matches!(event, ProviderStreamEvent::NativeEvent { .. }) {
        bail!("ephemeral provider native events cannot be persisted");
    }
    let event_type = provider_stream_event_type(event);
    let payload = serde_json::to_value(event)?;
    let record = repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id,
            node_run_id,
            event_type: event_type.to_string(),
            payload: payload.clone(),
        })
        .await?;
    append_host_event(
        repository,
        flow_run_id,
        node_run_id,
        span_id,
        event_type,
        domain::RuntimeEventLayer::ProviderRaw,
        payload,
    )
    .await?;

    Ok(record)
}

pub async fn append_provider_stream_events_raw<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    events: &[ProviderStreamEvent],
) -> Result<Vec<domain::RunEventRecord>>
where
    R: OrchestrationRuntimeRepository,
{
    append_provider_stream_events_raw_filtered(
        repository,
        flow_run_id,
        node_run_id,
        span_id,
        events,
        |_| true,
    )
    .await
}

pub async fn append_provider_stream_events_raw_filtered<R, F>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    events: &[ProviderStreamEvent],
    should_append_run_event: F,
) -> Result<Vec<domain::RunEventRecord>>
where
    R: OrchestrationRuntimeRepository,
    F: Fn(&ProviderStreamEvent) -> bool,
{
    if events.is_empty() {
        return Ok(Vec::new());
    }

    let mut run_inputs = Vec::with_capacity(events.len());
    let mut runtime_inputs = Vec::with_capacity(events.len());

    for event in events {
        if matches!(event, ProviderStreamEvent::NativeEvent { .. }) {
            continue;
        }
        let event_type = provider_stream_event_type(event);
        let payload = serde_json::to_value(event)?;
        if should_append_run_event(event) {
            run_inputs.push(AppendRunEventInput {
                flow_run_id,
                node_run_id,
                event_type: event_type.to_string(),
                payload: payload.clone(),
            });
        }
        runtime_inputs.push(AppendRuntimeEventInput {
            flow_run_id,
            node_run_id,
            span_id,
            parent_span_id: None,
            event_type: event_type.to_string(),
            layer: domain::RuntimeEventLayer::ProviderRaw,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        });
    }

    let records = if run_inputs.is_empty() {
        Vec::new()
    } else {
        repository.append_run_events(&run_inputs).await?
    };
    let runtime_records = repository.append_runtime_events(&runtime_inputs).await?;
    for runtime_record in runtime_records {
        if let Some(kind) = item_kind_for_event(&runtime_record.event_type) {
            repository
                .append_runtime_item(&AppendRuntimeItemInput {
                    flow_run_id,
                    span_id,
                    kind,
                    status: domain::RuntimeItemStatus::Created,
                    source_event_id: Some(runtime_record.id),
                    input_ref: None,
                    output_ref: None,
                    usage_ledger_id: None,
                    trust_level: domain::RuntimeTrustLevel::HostFact,
                })
                .await?;
        }
    }

    Ok(records)
}

pub fn provider_stream_event_type(event: &ProviderStreamEvent) -> &'static str {
    match event {
        ProviderStreamEvent::NativeEvent { .. } => "native_event",
        ProviderStreamEvent::TextDelta { .. } => "text_delta",
        ProviderStreamEvent::ReasoningDelta { .. } => "reasoning_delta",
        ProviderStreamEvent::ToolCallDelta { .. } => "tool_call_delta",
        ProviderStreamEvent::ToolCallCommit { .. } => "tool_call_commit",
        ProviderStreamEvent::McpCallDelta { .. } => "mcp_call_delta",
        ProviderStreamEvent::McpCallCommit { .. } => "mcp_call_commit",
        ProviderStreamEvent::UsageDelta { .. } => "usage_delta",
        ProviderStreamEvent::UsageSnapshot { .. } => "usage_snapshot",
        ProviderStreamEvent::Finish { .. } => "finish",
        ProviderStreamEvent::Error { .. } => "error",
    }
}

fn flush_text_delta(
    bus: &RuntimeEventBus,
    coalesced: &mut Vec<ProviderStreamEvent>,
    coalescer: &mut DeltaCoalescer,
) {
    push_runtime_bus_delta(bus, coalesced, coalescer.flush_text());
}

fn flush_reasoning_delta(
    bus: &RuntimeEventBus,
    coalesced: &mut Vec<ProviderStreamEvent>,
    coalescer: &mut DeltaCoalescer,
) {
    push_runtime_bus_delta(bus, coalesced, coalescer.flush_reasoning());
}

fn push_runtime_bus_delta(
    bus: &RuntimeEventBus,
    coalesced: &mut Vec<ProviderStreamEvent>,
    event: Option<RuntimeBusEvent>,
) {
    let Some(event) = event else {
        return;
    };

    bus.publish(event.clone());
    match event {
        RuntimeBusEvent::TextDelta { delta } => {
            coalesced.push(ProviderStreamEvent::TextDelta { delta });
        }
        RuntimeBusEvent::ReasoningDelta { delta } => {
            coalesced.push(ProviderStreamEvent::ReasoningDelta { delta });
        }
        _ => {}
    }
}

/// Streaming coalescer that merges consecutive text/reasoning deltas as events arrive.
pub struct LiveEventCoalescer {
    coalescer: DeltaCoalescer,
}

impl LiveEventCoalescer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            coalescer: DeltaCoalescer::new(max_bytes),
        }
    }

    /// Push a single event through the coalescer. Returns zero or more coalesced events
    /// that are ready to be persisted.
    pub fn push(&mut self, event: ProviderStreamEvent) -> Vec<ProviderStreamEvent> {
        let mut ready = Vec::new();
        match &event {
            ProviderStreamEvent::TextDelta { delta } => {
                self.flush_reasoning(&mut ready);
                if let Some(RuntimeBusEvent::TextDelta { delta: d }) =
                    self.coalescer.push_text(delta)
                {
                    ready.push(ProviderStreamEvent::TextDelta { delta: d });
                }
            }
            ProviderStreamEvent::ReasoningDelta { delta } => {
                self.flush_text(&mut ready);
                if let Some(RuntimeBusEvent::ReasoningDelta { delta: d }) =
                    self.coalescer.push_reasoning(delta)
                {
                    ready.push(ProviderStreamEvent::ReasoningDelta { delta: d });
                }
            }
            other => {
                self.flush_text(&mut ready);
                self.flush_reasoning(&mut ready);
                ready.push(other.clone());
            }
        }
        ready
    }

    /// Drain remaining coalesced deltas. Call this when the event stream ends.
    pub fn finish(&mut self) -> Vec<ProviderStreamEvent> {
        self.flush_buffered()
    }

    /// Drain currently buffered deltas without ending the stream.
    pub fn flush_buffered(&mut self) -> Vec<ProviderStreamEvent> {
        let mut ready = Vec::new();
        self.flush_text(&mut ready);
        self.flush_reasoning(&mut ready);
        ready
    }

    fn flush_text(&mut self, ready: &mut Vec<ProviderStreamEvent>) {
        if let Some(RuntimeBusEvent::TextDelta { delta }) = self.coalescer.flush_text() {
            ready.push(ProviderStreamEvent::TextDelta { delta });
        }
    }

    fn flush_reasoning(&mut self, ready: &mut Vec<ProviderStreamEvent>) {
        if let Some(RuntimeBusEvent::ReasoningDelta { delta }) = self.coalescer.flush_reasoning() {
            ready.push(ProviderStreamEvent::ReasoningDelta { delta });
        }
    }
}
