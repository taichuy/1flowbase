use super::*;
#[tokio::test]
async fn delegated_history_scope_rejects_another_channel_even_for_root() {
    let workspace = Uuid::now_v7();
    let channel = Uuid::now_v7();
    let actor = domain::ActorContext::root(Uuid::now_v7(), workspace, "root");
    let scope = CommunicationHistoryScope {
        workspace_id: workspace,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    with_scope(scope, async {
        assert_eq!(
            for_channel(&actor, channel)
                .unwrap()
                .unwrap()
                .external_identity,
            "alice"
        );
        assert!(for_channel(&actor, Uuid::now_v7()).is_err());
        let mut other = actor.clone();
        other.current_workspace_id = Uuid::now_v7();
        assert!(for_channel(&other, channel).is_err());
    })
    .await;
    assert!(for_channel(&actor, channel).unwrap().is_none());
}
#[tokio::test]
async fn simultaneous_communication_invocations_do_not_share_identity() {
    let workspace = Uuid::now_v7();
    let channel = Uuid::now_v7();
    let actor = domain::ActorContext::root(Uuid::now_v7(), workspace, "root");
    let a = CommunicationHistoryScope {
        workspace_id: workspace,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    let b = CommunicationHistoryScope {
        external_identity: "bob".into(),
        ..a.clone()
    };
    let (left, right) = tokio::join!(
        with_scope(a, async {
            tokio::task::yield_now().await;
            for_channel(&actor, channel)
                .unwrap()
                .unwrap()
                .external_identity
        }),
        with_scope(b, async {
            tokio::task::yield_now().await;
            for_channel(&actor, channel)
                .unwrap()
                .unwrap()
                .external_identity
        })
    );
    assert_eq!((left.as_str(), right.as_str()), ("alice", "bob"));
}

#[tokio::test]
async fn delegated_history_cannot_use_workspace_management_even_for_same_channel_root() {
    assert!(require_management_scope().is_ok());
    with_scope(
        CommunicationHistoryScope {
            workspace_id: Uuid::now_v7(),
            instance_id: Uuid::now_v7(),
            external_identity: "alice".into(),
        },
        async {
            assert!(require_management_scope().is_err());
        },
    )
    .await;
    assert!(require_management_scope().is_ok());
}
