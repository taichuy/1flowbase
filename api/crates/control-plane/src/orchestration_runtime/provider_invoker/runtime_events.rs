use anyhow::Result;
use uuid::Uuid;

use crate::orchestration_runtime::runtime_event_persister::{
    is_stream_delta_event, payload_with_stream_sequence,
    persist_runtime_event_payload_for_generation,
};
use crate::ports::{
    OrchestrationRuntimeRepository, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventStream,
    RuntimeEventTerminalWriter,
};

/// The required forwarding owner must finish an observation write before it
/// returns. The live copy cannot enqueue a second write in the async persister.
pub(super) async fn forward_required_runtime_event<R: OrchestrationRuntimeRepository>(
    repository: &R,
    stream: &dyn RuntimeEventStream,
    flow_run_id: Uuid,
    mut fact: RuntimeEventPayload,
    generation_writer: Option<&dyn RuntimeEventTerminalWriter>,
) -> Result<RuntimeEventEnvelope> {
    if !fact.persist_required || is_stream_delta_event(&fact.event_type) {
        fact.persist_required = false;
        return stream.append(flow_run_id, fact).await;
    }
    let mut live = fact.clone();
    live.persist_required = false;
    let envelope = stream.append(flow_run_id, live).await?;
    fact.payload = payload_with_stream_sequence(fact.payload, envelope.sequence, envelope.sequence);
    // This writer was captured before forwarding began. The shared persister
    // marks persistence failures on that exact generation.
    persist_runtime_event_payload_for_generation(repository, flow_run_id, &fact, generation_writer)
        .await?;
    Ok(envelope)
}

#[cfg(test)]
#[path = "_tests/runtime_events.rs"]
mod tests;
