use std::sync::Arc;

use anyhow::{anyhow, Result};
use control_plane::{
    application_public_api::workflow_schedule::{
        WorkflowScheduleTriggerService, WORKFLOW_SCHEDULE_RUN_QUEUE,
    },
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    ports::TaskQueue,
};
use serde::Deserialize;
use std::time::Duration as StdDuration;

use time::Duration;
use time::OffsetDateTime;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    routes::application_public_api::native::api_provider_runtime,
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowScheduleWorkerOutcome {
    QueueUnavailable,
    NoTask,
    Executed {
        task_id: String,
        flow_run_id: Uuid,
    },
    ExecutionFailed {
        task_id: String,
        flow_run_id: Uuid,
        error: String,
    },
    InvalidTask {
        task_id: String,
    },
}

#[derive(Debug, Deserialize)]
struct WorkflowScheduleRunTask {
    application_id: Uuid,
    flow_run_id: Uuid,
}

pub async fn consume_one_workflow_schedule_run(
    state: Arc<ApiState>,
    worker_id: &str,
    visibility_timeout: Duration,
) -> Result<WorkflowScheduleWorkerOutcome> {
    let Some(task_queue) = state.infrastructure.registered_task_queue() else {
        return Ok(WorkflowScheduleWorkerOutcome::QueueUnavailable);
    };
    let Some(task) = task_queue
        .claim(WORKFLOW_SCHEDULE_RUN_QUEUE, worker_id, visibility_timeout)
        .await?
    else {
        return Ok(WorkflowScheduleWorkerOutcome::NoTask);
    };

    let decoded = serde_json::from_value::<WorkflowScheduleRunTask>(task.payload.clone());
    let task_payload = match decoded {
        Ok(payload) => payload,
        Err(error) => {
            acknowledge_workflow_schedule_task(&task_queue, &task.task_id, worker_id).await?;
            error!(
                task_id = %task.task_id,
                error = %error,
                "acknowledged invalid workflow schedule task"
            );
            return Ok(WorkflowScheduleWorkerOutcome::InvalidTask {
                task_id: task.task_id,
            });
        }
    };

    let _execution_activity = state.runtime_activity.start(
        task_payload.application_id,
        ApplicationActivityKind::ApplicationExecution,
    );
    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let execution = scope_application_activity(
        task_payload.application_id,
        runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: task_payload.application_id,
            flow_run_id: task_payload.flow_run_id,
            provider_transport_slot: None,
        }),
    )
    .await;
    acknowledge_workflow_schedule_task(&task_queue, &task.task_id, worker_id).await?;

    match execution {
        Ok(_) => Ok(WorkflowScheduleWorkerOutcome::Executed {
            task_id: task.task_id,
            flow_run_id: task_payload.flow_run_id,
        }),
        Err(error) => {
            error!(
                application_id = %task_payload.application_id,
                flow_run_id = %task_payload.flow_run_id,
                task_id = %task.task_id,
                error = %error,
                "workflow schedule run execution failed"
            );
            Ok(WorkflowScheduleWorkerOutcome::ExecutionFailed {
                task_id: task.task_id,
                flow_run_id: task_payload.flow_run_id,
                error: error.to_string(),
            })
        }
    }
}

async fn acknowledge_workflow_schedule_task(
    task_queue: &Arc<dyn TaskQueue>,
    task_id: &str,
    worker_id: &str,
) -> Result<()> {
    if task_queue
        .ack(WORKFLOW_SCHEDULE_RUN_QUEUE, task_id, worker_id)
        .await?
    {
        Ok(())
    } else {
        Err(anyhow!("workflow schedule task acknowledgement failed"))
    }
}

const WORKFLOW_SCHEDULE_WORKER_ID: &str = "workflow-schedule-worker";
const WORKFLOW_SCHEDULE_VISIBILITY_TIMEOUT: Duration = Duration::minutes(5);
const WORKFLOW_SCHEDULE_TICK_INTERVAL: StdDuration = StdDuration::from_secs(60);
const WORKFLOW_SCHEDULE_IDLE_SLEEP: StdDuration = StdDuration::from_secs(1);

/// Wires the workflow schedule trigger into the running server: one loop
/// scans due cron schedules every minute, the other consumes enqueued
/// schedule runs. Loop failures are logged and retried on the next tick so a
/// transient error never kills the scheduler.
pub fn spawn_workflow_schedule_loops(state: Arc<ApiState>) {
    tokio::spawn(run_workflow_schedule_dispatch_loop(state.clone()));
    tokio::spawn(run_workflow_schedule_worker_loop(state));
}

async fn run_workflow_schedule_dispatch_loop(state: Arc<ApiState>) {
    let mut interval = tokio::time::interval(WORKFLOW_SCHEDULE_TICK_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        let task_queue = state.infrastructure.registered_task_queue();
        let service = WorkflowScheduleTriggerService::new(state.store.clone());
        match service
            .dispatch_due_schedules(OffsetDateTime::now_utc(), task_queue.as_deref())
            .await
        {
            Ok(entries) if entries.is_empty() => {}
            Ok(entries) => {
                for entry in entries {
                    debug!(
                        application_id = %entry.application_id,
                        outcome = ?entry.outcome,
                        "workflow schedule tick outcome"
                    );
                }
            }
            Err(dispatch_error) => {
                error!(
                    error = %dispatch_error,
                    "workflow schedule dispatch tick failed"
                );
            }
        }
    }
}

async fn run_workflow_schedule_worker_loop(state: Arc<ApiState>) {
    loop {
        match consume_one_workflow_schedule_run(
            state.clone(),
            WORKFLOW_SCHEDULE_WORKER_ID,
            WORKFLOW_SCHEDULE_VISIBILITY_TIMEOUT,
        )
        .await
        {
            Ok(
                WorkflowScheduleWorkerOutcome::NoTask
                | WorkflowScheduleWorkerOutcome::QueueUnavailable,
            ) => {
                tokio::time::sleep(WORKFLOW_SCHEDULE_IDLE_SLEEP).await;
            }
            Ok(_) => {}
            Err(worker_error) => {
                error!(
                    error = %worker_error,
                    "workflow schedule worker loop failed"
                );
                tokio::time::sleep(WORKFLOW_SCHEDULE_IDLE_SLEEP).await;
            }
        }
    }
}
