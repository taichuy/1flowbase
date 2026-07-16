use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::{AppendRunEventInput, OrchestrationRuntimeRepository, UpdateFlowRunInput},
    state_transition::ensure_flow_run_transition,
};

use super::super::OrchestrationRuntimeService;
use super::emit_flow_failed_and_close;

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
    service
        .repository
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id,
            status: domain::FlowRunStatus::Failed,
            output_payload: flow_run.output_payload,
            error_payload: Some(error_payload.clone()),
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await?;
    emit_flow_failed_and_close(service, flow_run_id, error_payload.clone()).await;
    service
        .repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id,
            node_run_id: None,
            event_type: "flow_run_failed".to_string(),
            payload: error_payload,
        })
        .await?;

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
