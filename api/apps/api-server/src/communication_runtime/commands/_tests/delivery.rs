use super::*;
use control_plane::{
    errors::ControlPlaneError,
    notifications::{BuiltinCommunicationIntent, CommunicationCommandIntent},
};
use extension_contracts::CommunicationCommandReply;

#[test]
fn communication_command_transport_uncertainty_never_becomes_retryable_failure() {
    for error in [
        anyhow::anyhow!("connection lost"),
        ControlPlaneError::UpstreamUnavailable("notification_provider_failed").into(),
    ] {
        assert!(matches!(
            super::super::delivery::send_failure(&error),
            CommunicationCommandSendOutcome::Unknown
        ));
    }
    assert!(matches!(
        super::super::delivery::send_failure(
            &ControlPlaneError::UpstreamUnavailable("provider_rejected").into()
        ),
        CommunicationCommandSendOutcome::Failed
    ));
}

#[tokio::test]
async fn communication_command_response_target_is_persisted_scoped_and_uncertain_send_stays_reserved(
) {
    let fixture = Fixture::new().await;
    let (scope, input) = fixture
        .receive("alice", "external-original-key", "/help")
        .await;
    let event = fixture
        .state
        .notification_service
        .enqueue_command(
            &scope,
            input,
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Help),
        )
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .command_response_target(&scope, event.event_id)
        .await
        .unwrap()
        .is_none());
    let claim = fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .unwrap();
    fixture
        .state
        .notification_service
        .complete_builtin_command(
            &claim,
            Uuid::parse_str(&event.request.session_id).unwrap(),
            CommunicationCommandReply {
                text: "reply".into(),
            },
            None,
        )
        .await
        .unwrap();
    let target = fixture
        .state
        .notification_service
        .command_response_target(&scope, event.event_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.sender_id, "alice");
    assert_eq!(target.conversation_id, "external-chat-alice");
    assert_eq!(target.provider_message_id, "external-original-key");
    for foreign in [
        CommunicationConversationScope {
            external_identity: "bob".into(),
            ..scope.clone()
        },
        CommunicationConversationScope {
            workspace_id: Uuid::now_v7(),
            ..scope.clone()
        },
        CommunicationConversationScope {
            conversation_id: Uuid::now_v7(),
            ..scope.clone()
        },
    ] {
        assert!(fixture
            .state
            .notification_service
            .command_response_target(&foreign, event.event_id)
            .await
            .unwrap()
            .is_none());
    }
    let send = fixture
        .state
        .notification_service
        .reserve_command_response(&scope, event.event_id)
        .await
        .unwrap()
        .unwrap();
    fixture
        .state
        .notification_service
        .record_command_response(
            &scope,
            event.event_id,
            send.claim_id,
            CommunicationCommandSendOutcome::Unknown,
        )
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .reserve_command_response(&scope, event.event_id)
        .await
        .unwrap()
        .is_none());
    assert!(fixture
        .state
        .notification_service
        .pending_command_responses(scope.workspace_id, scope.instance_id, 10)
        .await
        .unwrap()
        .is_empty());
}
