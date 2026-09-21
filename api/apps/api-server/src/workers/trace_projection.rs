//! Durable, coalesced trace projection updates. Log reads never run this work.
use crate::{
    app_state::ApiState,
    routes::applications::application_runtime::enrich_application_run_detail_visible_internal_llm_route_traces,
};
use control_plane::{
    orchestration_runtime::trace_projection::{
        build_application_run_trace_projection, APPLICATION_RUN_TRACE_PROJECTION_VERSION,
    },
    ports::{OrchestrationRuntimeRepository, UpsertApplicationRunTraceProjectionStatusInput},
    system_recovery::SystemWriteOwner,
};
use std::{sync::Arc, time::Duration};

pub fn spawn_trace_projection_worker(state: Arc<ApiState>) {
    tokio::spawn(async move {
        let shutdown = crate::shutdown_signal();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _=&mut shutdown => return,
                _=tokio::time::sleep(Duration::from_millis(250)) => {}
            }
            if let Err(error) = refresh_next_trace_projection(&state).await {
                tracing::warn!(%error, "trace projection refresh failed; durable queue will retry");
            }
        }
    });
}

pub(crate) async fn refresh_next_trace_projection(state: &ApiState) -> anyhow::Result<bool> {
    let Ok(_permit) = state
        .system_maintenance
        .try_enter_write(SystemWriteOwner::TraceProjectionPersistence)
    else {
        return Ok(false);
    };
    let Some(job) = state.store.claim_application_run_trace_refresh().await? else {
        return Ok(false);
    };
    let result: anyhow::Result<()> = async {
        let source = state
            .store
            .get_application_run_trace_projection_source(job.application_id, job.flow_run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("trace projection source disappeared"))?;
        let events = state
            .store
            .list_trace_enrichment_events(job.flow_run_id)
            .await?;
        let source =
            enrich_application_run_detail_visible_internal_llm_route_traces(source, &events);
        let projection = build_application_run_trace_projection(&source)?;
        state
            .store
            .replace_application_run_trace_projection(&projection)
            .await?;
        Ok(())
    }
    .await;
    if let Err(error) = &result {
        let now = time::OffsetDateTime::now_utc();
        state
            .store
            .upsert_application_run_trace_projection_status(
                &UpsertApplicationRunTraceProjectionStatusInput {
                    flow_run_id: job.flow_run_id,
                    projection_version: APPLICATION_RUN_TRACE_PROJECTION_VERSION,
                    status: domain::ApplicationRunTraceProjectionStatus::Failed,
                    source_watermark: String::new(),
                    attempt_count: 1,
                    last_attempt_at: Some(now),
                    last_success_at: None,
                    diagnostic: Some(domain::ApplicationRunTraceProjectionDiagnostic {
                        last_error_code: Some("trace_projection_refresh_failed".into()),
                        last_error_stage: Some("projection_worker".into()),
                        last_error_source_kind: None,
                        last_error_source_locator: None,
                        last_error_message: Some(error.to_string()),
                        last_error_ref: None,
                        retriable: true,
                    }),
                },
            )
            .await?;
    }
    state
        .store
        .finish_application_run_trace_refresh(&job, result.is_ok())
        .await?;
    result?;
    Ok(true)
}
