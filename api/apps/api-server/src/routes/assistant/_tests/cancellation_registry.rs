use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use uuid::Uuid;

use super::super::abort_registered_assistant_execution;

/// BE-001 AC-001/002: the process-owned run handle stops detached execution before the next
/// gated Provider/tool boundary can be entered.
#[tokio::test]
async fn registered_assistant_execution_abort_prevents_gated_work() {
    let run_id = Uuid::now_v7();
    let entered_provider_boundary = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(tokio::sync::Notify::new());
    let entered = entered_provider_boundary.clone();
    let task_gate = gate.clone();
    let task = tokio::spawn(async move {
        task_gate.notified().await;
        entered.store(true, Ordering::SeqCst);
    });
    let executions = Mutex::new(std::collections::HashMap::from([(
        run_id,
        task.abort_handle(),
    )]));

    let abort = abort_registered_assistant_execution(&executions, run_id)
        .expect("registered execution must have an abort handle");
    abort.abort();
    gate.notify_waiters();
    let _ = task.await;

    assert!(!entered_provider_boundary.load(Ordering::SeqCst));
    assert!(executions
        .lock()
        .expect("execution registry lock should be available")
        .is_empty());
}
