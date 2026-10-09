//! Persistent log-deletion execution, independent from HTTP response delivery.
use crate::app_state::ApiState;
use control_plane::{agent_logs::AgentLogsService, system_recovery::SystemWriteOwner};
use std::{sync::Arc, time::Duration};

pub fn spawn(state: Arc<ApiState>) {
    tokio::spawn(async move {
        let shutdown = crate::shutdown_signal();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _=&mut shutdown => return,
                _=tokio::time::sleep(Duration::from_millis(250)) => {}
            }
            let Ok(_permit) = state
                .system_maintenance
                .try_enter_write(SystemWriteOwner::ApplicationLogDeletion)
            else {
                continue;
            };
            if let Err(error) = AgentLogsService::new(state.store.clone())
                .advance_next_deletion()
                .await
            {
                tracing::warn!(%error,"application log deletion batch failed; durable status retained");
            }
        }
    });
}
