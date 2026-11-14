use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;
pub fn abort_registered_assistant_execution(
    executions: &std::sync::Mutex<std::collections::HashMap<Uuid, tokio::task::AbortHandle>>,
    run_id: Uuid,
) -> Option<tokio::task::AbortHandle> {
    executions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&run_id)
}

pub fn abort_assistant_execution_in(
    executions: &Mutex<HashMap<Uuid, tokio::task::AbortHandle>>,
    run_id: Uuid,
) -> bool {
    let handle = abort_registered_assistant_execution(executions, run_id);
    if let Some(handle) = handle {
        handle.abort();
        true
    } else {
        false
    }
}
