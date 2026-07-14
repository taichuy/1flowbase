use control_plane::application_public_api::workflow_schedule::WORKFLOW_SCHEDULE_RUN_QUEUE;
use serde_json::json;
use time::Duration;

use crate::{
    _tests::support::test_api_state_with_database_url,
    workers::workflow_schedule::{
        WorkflowScheduleWorkerOutcome, consume_one_workflow_schedule_run,
    },
};

#[tokio::test]
async fn workflow_schedule_worker_acks_invalid_tasks_without_retrying() {
    let (state, _) = test_api_state_with_database_url().await;
    let task_queue = state.infrastructure.task_queue();
    let task_id = task_queue
        .enqueue(
            WORKFLOW_SCHEDULE_RUN_QUEUE,
            json!({ "application_id": "not-a-uuid" }),
            None,
        )
        .await
        .unwrap();

    let outcome = consume_one_workflow_schedule_run(
        state.clone(),
        "schedule-worker-test",
        Duration::seconds(30),
    )
    .await
    .unwrap();
    let entries = task_queue.list_ephemeral_entries().await.unwrap();

    assert_eq!(
        outcome,
        WorkflowScheduleWorkerOutcome::InvalidTask {
            task_id: task_id.clone()
        }
    );
    assert!(
        entries.iter().all(|entry| entry.key != task_id),
        "invalid schedule task should be acknowledged instead of retried"
    );
}
