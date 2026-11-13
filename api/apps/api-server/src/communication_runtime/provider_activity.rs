//! Best-effort native status shared by ordinary replies and overlapping event workflows.
use super::{active_binding, actor_for_binding};
use crate::app_state::ApiState;
use control_plane::event_center::{EventCenterSignal, EventCenterSignalBody};
use control_plane_contracts::ports::CommunicationCommandRepository;
use extension_contracts::{NotificationActivityState, NotificationSendTarget};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;
use uuid::Uuid;

type Key = (Uuid, Uuid, Uuid);
#[derive(Default)]
pub(super) struct ProviderActivity {
    entries: Mutex<HashMap<Key, watch::Sender<usize>>>,
}
pub(super) struct Lease {
    state: Arc<ApiState>,
    key: Key,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let entries = self
            .state
            .communication_activity
            .native_status
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(changed) = entries.get(&self.key) {
            let count = *changed.borrow();
            changed.send_replace(count.saturating_sub(1));
        }
    }
}

pub(super) async fn destination(
    state: &ApiState,
    signal: &EventCenterSignal,
) -> anyhow::Result<(Key, NotificationSendTarget)> {
    let EventCenterSignalBody::Published { payload } = &signal.body else {
        anyhow::bail!("not a command publication");
    };
    let input_id = Uuid::parse_str(payload["input_id"].as_str().unwrap_or_default())?;
    let record = state
        .store
        .communication_command(signal.workspace_id, input_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("command missing"))?;
    let scope = &record.scope;
    anyhow::ensure!(
        scope.workspace_id == signal.workspace_id
            && payload["instance_id"].as_str() == Some(scope.instance_id.to_string().as_str())
            && payload["conversation_id"].as_str()
                == Some(scope.conversation_id.to_string().as_str())
            && payload["arguments"].as_str() == Some(record.request.arguments.as_str())
            && record.request.command_id.starts_with("user/"),
        "command scope mismatch"
    );
    let binding = active_binding(state, (scope.workspace_id, scope.instance_id)).await?;
    anyhow::ensure!(
        binding
            .config
            .commands
            .iter()
            .filter_map(|command| command.event())
            .any(|command| command.enabled
                && command.command_id == record.request.command_id
                && command.contract_id == signal.contract_id),
        "command binding changed"
    );
    let target = state
        .notification_service
        .command_response_target(scope, input_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("command target missing"))?;
    Ok((
        (scope.workspace_id, scope.instance_id, scope.conversation_id),
        target,
    ))
}

async fn set_status(
    state: &ApiState,
    key: Key,
    target: &NotificationSendTarget,
    activity: NotificationActivityState,
) {
    let result = tokio::time::timeout(Duration::from_secs(8), async {
        let binding = active_binding(state, (key.0, key.1)).await?;
        let actor = actor_for_binding(&state.store, &binding).await?;
        state
            .notification_service
            .set_communication_activity(&actor, key.1, target.clone(), activity)
            .await
    })
    .await;
    // No provider credentials, destination or raw error belongs in the status log.
    if !matches!(result, Ok(Ok(_))) {
        tracing::debug!(instance_id = %key.1, "native communication activity unavailable");
    }
}

// Enter only for active generation/execution, using the durable input's supplier target.
// Scheduling the refresher never waits for provider I/O; dropping the lease also handles
// cancellation and errors without emitting a placeholder message or delaying the reply.
pub(super) fn enter(state: &Arc<ApiState>, key: Key, target: NotificationSendTarget) -> Lease {
    let mut entries = state
        .communication_activity
        .native_status
        .entries
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(changed) = entries.get(&key) {
        let count = *changed.borrow();
        changed.send_replace(count + 1);
    } else {
        let (changed, mut receiver) = watch::channel(1usize);
        entries.insert(key, changed);
        let owned = state.clone();
        tokio::spawn(async move {
            loop {
                let active = *receiver.borrow_and_update() > 0;
                set_status(
                    &owned,
                    key,
                    &target,
                    if active {
                        NotificationActivityState::Replying
                    } else {
                        NotificationActivityState::Idle
                    },
                )
                .await;
                if !active {
                    let mut entries = owned
                        .communication_activity
                        .native_status
                        .entries
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if *receiver.borrow() == 0 {
                        entries.remove(&key);
                        break;
                    }
                    continue;
                }
                tokio::select! {
                    _ = receiver.changed() => {},
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                }
            }
        });
    }
    Lease {
        state: state.clone(),
        key,
    }
}

pub(crate) async fn scope_event_activity<F: Future>(
    state: &Arc<ApiState>,
    signal: &EventCenterSignal,
    execution: F,
) -> F::Output {
    let _lease =
        match tokio::time::timeout(Duration::from_secs(3), destination(state, signal)).await {
            Ok(Ok((key, target))) => Some(enter(state, key, target)),
            _ => None,
        };
    execution.await
}
