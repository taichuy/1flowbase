use super::*;
use control_plane::notifications::CommunicationCommandIntent;
use domain::CommunicationConversationScope;
use std::collections::BTreeSet;

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), mut stop: watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            return;
        }
        tokio::select! {
            _ = stop.changed() => return,
            result = admit_available(&state, key) => {
                if let Err(error) = result { tracing::warn!(workspace_id=%key.0,instance_id=%key.1,%error,"communication command admission deferred"); }
            }
        }
        if !wait(&mut stop).await {
            return;
        }
    }
}

/// Only classify the earliest unresolved receipt in a chat. A command remains the admission
/// barrier until its effect and reply receipt commit, independent of ordinary run progress.
pub(super) async fn admit_available(state: &ApiState, key: (Uuid, Uuid)) -> Result<()> {
    let _permit = match state
        .system_maintenance
        .try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)
    {
        Ok(permit) => permit,
        Err(_) => return Ok(()),
    };
    let binding = active_binding(state, key).await?;
    let actor = actor_for_binding(&state.store, &binding).await?;
    let candidates = state
        .notification_service
        .scan_pending_admissions(key.0, key.1, 100)
        .await?;
    let mut blocked = BTreeSet::new();
    for candidate in candidates {
        let conversation = candidate.conversation;
        if blocked.contains(&conversation.id) {
            continue;
        }
        ensure!(
            conversation.workspace_id == key.0 && conversation.instance_id == key.1,
            "command admission scope mismatch"
        );
        let scope = CommunicationConversationScope {
            workspace_id: key.0,
            instance_id: key.1,
            external_identity: conversation.external_identity,
            conversation_id: conversation.id,
        };
        if state
            .notification_service
            .pending_admissions(&scope)
            .await?
            .first()
            != Some(&candidate.input_id)
        {
            continue;
        }
        if let Some(request) = state
            .notification_service
            .command_for_input(&scope, candidate.input_id)
            .await?
        {
            if request.reply.is_none() {
                blocked.insert(scope.conversation_id);
            }
            continue;
        }
        let text = candidate.content.as_deref().unwrap_or_default();
        if !text.trim_start().starts_with('/') {
            state
                .notification_service
                .admit_input(&scope, candidate.input_id)
                .await?;
            continue;
        }
        // Classification errors never fall through into ordinary Agent-flow input.
        let intent = state
            .notification_service
            .resolve_command(&actor, &binding.config.command_settings, text)
            .await?;
        ensure!(
            !matches!(intent, CommunicationCommandIntent::Ordinary),
            "slash command classified as ordinary input"
        );
        state
            .notification_service
            .enqueue_command(&scope, candidate.input_id, intent)
            .await?;
        blocked.insert(scope.conversation_id);
    }
    Ok(())
}
