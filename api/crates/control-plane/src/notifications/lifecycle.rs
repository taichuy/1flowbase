//! Observe the result of an actual provider attempt, independent of HTTP success or reply claims.
use super::*;
use control_plane_contracts::ports::{
    lifecycle_workflow_ancestor_ids, CommunicationFactContent, CommunicationLifecycleFact,
    CommunicationSendResultFact,
};

pub(super) struct SendAttemptObservation {
    pub result_kind: CommunicationSendResultKind,
    pub failure_code: Option<String>,
    pub archive: bool,
    pub receipt: Option<extension_contracts::NotificationSendReceipt>,
}

impl SendAttemptObservation {
    pub fn unknown() -> Self {
        Self {
            result_kind: CommunicationSendResultKind::Unknown,
            failure_code: Some("provider_result_unknown".into()),
            archive: false,
            receipt: None,
        }
    }
}

impl<R: NotificationRepository> NotificationService<R> {
    pub(super) async fn observe_send_result(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        log_id: Uuid,
        source: NotificationSource,
        target: Option<extension_contracts::NotificationSendTarget>,
        attempt: SendAttemptObservation,
        context: Option<control_plane_contracts::ports::CommunicationSendContext>,
    ) {
        let Some(observer) = &self.communication_lifecycle_observer else {
            return;
        };
        // The repository log is already the validated credential-checked projection. Never
        // re-project raw caller input or include plugin output/extra/config/authorization.
        let log = self
            .repository
            .get_notification_send_log(actor.current_workspace_id, log_id)
            .await
            .ok()
            .flatten();
        observer.observed(CommunicationLifecycleFact::SendResult(
            CommunicationSendResultFact {
                workspace_id: actor.current_workspace_id,
                instance_id,
                send_log_id: Some(log_id),
                input_id: context.as_ref().map(|context| context.input_id),
                turn_id: None,
                conversation_id: context
                    .as_ref()
                    .map(|context| context.scope.conversation_id),
                type_code: log.as_ref().and_then(|log| log.type_code.clone()),
                source: Some(source),
                result_kind: attempt.result_kind,
                failure_code: attempt.failure_code,
                target,
                content: log.and_then(|log| log.content).map(|content| {
                    CommunicationFactContent::Text {
                        message_id: None,
                        content,
                    }
                }),
                occurred_at_ms: (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
                    as i64,
                workflow_ancestor_ids: lifecycle_workflow_ancestor_ids(),
            },
        ));
    }
}
