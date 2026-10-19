use super::*;
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_owned_operation_shutdown() {
    let gate = ManagedOperationOwner::new(1);
    let permit = gate.admit(ManagedOwnedOperation::Create).unwrap();
    assert!(gate.admit(ManagedOwnedOperation::Retirement).is_err());
    let (release, wait) = tokio::sync::oneshot::channel::<()>();
    let (started, entered) = tokio::sync::oneshot::channel();
    let owner = tokio::spawn(async move {
        let _permit = permit;
        started.send(()).unwrap();
        wait.await.unwrap();
    });
    entered.await.unwrap();
    drop(owner); // Protocol waiter cancellation cannot release the owner's permit.
    gate.close();
    assert!(gate.admit(ManagedOwnedOperation::Candidate).is_err());
    assert!(gate.wait(std::time::Duration::ZERO).await.is_err());
    assert_eq!(gate.snapshot().active, [0, 1, 0, 0]);
    release.send(()).unwrap();
    gate.wait(std::time::Duration::from_secs(2)).await.unwrap();
    assert_eq!(gate.snapshot().active, [0; 4]);
}
