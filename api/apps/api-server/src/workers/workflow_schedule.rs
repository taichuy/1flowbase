use std::sync::Arc;

use anyhow::{anyhow, Result};
use control_plane::{
    application_public_api::workflow_schedule::WORKFLOW_SCHEDULE_RUN_QUEUE,
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    ports::TaskQueue,
};
use serde::Deserialize;
use time::Duration;
use tracing::error;
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
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let execution = scope_application_activity(
        task_payload.application_id,
        runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: task_payload.application_id,
            flow_run_id: task_payload.flow_run_id,
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
