use control_plane::ports::{
    ListModelProviderRequestLogsPageInput, OrchestrationRuntimeRepository, ProviderRequestLogTask,
    PROVIDER_REQUEST_LOG_QUEUE,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    _tests::support::test_api_state_with_database_url,
    workers::provider_request_logs::{
        consume_provider_request_log_batch, ProviderRequestLogWorkerOutcome,
    },
};

fn request_log_task(scope_id: Uuid, attempt_id: Uuid) -> ProviderRequestLogTask {
    let started_at = OffsetDateTime::now_utc();
    ProviderRequestLogTask {
        scope_id,
        attempt_id,
        flow_run_id: Uuid::now_v7(),
        application_name: "Worker App Snapshot".into(),
        attempt_index: 0,
        provider_instance_id: Some(Uuid::now_v7()),
        provider_instance_display_name: Some("Worker Provider Snapshot".into()),
        provider_code: "worker_fixture".into(),
        protocol: "openai_compatible".into(),
        upstream_model_id: "worker-model".into(),
        reasoning_effort: Some("high".into()),
        status: "succeeded".into(),
        error_code: None,
        failed_after_first_token: false,
        input_tokens: Some(12),
        output_tokens: Some(8),
        total_tokens: Some(20),
        started_at,
        first_token_at: Some(started_at + Duration::milliseconds(25)),
        finished_at: Some(started_at + Duration::milliseconds(80)),
        time_to_first_token_ms: Some(25),
        total_duration_ms: Some(80),
    }
}

#[tokio::test]
async fn provider_request_log_worker_acks_invalid_payloads_without_persisting() {
    let (state, _) = test_api_state_with_database_url().await;
    let task_queue = state.infrastructure.task_queue();
    let task_id = task_queue
        .enqueue(
            PROVIDER_REQUEST_LOG_QUEUE,
            json!({ "attempt_id": "not-a-complete-request-log" }),
            None,
        )
        .await
        .unwrap();

    let outcome = consume_provider_request_log_batch(
        state.clone(),
        "provider-request-log-test",
        Duration::seconds(10),
    )
    .await
    .unwrap();
    let entries = task_queue.list_ephemeral_entries().await.unwrap();

    assert_eq!(
        outcome,
        ProviderRequestLogWorkerOutcome::Processed {
            claimed: 1,
            persisted: 0,
            discarded: 1,
        }
    );
    assert!(entries.iter().all(|entry| entry.key != task_id));
}

#[tokio::test]
async fn provider_request_log_worker_batches_valid_payloads_and_acks_each_task() {
    let (state, _) = test_api_state_with_database_url().await;
    let task_queue = state.infrastructure.task_queue();
    let scope_id = Uuid::now_v7();
    let first = request_log_task(scope_id, Uuid::now_v7());
    let second = request_log_task(scope_id, Uuid::now_v7());
    let first_task_id = task_queue
        .enqueue(
            PROVIDER_REQUEST_LOG_QUEUE,
            serde_json::to_value(&first).unwrap(),
            Some(&first.attempt_id.to_string()),
        )
        .await
        .unwrap();
    let second_task_id = task_queue
        .enqueue(
            PROVIDER_REQUEST_LOG_QUEUE,
            serde_json::to_value(&second).unwrap(),
            Some(&second.attempt_id.to_string()),
        )
        .await
        .unwrap();

    let outcome = consume_provider_request_log_batch(
        state.clone(),
        "provider-request-log-test",
        Duration::seconds(10),
    )
    .await
    .unwrap();
    let page = state
        .store
        .list_model_provider_request_logs_page(ListModelProviderRequestLogsPageInput {
            scope_id,
            application_name: None,
            provider_instance_id: None,
            model_id: None,
            status: None,
            zero_output_only: false,
            started_after: None,
            started_before: None,
            page: 1,
            page_size: 20,
        })
        .await
        .unwrap();
    let entries = task_queue.list_ephemeral_entries().await.unwrap();

    assert_eq!(
        outcome,
        ProviderRequestLogWorkerOutcome::Processed {
            claimed: 2,
            persisted: 2,
            discarded: 0,
        }
    );
    assert_eq!(page.total_count, 2);
    assert!(page
        .items
        .iter()
        .any(|item| item.attempt_id == first.attempt_id));
    assert!(page
        .items
        .iter()
        .any(|item| item.attempt_id == second.attempt_id));
    assert!(entries
        .iter()
        .all(|entry| entry.key != first_task_id && entry.key != second_task_id));
}
