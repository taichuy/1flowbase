use super::*;
use control_plane::ports::{
    CommitCommunicationReceiveInput, CommunicationReceiveAttachment, CommunicationReceiveMessage,
    SaveNotificationAuthorizationInput,
};
use extension_contracts::{
    NotificationOperation, NotificationReceiveOutcome, NotificationRuntimeResult,
    NOTIFICATION_MAX_RECEIVE_MESSAGES, NOTIFICATION_MAX_RECEIVE_WAIT_SECONDS,
};

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), mut stop: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = stop.changed() => return,
            result = receive(&state,key) => {
                if result.is_err() {
                    // Only a safe fixed code is logged. Runtime envelopes can contain credentials.
                    tracing::warn!(instance_id=%key.1, "communication_receive_failed");
                }
            }
        }
        tokio::select! { _ = stop.changed() => return, _ = tokio::time::sleep(RETRY_DELAY) => {} }
    }
}
async fn receive(state: &ApiState, key: (Uuid, Uuid)) -> Result<()> {
    let _write_permit = match state
        .system_maintenance
        .try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationReceive)
    {
        Ok(permit) => permit,
        Err(_) => return Ok(()),
    };
    let binding = active_binding(state, key).await?;
    let mut context = ProviderContext::load(state, &binding).await?;
    let cursor = state
        .notification_service
        .receive_cursor(key.0, key.1)
        .await?;
    let result = context
        .invoke(context.input(NotificationOperation::Receive {
            cursor,
            max_messages: NOTIFICATION_MAX_RECEIVE_MESSAGES,
            wait_seconds: NOTIFICATION_MAX_RECEIVE_WAIT_SECONDS,
        }))
        .await?;
    let NotificationRuntimeResult::Receive {
        outcome,
        authorization,
        managed_secret_patch,
    } = result
    else {
        anyhow::bail!("receive_result_invalid")
    };
    // validate_for already rejects undeclared patch keys and legacy cursor secret writes.
    for (key, value) in managed_secret_patch {
        if value.is_null() {
            context.authorization.managed_secrets.remove(&key);
        } else {
            context.authorization.managed_secrets.insert(key, value);
        }
    }
    let authorization = authorization
        .or(context.authorization.authorization)
        .unwrap_or(extension_contracts::NotificationAuthorizationView {
            status: extension_contracts::NotificationAuthorizationStatus::Authorized,
            user_action: None,
            delivery_window: None,
        });
    let (candidate_cursor, messages) = match outcome {
        NotificationReceiveOutcome::Succeeded { batch } => (
            batch.candidate_cursor,
            batch
                .messages
                .into_iter()
                .map(|message| {
                    Ok(CommunicationReceiveMessage {
                        provider_message_id: message.provider_message_id,
                        conversation_id: message.conversation_id,
                        sender_id: message.sender.id,
                        sender_display_name: message.sender.display_name,
                        received_at_ms: i64::try_from(message.received_at_ms)?,
                        text: message.text,
                        attachments: message
                            .attachments
                            .into_iter()
                            .map(|attachment| {
                                Ok(CommunicationReceiveAttachment {
                                    provider_attachment_id: attachment.provider_attachment_id,
                                    file_name: attachment.file_name,
                                    media_type: attachment.media_type,
                                    size_bytes: attachment
                                        .size_bytes
                                        .map(i64::try_from)
                                        .transpose()?,
                                    reference: serde_json::to_value(attachment.reference)?,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        NotificationReceiveOutcome::Failed { .. } => {
            // Persist authorization invalidation with CAS, but never advance a failed poll cursor.
            let _ = state
                .store
                .save_notification_authorization(SaveNotificationAuthorizationInput {
                    workspace_id: key.0,
                    instance_id: key.1,
                    expected_revision: context.authorization.revision,
                    authorization,
                    managed_secrets: context.authorization.managed_secrets,
                    master_key: state.provider_secret_master_key.clone(),
                })
                .await?;
            anyhow::bail!("receive_provider_rejected")
        }
    };
    let committed = state
        .notification_service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: SaveNotificationAuthorizationInput {
                workspace_id: key.0,
                instance_id: key.1,
                expected_revision: context.authorization.revision,
                authorization,
                managed_secrets: context.authorization.managed_secrets,
                master_key: String::new(),
            },
            candidate_cursor,
            messages,
        })
        .await?;
    ensure!(committed.is_some(), "receive_superseded");
    Ok(())
}
