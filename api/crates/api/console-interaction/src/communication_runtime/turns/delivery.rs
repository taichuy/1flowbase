use super::*;
use serde_json::json;

pub(super) async fn send(
    state: &ApiState,
    input: &ClaimedCommunicationTurn,
    actor: ActorContext,
    reply: String,
    stage: &AtomicU8,
) -> Result<()> {
    let _permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
    // This check is inside the session cancellation lease and immediately precedes
    // supplier submission. Rotation fences it even if cancellation raced reservation.
    ensure!(
        state
            .notification_service
            .renew_turn_claim(&input.claim, LEASE_SECONDS)
            .await?,
        "communication_turn_fenced"
    );
    let result = state
        .notification_service
        .send_to(
            &actor,
            domain::NotificationSource::Workflow,
            input.binding.instance_id,
            json!({"text": reply}),
            extension_contracts::NotificationSendTarget {
                sender_id: input.conversation.external_identity.clone(),
                conversation_id: input.conversation.external_conversation_key.clone(),
                provider_message_id: input.anchor_external_message_key.clone(),
            },
        )
        .await;
    let progress = match result {
        Ok(_) => CommunicationTurnProgress::SendSucceeded,
        Err(error) => match super::super::execution::send_failure(&error) {
            control_plane::ports::CommunicationProgress::SendFailed { failure_code } => {
                CommunicationTurnProgress::SendFailed { failure_code }
            }
            _ => CommunicationTurnProgress::SendUnknown {
                failure_code: "provider_result_unknown".into(),
            },
        },
    };
    ensure!(
        state
            .notification_service
            .record_turn_progress(&input.claim, progress)
            .await?,
        "communication_turn_claim_lost"
    );
    stage.store(4, Ordering::SeqCst);
    Ok(())
}
