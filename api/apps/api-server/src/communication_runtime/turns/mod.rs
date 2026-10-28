//! One active executor per durable turn; different external chats can progress independently.
use super::*;
use control_plane::{
    orchestration_runtime::{
        CommunicationExecutionBinding, CommunicationInputPreparer, CommunicationTurnExecutionBridge,
    },
    ports::{
        ClaimedCommunicationTurn, CommunicationTurnClaim, CommunicationTurnProgress,
        OrchestrationRuntimeRepository,
    },
};
use domain::CommunicationTurnState;
use std::sync::atomic::{AtomicU8, Ordering};

mod delivery;
mod dispatch;

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), mut stop: watch::Receiver<bool>) {
    let mut tasks = JoinSet::new();
    loop {
        if *stop.borrow() {
            break;
        }
        let claimed = match state.system_maintenance.try_enter_write(
            control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
        ) {
            Ok(_permit) => {
                state
                    .notification_service
                    .claim_pending_turn(key.0, key.1, LEASE_SECONDS)
                    .await
            }
            Err(_) => Ok(None),
        };
        match claimed {
            Ok(Some(input)) => {
                tasks.spawn(own_turn(state.clone(), input, stop.clone()));
                continue;
            }
            Err(_) => tracing::warn!(instance_id=%key.1, "communication_turn_claim_failed"),
            Ok(None) => {}
        }
        tokio::select! {
            _ = stop.changed() => break,
            _ = tasks.join_next(), if !tasks.is_empty() => {},
            _ = tokio::time::sleep(RETRY_DELAY) => {},
        }
    }
    while tasks.join_next().await.is_some() {}
}

async fn own_turn(
    state: Arc<ApiState>,
    input: ClaimedCommunicationTurn,
    mut stop: watch::Receiver<bool>,
) {
    let claim = input.claim.clone();
    let session = match state
        .communication_activity
        .session(claim.session_id, claim.session_epoch)
    {
        Ok(session) => session,
        Err(_) => return,
    };
    let Ok(lease) = session.enter() else {
        return;
    };
    let stage = AtomicU8::new(if input.pending.turn.run_id.is_some() {
        1
    } else {
        0
    });
    let stopped = {
        let work = process(&state, input, &stage);
        tokio::pin!(work);
        let mut heartbeat =
            tokio::time::interval(Duration::from_secs(u64::from(LEASE_SECONDS / 3)));
        heartbeat.tick().await;
        loop {
            tokio::select! {
                biased;
                _ = session.cancelled() => break true,
                _ = stop.changed() => { session.cancel(); break true; },
                result = &mut work => {
                    if result.is_err() {
                        record_interruption(&state, &claim, &stage, "communication_execution_failed").await;
                        tracing::warn!(turn_id=%claim.turn_id, "communication_turn_execution_failed");
                    }
                    break false;
                }
                _ = heartbeat.tick() => {
                    let Ok(_permit) = state.system_maintenance.try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution) else { session.cancel(); break true; };
                    match state.notification_service.renew_turn_claim(&claim, LEASE_SECONDS).await {
                        Ok(true) => {}, _ => { session.cancel(); break true; },
                    }
                }
            }
        }
    }; // Drop provider, import and send futures before acknowledging cancellation.
    if stopped {
        record_interruption(&state, &claim, &stage, "host_stopped").await;
    }
    drop(lease);
    if stopped {
        if let Err(error) = state
            .communication_activity
            .cancel_session(claim.session_id)
            .await
        {
            tracing::warn!(session_id=%claim.session_id, %error, "communication_provider_cancel_join_failed");
        }
    }
}

async fn process(
    state: &Arc<ApiState>,
    input: ClaimedCommunicationTurn,
    stage: &AtomicU8,
) -> Result<()> {
    ensure!(
        state
            .notification_service
            .renew_turn_claim(&input.claim, LEASE_SECONDS)
            .await?,
        "communication_turn_fenced"
    );
    if input.pending.turn.state == CommunicationTurnState::Sealed {
        stage.store(2, Ordering::SeqCst);
        let actor = validated_actor(state, &input).await?;
        let reply = state
            .notification_service
            .reserve_turn_send(&input.claim)
            .await?
            .context("communication_reply_not_retryable")?;
        stage.store(3, Ordering::SeqCst);
        return delivery::send(state, &input, actor, reply, stage).await;
    }
    let preparer = Arc::new(turn_inputs::TurnInputPreparer::new(
        state.clone(),
        input.binding.clone(),
    ));
    let run_id = match input.pending.turn.run_id {
        Some(run_id) => run_id,
        None => dispatch::create(state, &input, preparer.as_ref()).await?,
    };
    stage.store(1, Ordering::SeqCst);
    let bridge = Arc::new(
        CommunicationTurnExecutionBridge::new(
            control_plane::notifications::NotificationService::new(
                state.store.clone(),
                state.provider_secret_master_key.clone(),
            ),
            input.claim.clone(),
            preparer,
        )
        .with_activity(state.communication_activity.clone()),
    );
    // Every initial request contains exactly member 1. Later members are appended by the
    // runtime hook, including when a queued run is reclaimed before its first checkpoint.
    let provider_session = state
        .communication_activity
        .session(input.claim.session_id, input.claim.session_epoch)?;
    let _provider_registration = state
        .communication_activity
        .register_provider_run(run_id, provider_session)?;
    let _registration = state.communication_activity.register_run(
        run_id,
        CommunicationExecutionBinding {
            port: bridge.clone(),
            initial_prompt_watermark: 1,
        },
    )?;
    let application_id = input.binding.config.application_id;
    let mut dispatched = false;
    loop {
        let metadata = state
            .store
            .get_flow_run_metadata(application_id, run_id)
            .await?
            .context("linked_run_missing")?;
        ensure!(
            metadata.created_by == input.binding.actor_user_id,
            "linked_run_actor_mismatch"
        );
        match metadata.status {
            domain::FlowRunStatus::Queued | domain::FlowRunStatus::Running
                if !dispatched && !state.store.flow_run_has_resume_claims(run_id).await? =>
            {
                dispatched = true;
                if dispatch::execute(state, &input, run_id).await.is_err() {
                    super::execution::runtime(state, state.store.clone())
                        .reject_communication_dispatch(application_id, run_id)
                        .await?;
                }
                continue;
            }
            domain::FlowRunStatus::Succeeded => {
                stage.store(2, Ordering::SeqCst);
                let actor = validated_actor(state, &input).await?;
                let receipt = bridge
                    .take_reserved_reply()
                    .await?
                    .context("communication_reserved_reply_missing")?;
                ensure!(
                    receipt.run_id == run_id && receipt.turn_id == input.claim.turn_id,
                    "communication_reply_owner_mismatch"
                );
                stage.store(3, Ordering::SeqCst);
                return delivery::send(state, &input, actor, receipt.final_reply, stage).await;
            }
            domain::FlowRunStatus::Failed
            | domain::FlowRunStatus::Cancelled
            | domain::FlowRunStatus::Incomplete => {
                state
                    .notification_service
                    .record_turn_progress(
                        &input.claim,
                        CommunicationTurnProgress::RunFailed {
                            failure_code: "run_failed".into(),
                        },
                    )
                    .await?;
                return Ok(());
            }
            _ => {}
        }
        tokio::time::sleep(RETRY_DELAY).await;
    }
}

async fn validated_actor(
    state: &ApiState,
    input: &ClaimedCommunicationTurn,
) -> Result<ActorContext> {
    let current = active_binding(
        state,
        (input.binding.workspace_id, input.binding.instance_id),
    )
    .await?;
    ensure!(
        current.revision == input.binding.revision,
        "binding_changed"
    );
    super::validate_binding(&state.store, &current).await
}

async fn record_interruption(
    state: &ApiState,
    claim: &CommunicationTurnClaim,
    stage: &AtomicU8,
    code: &str,
) {
    let progress = match stage.load(Ordering::SeqCst) {
        0 => CommunicationTurnProgress::RunFailed {
            failure_code: code.into(),
        },
        2 => CommunicationTurnProgress::SendFailed {
            failure_code: code.into(),
        },
        3 => CommunicationTurnProgress::SendUnknown {
            failure_code: "provider_result_unknown".into(),
        },
        _ => return,
    };
    let Ok(_permit) = state
        .system_maintenance
        .try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)
    else {
        return;
    };
    let _ = state
        .notification_service
        .record_turn_progress(claim, progress)
        .await;
}
