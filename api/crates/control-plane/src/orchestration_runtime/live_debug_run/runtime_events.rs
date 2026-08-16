use uuid::Uuid;

use crate::ports::{OrchestrationRuntimeRepository, RuntimeEventDurability};

use super::super::{
    is_expected_runtime_event_stream_closed_error, runtime_event_persister,
    OrchestrationRuntimeService,
};

pub(in crate::orchestration_runtime) async fn project_committed_terminal<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run: &domain::FlowRunRecord,
) where
    R: OrchestrationRuntimeRepository,
{
    if let Some(stream) = &service.runtime_event_stream {
        runtime_event_persister::project_runtime_event_stream_terminal(stream.clone(), flow_run)
            .await;
    }
    service.clear_provider_protocol_contexts(flow_run.id).await;
}

pub(super) async fn append_runtime_event<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: Uuid,
    mut event: crate::ports::RuntimeEventPayload,
) where
    R: OrchestrationRuntimeRepository,
{
    if let Some(stream) = &service.runtime_event_stream {
        let mut durable_event = event.clone();
        event.persist_required = false;
        event.durability = RuntimeEventDurability::Ephemeral;
        let event_type = event.event_type.clone();
        let source = event.source;
        match stream.append(flow_run_id, event).await {
            Ok(envelope) => {
                durable_event.payload = runtime_event_persister::payload_with_stream_sequence(
                    durable_event.payload,
                    envelope.sequence,
                    envelope.sequence,
                );
                if let Err(error) = runtime_event_persister::persist_runtime_event_payload(
                    &service.repository,
                    flow_run_id,
                    &durable_event,
                )
                .await
                {
                    tracing::warn!(
                        flow_run_id = %flow_run_id,
                        event_type = %event_type,
                        source = ?source,
                        error = %error,
                        "failed to persist sequenced runtime event"
                    );
                }
            }
            Err(error) => {
                if is_expected_runtime_event_stream_closed_error(&error) {
                    tracing::debug!(
                        flow_run_id = %flow_run_id,
                        event_type = %event_type,
                        source = ?source,
                        error = %error,
                        "runtime event append skipped because stream is already closed"
                    );
                } else {
                    tracing::warn!(
                        flow_run_id = %flow_run_id,
                        event_type = %event_type,
                        source = ?source,
                        error = %error,
                        "failed to append runtime event"
                    );
                }
            }
        }
        return;
    }

    if let Err(error) = runtime_event_persister::persist_runtime_event_payload(
        &service.repository,
        flow_run_id,
        &event,
    )
    .await
    {
        tracing::warn!(
            flow_run_id = %flow_run_id,
            event_type = %event.event_type,
            source = ?event.source,
            error = %error,
            "failed to persist runtime event"
        );
    }
}
