use std::{sync::Arc, time::Duration as StdDuration};

use anyhow::{anyhow, Result};
use control_plane::ports::{
    ClaimedTask, ProviderRequestLogTask, TaskQueue, PROVIDER_REQUEST_LOG_QUEUE,
};
use control_plane::system_recovery::SystemWriteOwner;
use serde_json::{json, Map, Value};
use time::Duration;
use tracing::{error, warn};

use crate::app_state::ApiState;

const MAX_BATCH_SIZE: usize = 50;
const MAX_BATCH_WINDOW: StdDuration = StdDuration::from_millis(20);
const CLAIM_RETRY_INTERVAL: StdDuration = StdDuration::from_millis(1);
const WRITE_RETRY_DELAYS: [StdDuration; 2] =
    [StdDuration::from_millis(100), StdDuration::from_millis(300)];
const WORKER_ID: &str = "provider-request-log-worker";
const VISIBILITY_TIMEOUT: Duration = Duration::seconds(10);
const IDLE_SLEEP: StdDuration = StdDuration::from_millis(200);
const SHUTDOWN_DRAIN_TIMEOUT: StdDuration = StdDuration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRequestLogWorkerOutcome {
    PausedForMaintenance,
    QueueUnavailable,
    NoTask,
    Processed {
        claimed: usize,
        persisted: usize,
        discarded: usize,
    },
}

pub async fn consume_provider_request_log_batch(
    state: Arc<ApiState>,
    worker_id: &str,
    visibility_timeout: Duration,
) -> Result<ProviderRequestLogWorkerOutcome> {
    let _write_permit = match state
        .system_maintenance
        .try_enter_write(SystemWriteOwner::ProviderRequestLogPersistence)
    {
        Ok(permit) => permit,
        Err(_) => return Ok(ProviderRequestLogWorkerOutcome::PausedForMaintenance),
    };
    let Some(task_queue) = state.infrastructure.registered_task_queue() else {
        return Ok(ProviderRequestLogWorkerOutcome::QueueUnavailable);
    };
    let Some(first_task) = task_queue
        .claim(PROVIDER_REQUEST_LOG_QUEUE, worker_id, visibility_timeout)
        .await?
    else {
        return Ok(ProviderRequestLogWorkerOutcome::NoTask);
    };

    let deadline = tokio::time::Instant::now() + MAX_BATCH_WINDOW;
    let mut tasks = vec![first_task];
    while tasks.len() < MAX_BATCH_SIZE && tokio::time::Instant::now() < deadline {
        match task_queue
            .claim(PROVIDER_REQUEST_LOG_QUEUE, worker_id, visibility_timeout)
            .await?
        {
            Some(task) => tasks.push(task),
            None => {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                tokio::time::sleep(CLAIM_RETRY_INTERVAL.min(remaining)).await;
            }
        }
    }

    let claimed = tasks.len();
    let mut valid_tasks = Vec::with_capacity(claimed);
    let mut records = Vec::with_capacity(claimed);
    let mut discarded = 0;
    for task in tasks {
        match serde_json::from_value::<ProviderRequestLogTask>(task.payload.clone()) {
            Ok(record) => {
                if let Some(receipt) = record
                    .provider_timing_receipt
                    .as_ref()
                    .and_then(safe_provider_timing_log_receipt)
                {
                    tracing::info!(
                        attempt_id = %record.attempt_id,
                        provider_code = %record.provider_code,
                        attempt_index = record.attempt_index,
                        provider_timing_receipt = %receipt,
                        "provider request timing receipt"
                    );
                }
                valid_tasks.push(task);
                records.push(record);
            }
            Err(decode_error) => {
                acknowledge_task(task_queue.as_ref(), &task, worker_id).await?;
                discarded += 1;
                warn!(
                    task_id = %task.task_id,
                    error = %decode_error,
                    "discarded invalid provider request log task"
                );
            }
        }
    }

    if records.is_empty() {
        return Ok(ProviderRequestLogWorkerOutcome::Processed {
            claimed,
            persisted: 0,
            discarded,
        });
    }

    let write_result = insert_batch_with_retries(state.as_ref(), &records).await;
    let persisted = if write_result.is_ok() {
        records.len()
    } else {
        0
    };
    if let Err(write_error) = write_result {
        discarded += records.len();
        error!(
            error = %write_error,
            task_count = records.len(),
            "discarded provider request log batch after write retries"
        );
    }

    for task in &valid_tasks {
        acknowledge_task(task_queue.as_ref(), task, worker_id).await?;
    }

    Ok(ProviderRequestLogWorkerOutcome::Processed {
        claimed,
        persisted,
        discarded,
    })
}

pub(crate) fn safe_provider_timing_log_receipt(receipt: &Value) -> Option<Value> {
    if receipt.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let mut stages = Map::new();
    for name in [
        "ingress", "mapping", "flow", "queue", "connect", "upstream", "flush",
    ] {
        let duration_ms = receipt
            .pointer(&format!("/stages/{name}/duration_ms"))
            .and_then(Value::as_u64)
            .filter(|value| *value <= 24 * 60 * 60 * 1_000);
        stages.insert(
            name.to_string(),
            json!({
                "duration_ms": duration_ms,
                "availability": if duration_ms.is_some() { "observed" } else { "unavailable" },
            }),
        );
    }
    let connection = receipt.get("connection").and_then(Value::as_object).map(|value| {
        json!({
            "cold": value.get("cold").and_then(Value::as_bool),
            "reused": value.get("reused").and_then(Value::as_bool),
            "generation": value.get("generation").and_then(Value::as_u64),
            "connection_age_ms": value.get("connection_age_ms").and_then(Value::as_u64),
            "ttl_remaining_ms": value.get("ttl_remaining_ms").and_then(Value::as_u64),
            "physical_state": value.get("physical_state").and_then(Value::as_str).filter(|state| matches!(*state, "ready" | "draining" | "closing" | "closed" | "faulted")),
            "close_reason": value.get("close_reason").and_then(Value::as_str).filter(|reason| matches!(*reason, "requested_drain" | "requested_close" | "ttl_expired" | "capacity_rejected" | "transport_fault" | "invocation_deadline")),
            "close_acknowledged": value.get("close_acknowledged").and_then(Value::as_bool),
        })
    });
    let termination_kind = receipt
        .get("termination_kind")
        .and_then(Value::as_str)
        .filter(|kind| {
            matches!(
                *kind,
                "completed"
                    | "upstream_error"
                    | "transport_error"
                    | "deadline"
                    | "failed"
                    | "empty_response"
            )
        })
        .unwrap_or("unknown");
    Some(json!({
        "schema_version": 1,
        "attempt_index": receipt.get("attempt_index").and_then(Value::as_u64),
        "stages": stages,
        "connection": connection,
        "termination_kind": termination_kind,
    }))
}

async fn insert_batch_with_retries(
    state: &ApiState,
    records: &[ProviderRequestLogTask],
) -> Result<()> {
    let mut result = state
        .store
        .insert_model_provider_request_logs_batch(records)
        .await;
    for delay in WRITE_RETRY_DELAYS {
        if result.is_ok() {
            break;
        }
        tokio::time::sleep(delay).await;
        result = state
            .store
            .insert_model_provider_request_logs_batch(records)
            .await;
    }
    result
}

async fn acknowledge_task(
    task_queue: &dyn TaskQueue,
    task: &ClaimedTask,
    worker_id: &str,
) -> Result<()> {
    if task_queue
        .ack(PROVIDER_REQUEST_LOG_QUEUE, &task.task_id, worker_id)
        .await?
    {
        Ok(())
    } else {
        Err(anyhow!(
            "provider request log task acknowledgement failed: {}",
            task.task_id
        ))
    }
}

pub fn spawn_provider_request_log_worker(state: Arc<ApiState>) {
    tokio::spawn(run_provider_request_log_worker(state));
}

async fn run_provider_request_log_worker(state: Arc<ApiState>) {
    let shutdown = crate::shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                drain_provider_request_logs(state.clone()).await;
                return;
            }
            outcome = consume_provider_request_log_batch(state.clone(), WORKER_ID, VISIBILITY_TIMEOUT) => {
                match outcome {
                    Ok(ProviderRequestLogWorkerOutcome::PausedForMaintenance) => {
                        // Return to the select promptly so shutdown can still win
                        // while recovery keeps the worker paused.
                        tokio::time::sleep(IDLE_SLEEP).await;
                    }
                    Ok(ProviderRequestLogWorkerOutcome::NoTask | ProviderRequestLogWorkerOutcome::QueueUnavailable) => {
                        tokio::time::sleep(IDLE_SLEEP).await;
                    }
                    Ok(_) => {}
                    Err(worker_error) => {
                        error!(error = %worker_error, "provider request log worker iteration failed");
                        tokio::time::sleep(IDLE_SLEEP).await;
                    }
                }
            }
        }
    }
}

async fn drain_provider_request_logs(state: Arc<ApiState>) {
    let deadline = tokio::time::Instant::now() + SHUTDOWN_DRAIN_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        match consume_provider_request_log_batch(state.clone(), WORKER_ID, VISIBILITY_TIMEOUT).await
        {
            Ok(
                ProviderRequestLogWorkerOutcome::PausedForMaintenance
                | ProviderRequestLogWorkerOutcome::NoTask
                | ProviderRequestLogWorkerOutcome::QueueUnavailable,
            ) => return,
            Ok(_) => {}
            Err(worker_error) => {
                error!(error = %worker_error, "provider request log shutdown drain failed");
                return;
            }
        }
    }
}
