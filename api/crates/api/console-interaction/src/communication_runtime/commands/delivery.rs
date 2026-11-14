use super::*;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        ClaimedCommunicationCommandResponse, CommunicationCommandRecord,
        CommunicationCommandSendOutcome,
    },
};
use domain::NotificationSource;
use serde_json::json;

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), mut stop: watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            return;
        }
        if let Err(error) = deliver_pending(&state, key, stop.clone()).await {
            tracing::warn!(workspace_id=%key.0,instance_id=%key.1,%error,"communication command response deferred");
        }
        if !wait(&mut stop).await {
            return;
        }
    }
}

async fn deliver_pending(
    state: &ApiState,
    key: (Uuid, Uuid),
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let pending = {
        let _permit = match state.system_maintenance.try_enter_write(
            control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
        ) {
            Ok(permit) => permit,
            Err(_) => return Ok(()),
        };
        state
            .notification_service
            .pending_command_responses(key.0, key.1, 100)
            .await?
    };
    for record in pending {
        if *stop.borrow() {
            return Ok(());
        }
        // One failed target does not suppress replies belonging to other conversations.
        if let Err(error) = deliver(state, &record, &mut stop).await {
            tracing::warn!(event_id=%record.event_id,%error,"communication command response not completed");
        }
    }
    Ok(())
}

async fn deliver(
    state: &ApiState,
    record: &CommunicationCommandRecord,
    stop: &mut watch::Receiver<bool>,
) -> Result<()> {
    let scope = &record.scope;
    let binding = active_binding(state, (scope.workspace_id, scope.instance_id)).await?;
    let actor = actor_for_binding(&state.store, &binding).await?;
    let current = state
        .notification_service
        .active_session(scope)
        .await?
        .context("command reply session missing")?;
    if Some(current.id) != record.response_session_id {
        return Ok(());
    }
    let activity = state
        .communication_activity
        .session(current.id, current.epoch)?;
    let _lease = activity.enter()?;
    let target = state
        .notification_service
        .command_response_target(scope, record.event_id)
        .await?
        .context("command response destination missing")?;
    ensure!(
        target.sender_id == scope.external_identity,
        "command response destination scope mismatch"
    );
    let permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
    // The session owner can cancel work before reservation without creating an unknown send.
    let claim = tokio::select! {
        biased;
        _ = stop.changed() => return Ok(()),
        _ = activity.cancelled() => return Ok(()),
        result = state.notification_service.reserve_command_response(scope,record.event_id) => result?,
    };
    let Some(claim) = claim else {
        return Ok(());
    };
    let reply = claim
        .record
        .reply
        .as_ref()
        .context("reserved command reply missing")?;
    let send = state.notification_service.send_to(
        &actor,
        NotificationSource::Workflow,
        scope.instance_id,
        json!({"text":reply.text}),
        target,
    );
    tokio::pin!(send);
    let outcome = tokio::select! {
        biased;
        _ = stop.changed() => CommunicationCommandSendOutcome::Unknown,
        _ = activity.cancelled() => CommunicationCommandSendOutcome::Unknown,
        result = &mut send => match result { Ok(_)=>CommunicationCommandSendOutcome::Succeeded,Err(error)=>send_failure(&error) },
    };
    // Finite result recording is separate from handler ACK. Cancellation drops provider work;
    // already submitted sends remain unknown and are never automatically retried.
    record_outcome(state, &claim, outcome).await?;
    drop(permit);
    Ok(())
}

async fn record_outcome(
    state: &ApiState,
    claim: &ClaimedCommunicationCommandResponse,
    outcome: CommunicationCommandSendOutcome,
) -> Result<()> {
    ensure!(
        state
            .notification_service
            .record_command_response(
                &claim.record.scope,
                claim.record.event_id,
                claim.claim_id,
                outcome
            )
            .await?,
        "command response claim lost"
    );
    Ok(())
}

pub(super) fn send_failure(error: &anyhow::Error) -> CommunicationCommandSendOutcome {
    match error.downcast_ref::<ControlPlaneError>() {
        Some(
            ControlPlaneError::Conflict(_)
            | ControlPlaneError::InvalidInput(_)
            | ControlPlaneError::PermissionDenied(_)
            | ControlPlaneError::NotFound(_)
            | ControlPlaneError::UpstreamUnavailable(
                "provider_rejected" | "subscription_unavailable",
            ),
        ) => CommunicationCommandSendOutcome::Failed,
        _ => CommunicationCommandSendOutcome::Unknown,
    }
}
