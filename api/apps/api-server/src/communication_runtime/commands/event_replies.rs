//! A workflow receives the immutable Host event destination, never a destination from node input.
use crate::app_state::{ApiNotificationService, ApiState};
use control_plane::{
    event_center::{EventCenterSignal, EventCenterSignalBody},
    notifications::{NotificationSenderPort, NotificationSource},
};
use control_plane_contracts::ports::{CommunicationCommandRepository, CommunicationSendContext};
use extension_contracts::NotificationSendTarget;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

struct EventReplySender {
    service: Arc<ApiNotificationService>,
    target: NotificationSendTarget,
    context: CommunicationSendContext,
}

#[async_trait::async_trait]
impl NotificationSenderPort for EventReplySender {
    async fn send(
        &self,
        actor: &domain::ActorContext,
        source: NotificationSource,
        instance_id: Uuid,
        input: Value,
    ) -> anyhow::Result<domain::NotificationSendResult> {
        anyhow::ensure!(
            instance_id == self.context.scope.instance_id,
            "event_reply_channel_mismatch"
        );
        self.service
            .send_to_with_context(
                actor,
                source,
                instance_id,
                input,
                self.target.clone(),
                self.context.clone(),
            )
            .await
    }
}

pub(crate) async fn sender_for_event(
    state: &Arc<ApiState>,
    signal: &EventCenterSignal,
    run_id: Uuid,
) -> anyhow::Result<Option<Arc<dyn NotificationSenderPort>>> {
    let EventCenterSignalBody::Published { payload } = &signal.body else {
        return Ok(None);
    };
    let Some(input_id) = payload
        .get("input_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
    else {
        return Ok(None);
    };
    // An arbitrary event with a similarly named property does not acquire command authority.
    let Some(command) = state
        .store
        .communication_command(signal.workspace_id, input_id)
        .await?
    else {
        return Ok(None);
    };
    let (_, target) = super::super::provider_activity::destination(state, signal).await?;
    Ok(Some(Arc::new(EventReplySender {
        service: state.notification_service.clone(),
        target,
        context: CommunicationSendContext {
            scope: command.scope,
            input_id,
            run_id: Some(run_id),
        },
    })))
}
