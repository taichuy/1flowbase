use uuid::Uuid;

use crate::ports::{OrchestrationRuntimeRepository, RuntimeEventDurability};

use super::super::{
    is_expected_runtime_event_stream_closed_error, runtime_event_persister,
    OrchestrationRuntimeService,
};

pub(super) async fn project_committed_terminal<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run: &domain::FlowRunRecord,
) where
    R: OrchestrationRuntimeRepository,
{
    let Some(stream) = &service.runtime_event_stream else {
        return;
    };
    runtime_event_persister::project_runtime_event_stream_terminal(stream.clone(), flow_run).await;
}

pub(super) async fn append_runtime_event<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: Uuid,
    mut event: crate::ports::RuntimeEventPayload,
) where
    R: OrchestrationRuntimeRepository,
{
    let already_persisted = match runtime_event_persister::persist_runtime_event_payload(
        &service.repository,
        flow_run_id,
        &event,
    )
    .await
    {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(
                flow_run_id = %flow_run_id,
                event_type = %event.event_type,
                source = ?event.source,
                error = %error,
                "failed to persist runtime event"
            );
            false
        }
    };
    if let Some(stream) = &service.runtime_event_stream {
        if already_persisted {
            event.persist_required = false;
            event.durability = RuntimeEventDurability::Ephemeral;
        }
        let event_type = event.event_type.clone();
        let source = event.source;
        if let Err(error) = stream.append(flow_run_id, event).await {
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
}
