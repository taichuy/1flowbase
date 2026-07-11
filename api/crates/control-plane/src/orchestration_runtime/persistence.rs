use anyhow::{anyhow, Result};
use observability::RuntimeEventBus;
use plugin_framework::provider_contract::ProviderStreamEvent;
use serde_json::{json, Value};
use std::sync::Arc;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    capability_runtime::{host_tool_capability_id, mcp_tool_capability_id},
    ports::{
        AppendCapabilityInvocationInput, AppendContextProjectionInput,
        AppendModelFailoverAttemptLedgerInput, AppendRunEventInput, AppendUsageLedgerInput,
        CreateCallbackTaskInput, CreateCheckpointInput, CreateNodeRunInput,
        LinkUsageLedgerToModelFailoverAttemptInput, OrchestrationRuntimeRepository,
        UpdateFlowRunInput, UpdateNodeRunInput,
    },
    runtime_observability::{
        append_host_event, append_host_span, append_provider_stream_events_raw,
        coalesce_provider_stream_events,
        projection::{estimate_tokens_for_text, model_input_hash},
        AppendHostSpanInput, PROVIDER_DELTA_COALESCE_MAX_BYTES,
    },
    state_transition::{ensure_flow_run_transition, ensure_node_run_transition},
};

mod answer_presentation_persistence;
mod checkpoint_locator;
pub(super) mod model_attempts;
mod node_traces;
mod provider_observability;
#[cfg(test)]
mod tests;

use super::{
    debug_stream_events,
    llm_observability_refs::{apply_llm_debug_observability_refs, LlmDebugObservabilityRefs},
    payloads::persisted_node_output_payload,
    runtime_event_persister,
};
use answer_presentation_persistence::{
    answer_node_id, append_answer_presentation_suffix, append_ready_answer_presentation_prefix,
    final_flow_output_payload, materialize_ready_answer_node_run,
};
pub(in crate::orchestration_runtime) use checkpoint_locator::{
    checkpoint_node_id, checkpoint_snapshot_from_record, CheckpointLocatorPayload,
};
use node_traces::persist_flow_debug_node_traces;
use provider_observability::{append_provider_stream_events, persist_llm_context_observability};

pub(super) struct WaitingNodeResumeUpdate {
    pub(super) node_run_id: Uuid,
    pub(super) from_status: domain::NodeRunStatus,
    pub(super) output_payload: Value,
    pub(super) metrics_payload: Value,
    pub(super) debug_payload: Value,
}

pub(super) struct PersistFlowDebugOutcomeInput<'a> {
    pub(super) scope_id: Uuid,
    pub(super) application_name: &'a str,
    pub(super) task_queue: Option<&'a Arc<dyn crate::ports::TaskQueue>>,
    pub(super) application_id: Uuid,
    pub(super) flow_run: &'a domain::FlowRunRecord,
    pub(super) compiled_plan: Option<&'a orchestration_runtime::compiled_plan::CompiledPlan>,
    pub(super) outcome: &'a orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    pub(super) trigger_event_type: &'a str,
    pub(super) trigger_event_payload: Value,
    pub(super) base_started_at: OffsetDateTime,
    pub(super) waiting_node_resume: Option<WaitingNodeResumeUpdate>,
}

pub(super) struct PersistedFlowDebugOutcome {
    pub(super) detail: domain::ApplicationRunDetail,
    pub(super) answer_presentation_events: Vec<crate::ports::RuntimeEventPayload>,
}

pub(super) async fn persist_flow_debug_outcome<R>(
    repository: &R,
    input: PersistFlowDebugOutcomeInput<'_>,
) -> Result<PersistedFlowDebugOutcome>
where
    R: OrchestrationRuntimeRepository,
{
    let PersistFlowDebugOutcomeInput {
        scope_id,
        application_name,
        task_queue,
        application_id,
        flow_run,
        compiled_plan,
        outcome,
        trigger_event_type,
        trigger_event_payload,
        base_started_at,
        waiting_node_resume,
    } = input;
    let flow_span = append_host_span(
        repository,
        AppendHostSpanInput {
            flow_run_id: flow_run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::Flow,
            name: "debug flow".to_string(),
            started_at: base_started_at,
            metadata: json!({
                "application_id": application_id,
                "run_mode": flow_run.run_mode.as_str(),
                "trigger_event_type": trigger_event_type,
            }),
        },
    )
    .await?;
    let answer_presentation_events;
    repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: waiting_node_resume.as_ref().map(|value| value.node_run_id),
            event_type: trigger_event_type.to_string(),
            payload: trigger_event_payload,
        })
        .await?;

    if let Some(waiting_node_resume) = waiting_node_resume {
        ensure_node_run_transition(
            waiting_node_resume.from_status,
            domain::NodeRunStatus::Succeeded,
            "resume_waiting_node",
        )?;
        repository
            .update_node_run(&UpdateNodeRunInput {
                node_run_id: waiting_node_resume.node_run_id,
                status: domain::NodeRunStatus::Succeeded,
                output_payload: waiting_node_resume.output_payload,
                error_payload: None,
                metrics_payload: waiting_node_resume.metrics_payload,
                debug_payload: waiting_node_resume.debug_payload,
                finished_at: Some(OffsetDateTime::now_utc()),
            })
            .await?;
    }

    let waiting_node_run = persist_flow_debug_node_traces(
        repository,
        scope_id,
        application_name,
        task_queue,
        flow_run.id,
        Some(flow_span.id),
        outcome,
        base_started_at,
    )
    .await?;

    match &outcome.stop_reason {
        orchestration_runtime::execution_state::ExecutionStopReason::WaitingHuman(wait) => {
            let snapshot = outcome
                .checkpoint_snapshot
                .as_ref()
                .ok_or_else(|| anyhow!("waiting_human outcome is missing checkpoint"))?;
            let waiting_node_run = waiting_node_run
                .ok_or_else(|| anyhow!("waiting_human outcome is missing node run"))?;
            let answer_output_payload = materialize_ready_answer_node_run(
                repository,
                flow_run.id,
                compiled_plan,
                outcome,
                base_started_at + Duration::seconds(outcome.node_traces.len() as i64),
            )
            .await?
            .unwrap_or_else(|| json!({}));
            repository
                .create_checkpoint(&CreateCheckpointInput {
                    flow_run_id: flow_run.id,
                    node_run_id: Some(waiting_node_run.id),
                    status: "waiting_human".to_string(),
                    reason: "等待人工输入".to_string(),
                    locator_payload: CheckpointLocatorPayload::from_snapshot(
                        &wait.node_id,
                        snapshot,
                    )
                    .into_json(),
                    variable_snapshot: Value::Object(snapshot.variable_pool.clone()),
                    external_ref_payload: Some(json!({ "prompt": wait.prompt })),
                })
                .await?;
            repository
                .update_flow_run(&UpdateFlowRunInput {
                    flow_run_id: flow_run.id,
                    status: {
                        ensure_flow_run_transition(
                            flow_run.status,
                            domain::FlowRunStatus::WaitingHuman,
                            "persist_flow_waiting_human",
                        )?;
                        domain::FlowRunStatus::WaitingHuman
                    },
                    output_payload: answer_output_payload,
                    error_payload: None,
                    finished_at: None,
                })
                .await?;
            answer_presentation_events = append_ready_answer_presentation_prefix(
                repository,
                flow_run.id,
                compiled_plan,
                outcome,
            )
            .await?;
            runtime_event_persister::persist_runtime_event_payload(
                repository,
                flow_run.id,
                &debug_stream_events::waiting_human(
                    flow_run.id,
                    waiting_node_run.id,
                    &wait.node_id,
                ),
            )
            .await?;
        }
        orchestration_runtime::execution_state::ExecutionStopReason::WaitingCallback(wait) => {
            let snapshot = outcome
                .checkpoint_snapshot
                .as_ref()
                .ok_or_else(|| anyhow!("waiting_callback outcome is missing checkpoint"))?;
            let waiting_node_run = waiting_node_run
                .ok_or_else(|| anyhow!("waiting_callback outcome is missing node run"))?;
            let answer_output_payload = materialize_ready_answer_node_run(
                repository,
                flow_run.id,
                compiled_plan,
                outcome,
                base_started_at + Duration::seconds(outcome.node_traces.len() as i64),
            )
            .await?
            .unwrap_or_else(|| json!({}));
            repository
                .create_checkpoint(&CreateCheckpointInput {
                    flow_run_id: flow_run.id,
                    node_run_id: Some(waiting_node_run.id),
                    status: "waiting_callback".to_string(),
                    reason: "等待 callback 回填".to_string(),
                    locator_payload: CheckpointLocatorPayload::from_snapshot(
                        &wait.node_id,
                        snapshot,
                    )
                    .into_json(),
                    variable_snapshot: Value::Object(snapshot.variable_pool.clone()),
                    external_ref_payload: Some(wait.request_payload.clone()),
                })
                .await?;
            let callback_task = repository
                .create_callback_task(&CreateCallbackTaskInput {
                    flow_run_id: flow_run.id,
                    node_run_id: waiting_node_run.id,
                    callback_kind: wait.callback_kind.clone(),
                    request_payload: wait.request_payload.clone(),
                    external_ref_payload: Some(wait.request_payload.clone()),
                })
                .await?;
            repository
                .update_flow_run(&UpdateFlowRunInput {
                    flow_run_id: flow_run.id,
                    status: {
                        ensure_flow_run_transition(
                            flow_run.status,
                            domain::FlowRunStatus::WaitingCallback,
                            "persist_flow_waiting_callback",
                        )?;
                        domain::FlowRunStatus::WaitingCallback
                    },
                    output_payload: answer_output_payload,
                    error_payload: None,
                    finished_at: None,
                })
                .await?;
            answer_presentation_events = append_ready_answer_presentation_prefix(
                repository,
                flow_run.id,
                compiled_plan,
                outcome,
            )
            .await?;
            runtime_event_persister::persist_runtime_event_payload(
                repository,
                flow_run.id,
                &debug_stream_events::waiting_callback_with_task(
                    flow_run.id,
                    waiting_node_run.id,
                    &wait.node_id,
                    &callback_task,
                ),
            )
            .await?;
        }
        orchestration_runtime::execution_state::ExecutionStopReason::Completed => {
            ensure_flow_run_transition(
                flow_run.status,
                domain::FlowRunStatus::Succeeded,
                "persist_flow_completed",
            )?;
            let output_payload = final_flow_output_payload(outcome);
            let updated = repository
                .update_flow_run_if_status(
                    &UpdateFlowRunInput {
                        flow_run_id: flow_run.id,
                        status: domain::FlowRunStatus::Succeeded,
                        output_payload: output_payload.clone(),
                        error_payload: None,
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                    flow_run.status,
                )
                .await?;
            if updated.is_none() {
                let detail = repository
                    .get_application_run_detail(application_id, flow_run.id)
                    .await?
                    .ok_or_else(|| anyhow!("persisted flow run detail not found"))?;
                return Ok(PersistedFlowDebugOutcome {
                    detail,
                    answer_presentation_events: Vec::new(),
                });
            }
            answer_presentation_events = append_answer_presentation_suffix(
                repository,
                flow_run.id,
                answer_node_id(outcome),
                &output_payload,
            )
            .await?;
            repository
                .append_run_event(&AppendRunEventInput {
                    flow_run_id: flow_run.id,
                    node_run_id: None,
                    event_type: "flow_run_completed".to_string(),
                    payload: output_payload.clone(),
                })
                .await?;
            runtime_event_persister::persist_runtime_event_payload(
                repository,
                flow_run.id,
                &debug_stream_events::flow_finished(flow_run.id, output_payload),
            )
            .await?;
        }
        orchestration_runtime::execution_state::ExecutionStopReason::Failed(failure) => {
            ensure_flow_run_transition(
                flow_run.status,
                domain::FlowRunStatus::Failed,
                "persist_flow_failed",
            )?;
            let output_payload = final_flow_output_payload(outcome);
            let error_payload = failure.error_payload.clone();
            let updated = repository
                .update_flow_run_if_status(
                    &UpdateFlowRunInput {
                        flow_run_id: flow_run.id,
                        status: domain::FlowRunStatus::Failed,
                        output_payload: output_payload.clone(),
                        error_payload: Some(error_payload.clone()),
                        finished_at: Some(OffsetDateTime::now_utc()),
                    },
                    flow_run.status,
                )
                .await?;
            if updated.is_none() {
                let detail = repository
                    .get_application_run_detail(application_id, flow_run.id)
                    .await?
                    .ok_or_else(|| anyhow!("persisted flow run detail not found"))?;
                return Ok(PersistedFlowDebugOutcome {
                    detail,
                    answer_presentation_events: Vec::new(),
                });
            }
            answer_presentation_events = append_answer_presentation_suffix(
                repository,
                flow_run.id,
                answer_node_id(outcome),
                &output_payload,
            )
            .await?;
            repository
                .append_run_event(&AppendRunEventInput {
                    flow_run_id: flow_run.id,
                    node_run_id: None,
                    event_type: "flow_run_failed".to_string(),
                    payload: error_payload.clone(),
                })
                .await?;
            runtime_event_persister::persist_runtime_event_payload(
                repository,
                flow_run.id,
                &debug_stream_events::flow_failed(flow_run.id, error_payload),
            )
            .await?;
        }
    }

    let detail = repository
        .get_application_run_detail(application_id, flow_run.id)
        .await?
        .ok_or_else(|| anyhow!("persisted flow run detail not found"))?;
    Ok(PersistedFlowDebugOutcome {
        detail,
        answer_presentation_events,
    })
}

pub(super) async fn persist_preview_events<R>(
    repository: &R,
    flow_run: &domain::FlowRunRecord,
    node_run: &domain::NodeRunRecord,
    preview: &orchestration_runtime::preview_executor::NodePreviewOutcome,
) -> Result<Vec<domain::RunEventRecord>>
where
    R: OrchestrationRuntimeRepository,
{
    let mut events = Vec::new();
    let started = repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: Some(node_run.id),
            event_type: "node_preview_started".to_string(),
            payload: json!({
                "target_node_id": preview.target_node_id,
                "input_payload": flow_run.input_payload,
            }),
        })
        .await?;
    events.push(started);
    append_host_event(
        repository,
        flow_run.id,
        Some(node_run.id),
        None,
        "node_preview_started",
        domain::RuntimeEventLayer::Diagnostic,
        json!({
            "target_node_id": preview.target_node_id,
            "input_payload": flow_run.input_payload,
        }),
    )
    .await?;
    events.extend(
        append_provider_stream_events(
            repository,
            flow_run.id,
            Some(node_run.id),
            None,
            &preview.provider_events,
        )
        .await?,
    );
    let completed = repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: Some(node_run.id),
            event_type: if preview.is_failed() {
                "node_preview_failed".to_string()
            } else {
                "node_preview_completed".to_string()
            },
            payload: preview.as_payload(),
        })
        .await?;
    events.push(completed);
    append_host_event(
        repository,
        flow_run.id,
        Some(node_run.id),
        None,
        if preview.is_failed() {
            "node_preview_failed"
        } else {
            "node_preview_completed"
        },
        domain::RuntimeEventLayer::Diagnostic,
        preview.as_payload(),
    )
    .await?;

    Ok(events)
}

pub(super) fn next_node_started_at(detail: &domain::ApplicationRunDetail) -> OffsetDateTime {
    detail
        .node_runs
        .iter()
        .map(|record| record.started_at)
        .max()
        .map(|value| value + Duration::seconds(1))
        .unwrap_or_else(OffsetDateTime::now_utc)
}
