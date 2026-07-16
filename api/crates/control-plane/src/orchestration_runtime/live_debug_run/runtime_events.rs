use serde_json::Value;
use uuid::Uuid;

use crate::ports::{OrchestrationRuntimeRepository, RuntimeEventCloseReason};

use super::super::{
    debug_stream_events, is_expected_runtime_event_stream_closed_error, runtime_event_persister,
    OrchestrationRuntimeService,
};

pub(super) async fn append_runtime_event<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: Uuid,
    event: crate::ports::RuntimeEventPayload,
) where
    R: OrchestrationRuntimeRepository,
{
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
    if let Some(stream) = &service.runtime_event_stream {
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

pub(super) async fn close_runtime_event_stream<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: Uuid,
    reason: RuntimeEventCloseReason,
) {
    if let Some(stream) = &service.runtime_event_stream {
        if let Err(error) = stream.close_run(flow_run_id, reason).await {
            if is_expected_runtime_event_stream_closed_error(&error) {
                tracing::debug!(
                    flow_run_id = %flow_run_id,
                    reason = ?reason,
                    error = %error,
                    "runtime event stream close skipped because stream is not open"
                );
            } else {
                tracing::warn!(
                    flow_run_id = %flow_run_id,
                    reason = ?reason,
                    error = %error,
                    "failed to close runtime event stream"
                );
            }
        }
    }
}

pub(super) async fn emit_flow_failed_and_close<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: Uuid,
    error_payload: Value,
) where
    R: OrchestrationRuntimeRepository,
{
    emit_flow_failed_and_close_with_reason(
        service,
        flow_run_id,
        error_payload,
        RuntimeEventCloseReason::Failed,
    )
    .await;
}

async fn emit_flow_failed_and_close_with_reason<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: Uuid,
    error_payload: Value,
    reason: RuntimeEventCloseReason,
) where
    R: OrchestrationRuntimeRepository,
{
    append_runtime_event(
        service,
        flow_run_id,
        debug_stream_events::flow_failed(flow_run_id, error_payload),
    )
    .await;
    close_runtime_event_stream(service, flow_run_id, reason).await;
}
