use super::*;
use control_plane::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventCloseReason, RuntimeEventClosure,
    RuntimeEventReceiver, RuntimeEventSubscription, RuntimeEventTerminalWriter,
};

const DURABLE_REPLAY_PAGE_SIZE: usize = 64;

/// A cold attachment is a read of existing facts, never a producer authority.
struct ColdTerminalWriter;
#[async_trait::async_trait]
impl RuntimeEventTerminalWriter for ColdTerminalWriter {
    async fn append_terminal_if_missing_and_close(
        &self,
        _event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("cold replay cannot publish or execute a terminal")
    }
}

pub(crate) async fn cold_runtime_event_subscription(
    dependencies: &NativeRunTerminalDependencies,
    initial_run: &NativeRunResult,
    from_sequence: Option<i64>,
) -> anyhow::Result<RuntimeEventSubscription> {
    let window = dependencies
        .replay_window(initial_run.id)
        .await?
        .filter(|window| window.generation_id.is_some());
    let (mut replay, reason) = if let Some(window) = window {
        let mut records = Vec::new();
        let mut terminal_reason = None;
        let mut cursor = window.after_sequence;
        while cursor < window.through_sequence {
            let page = dependencies
                .durable_page(
                    initial_run.id,
                    cursor,
                    window.through_sequence,
                    DURABLE_REPLAY_PAGE_SIZE,
                )
                .await?;
            let Some(last) = page.last() else {
                break;
            };
            if last.sequence <= cursor {
                anyhow::bail!("durable replay cursor did not advance");
            }
            cursor = last.sequence;
            for record in page {
                if record.event_type == "runtime_stream_opened" {
                    continue;
                }
                if let Some(generation) = window.generation_id {
                    if record.payload["stream_generation_id"]
                        .as_str()
                        .is_some_and(|value| value != generation.to_string())
                    {
                        continue;
                    }
                }
                // Generation membership is checked before terminal evidence.
                // The client cursor controls payload replay, not admission or
                // the terminal meaning of this generation's closed subscription.
                terminal_reason =
                    RuntimeEventCloseReason::from_terminal_event_type(&record.event_type);
                let local = compat_payload_i64(&record.payload, "sequence_end")
                    .or_else(|| compat_payload_i64(&record.payload, "stream_sequence"));
                if terminal_reason.is_none()
                    && local.is_some_and(|sequence| sequence <= from_sequence.unwrap_or(0))
                {
                    continue;
                }
                records.push(record);
                if terminal_reason.is_some() {
                    break;
                }
            }
            if terminal_reason.is_some() {
                break;
            }
        }
        let reason = match terminal_reason {
            Some(reason) => reason,
            None => {
                // Admission may have reopened the same run between the failed
                // original subscribe and this read. Attach only to the selected
                // existing generation; this read never opens a producer.
                if let Ok(subscription) = dependencies
                    .runtime_event_stream
                    .subscribe(initial_run.id, from_sequence)
                    .await
                {
                    if subscription.terminal_writer.generation_id() == window.generation_id {
                        return Ok(subscription);
                    }
                }
                anyhow::bail!("anchored runtime stream has no recoverable durable terminal");
            }
        };
        // Only the evidenced terminal closes an anchored generation. A mutable
        // run snapshot may still describe the preceding waiting callback round.
        (durable_round_prefix(records), reason)
    } else {
        // Explicit legacy policy: unanchored runs recover their terminal from
        // durable business state, without assigning it to a stream generation.
        let run = load_durable_native_run_for_terminal_projection_with_dependencies(
            dependencies,
            initial_run,
        )
        .await?;
        let terminal = terminal_runtime_event_from_native_run(&run).ok_or_else(|| {
            anyhow::anyhow!("active runtime stream has no recoverable durable terminal")
        })?;
        let reason = RuntimeEventCloseReason::from_terminal_event_type(&terminal.event_type)
            .ok_or_else(|| anyhow::anyhow!("durable runtime terminal has no close reason"))?;
        (vec![terminal], reason)
    };
    // This is one semantic cold projection. Its ordering cursor starts after
    // the client's cursor; durable and generation-local sequences stay separate.
    for (index, event) in replay.iter_mut().enumerate() {
        event.sequence = from_sequence.unwrap_or(0).saturating_add(index as i64 + 1);
        event.event_id = format!("{}:{}", initial_run.id, event.sequence);
    }
    let final_sequence = replay.last().map_or(0, |event| event.sequence);
    let (required, diagnostic, live_events) = RuntimeEventReceiver::bounded_lanes(1);
    drop((required, diagnostic));
    let (_closed, closure) = tokio::sync::watch::channel(Some(RuntimeEventClosure {
        reason,
        final_sequence,
    }));
    Ok(RuntimeEventSubscription {
        terminal_writer: Arc::new(ColdTerminalWriter),
        replay,
        live_events,
        closure,
    })
}

pub(crate) fn durable_round_prefix(
    records: Vec<domain::RuntimeEventRecord>,
) -> Vec<RuntimeEventEnvelope> {
    let mut prefix = Vec::new();
    for record in records {
        let event = durable_record_to_runtime_event_envelope(record);
        let terminal =
            RuntimeEventCloseReason::from_terminal_event_type(&event.event_type).is_some();
        prefix.push(event);
        if terminal {
            break;
        }
    }
    let first_item = prefix
        .iter()
        .position(|event| event.event_type == "provider_output_item_done");
    if let Some(first_item) = first_item {
        let mut output_items = Vec::new();
        let mut remaining = Vec::with_capacity(prefix.len());
        for event in prefix {
            if event.event_type == "provider_output_item_done" {
                output_items.push(event);
            } else {
                remaining.push(event);
            }
        }
        output_items.sort_by_key(|event| {
            (
                event.payload["output_index"].as_u64().unwrap_or(u64::MAX),
                event.sequence,
            )
        });
        remaining.splice(first_item..first_item, output_items);
        prefix = remaining;
    }
    // The projector cursor orders this one replay batch. Database sequence
    // reflects separate provider and callback writers, while output_index is
    // the provider's exact order for completed items.
    for (index, event) in prefix.iter_mut().enumerate() {
        event.sequence = index as i64 + 1;
    }
    prefix
}

pub(crate) fn durable_record_to_runtime_event_envelope(
    record: domain::RuntimeEventRecord,
) -> RuntimeEventEnvelope {
    let text = compat_payload_string(&record.payload, "text")
        .or_else(|| compat_payload_string(&record.payload, "delta"));
    let delta_index = compat_payload_i64(&record.payload, "delta_index")
        .or_else(|| compat_payload_i64(&record.payload, "sequence_start"));
    let content_type = compat_payload_string(&record.payload, "content_type");
    RuntimeEventEnvelope {
        run_id: record.flow_run_id,
        node_run_id: record.node_run_id,
        sequence: record.sequence,
        event_id: format!("{}:{}", record.flow_run_id, record.sequence),
        event_type: record.event_type,
        occurred_at: record.created_at,
        delta_index,
        content_type,
        text,
        source: match record.source {
            domain::RuntimeEventSource::ProviderPlugin => {
                control_plane::ports::RuntimeEventSource::Provider
            }
            _ => control_plane::ports::RuntimeEventSource::Runtime,
        },
        durability: match record.durability {
            domain::RuntimeEventDurability::Durable => {
                control_plane::ports::RuntimeEventDurability::DurableRequired
            }
            domain::RuntimeEventDurability::Ephemeral | domain::RuntimeEventDurability::Sampled => {
                control_plane::ports::RuntimeEventDurability::Ephemeral
            }
        },
        persist_required: true,
        trace_visible: true,
        payload: record.payload,
    }
}

fn compat_payload_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}
fn compat_payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}
