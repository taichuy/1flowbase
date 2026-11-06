use super::*;
use crate::ports::{FrozenLifecyclePublication, LifecyclePublicationPlan};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn dropping_transaction_guard_never_publishes_but_explicit_commit_does() {
    let calls = Arc::new(AtomicUsize::new(0));
    let guard = || {
        let calls = calls.clone();
        FrozenLifecyclePublication::native(LifecyclePublicationPlan {
            graph_fingerprint: "frozen".into(),
            subscribers: Vec::new(),
        })
        .after_commit(move || {
            calls.fetch_add(1, Ordering::SeqCst);
        })
    };
    drop(guard());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    guard().committed();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn causal_context_is_complete_and_spawn_requires_explicit_transfer() {
    let ancestors = (0..257).map(|_| Uuid::now_v7()).collect::<Vec<_>>();
    with_lifecycle_workflow_ancestors(ancestors.clone(), async {
        assert_eq!(lifecycle_workflow_ancestor_ids(), ancestors);
        let copied = lifecycle_workflow_ancestor_ids();
        let joined = tokio::spawn(with_lifecycle_workflow_ancestors(copied, async {
            LifecycleObservationContext::trusted(EventSourceScope::System, "trusted-native-write")
        }))
        .await
        .unwrap();
        assert_eq!(joined.workflow_ancestor_ids, ancestors);
    })
    .await;
    assert!(lifecycle_workflow_ancestor_ids().is_empty());
}
