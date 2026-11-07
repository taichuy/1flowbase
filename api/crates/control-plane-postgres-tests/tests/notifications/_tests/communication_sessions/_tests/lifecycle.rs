use super::*;
use control_plane_contracts::ports::{
    CommunicationFactContent, CommunicationLifecycleFact, CommunicationLifecycleObserver,
    CommunicationSendResultKind,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Facts(Mutex<Vec<CommunicationLifecycleFact>>);
impl CommunicationLifecycleObserver for Facts {
    fn observed(&self, fact: CommunicationLifecycleFact) {
        self.0.lock().unwrap().push(fact);
    }
}

#[tokio::test]
async fn communication_lifecycle_turn_interruption_is_unknown_with_original_reply_anchor() {
    let (store, _, scope, first) = fixture().await;
    let facts = Arc::new(Facts::default());
    let ancestor = Uuid::now_v7();
    let store = store
        .with_communication_lifecycle_observer(facts.clone())
        .with_workflow_ancestor_ids(vec![ancestor]);
    let service = Service::new(store.clone(), KEY.into());
    service.admit_input(&scope, first).await.unwrap().unwrap();
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .link_turn_run(&claimed.claim, Uuid::now_v7())
        .await
        .unwrap());
    assert!(service
        .checkpoint_turn(&claimed.claim, checkpoint(0, 1))
        .await
        .unwrap());
    assert!(matches!(
        service
            .seal_turn(&claimed.claim, candidate(1, 1))
            .await
            .unwrap(),
        CommunicationTurnSeal::Reserved { .. }
    ));
    sqlx::query("UPDATE communication_reply_turns SET claim_expires_at=clock_timestamp()-interval '1 second' WHERE id=$1").bind(claimed.claim.turn_id).execute(store.pool()).await.unwrap();
    for _ in 0..2 {
        assert!(service
            .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
            .await
            .unwrap()
            .is_none());
    }
    let recorded = facts.0.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let CommunicationLifecycleFact::SendResult(fact) = &recorded[0] else {
        panic!("send result")
    };
    assert_eq!(fact.result_kind, CommunicationSendResultKind::Unknown);
    assert_eq!(fact.failure_code.as_deref(), Some("send_interrupted"));
    assert_eq!(fact.input_id, Some(first));
    assert_eq!(fact.turn_id, Some(claimed.claim.turn_id));
    assert_eq!(fact.conversation_id, Some(scope.conversation_id));
    assert_eq!(fact.workspace_id, scope.workspace_id);
    assert_eq!(fact.workflow_ancestor_ids, vec![ancestor]);
    assert_eq!(fact.target.as_ref().unwrap().provider_message_id, "first");
    assert!(
        matches!(&fact.content, Some(CommunicationFactContent::Text { content, .. }) if content == "one complete answer")
    );
    drop(recorded);
    assert!(!store
        .retry_communication_message(scope.workspace_id, scope.instance_id, first)
        .await
        .unwrap());
}
