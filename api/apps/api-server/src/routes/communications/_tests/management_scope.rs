use super::*;

#[tokio::test]
async fn all_management_inputs_reject_delegated_history_authority() {
    let inputs = [
        Input::ManagementConversations(Default::default()),
        Input::ManagementMessages(Default::default()),
        Input::ManagementMessage(Uuid::now_v7()),
        Input::ManagementHistory(Uuid::now_v7(), Default::default()),
    ];
    for input in &inputs {
        assert!(input.authorize_scope().is_ok());
    }
    scope::with_scope(
        domain::CommunicationHistoryScope {
            workspace_id: Uuid::now_v7(),
            instance_id: Uuid::now_v7(),
            external_identity: "alice".into(),
        },
        async {
            for input in inputs {
                assert!(input.authorize_scope().is_err());
            }
        },
    )
    .await;
}
