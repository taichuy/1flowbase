use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::{
        CommitFlowRunTerminalInput, CommitFlowRunTerminalResult, OrchestrationRuntimeRepository,
    },
    state_transition::ensure_flow_run_transition,
};

use super::super::{
    debug_stream_events,
    stream_terminal_recovery::{resolve_terminal_commit, TerminalCommitResolution},
    OrchestrationRuntimeService,
};
use super::project_committed_terminal;

pub(super) async fn load_run_detail<R>(
    repository: &R,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<domain::ApplicationRunDetail>
where
    R: OrchestrationRuntimeRepository,
{
    repository
        .get_application_run_detail(application_id, flow_run_id)
        .await?
        .ok_or_else(|| anyhow!("flow run detail not found"))
}

pub(super) async fn fail_flow_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    application_id: Uuid,
    flow_run_id: Uuid,
    error: &anyhow::Error,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    let Some(flow_run) = service
        .repository
        .get_flow_run(application_id, flow_run_id)
        .await?
    else {
        return Err(anyhow!("flow run not found"));
    };
    if matches!(
        flow_run.status,
        domain::FlowRunStatus::Cancelled
            | domain::FlowRunStatus::Succeeded
            | domain::FlowRunStatus::Incomplete
            | domain::FlowRunStatus::Failed
    ) {
        return load_run_detail(&service.repository, application_id, flow_run_id).await;
    }
    ensure_flow_run_transition(
        flow_run.status,
        domain::FlowRunStatus::Failed,
        "fail_flow_run",
    )?;
    let error_payload = serde_error_payload(error);
    let terminal_event = debug_stream_events::flow_failed(flow_run_id, error_payload.clone());
    let receipt = service
        .repository
        .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
            flow_run_id: flow_run.id,
            expected_status: flow_run.status,
            result: CommitFlowRunTerminalResult::Failed {
                output_payload: flow_run.output_payload,
                error_payload: error_payload.clone(),
            },
            flow_run_event_payload: error_payload,
            terminal_event_payload: terminal_event.payload,
            finished_at: OffsetDateTime::now_utc(),
        })
        .await?;
    let flow_run =
        match resolve_terminal_commit(&service.repository, application_id, flow_run_id, receipt)
            .await?
        {
            TerminalCommitResolution::Winner(flow_run)
            | TerminalCommitResolution::Loser(flow_run) => flow_run,
        };
    project_committed_terminal(service, &flow_run).await;

    load_run_detail(&service.repository, application_id, flow_run_id).await
}

fn serde_error_payload(error: &anyhow::Error) -> Value {
    let text = error.to_string();
    let Ok(payload) = serde_json::from_str::<Value>(&text) else {
        return json!({ "message": text });
    };

    if !payload.is_object() {
        return json!({ "message": text });
    }

    let Some(message) = payload.get("message") else {
        return payload;
    };

    if message.is_null() {
        return payload;
    }

    payload
}
