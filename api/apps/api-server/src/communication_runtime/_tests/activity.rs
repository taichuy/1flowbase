use super::*;

#[tokio::test]
async fn communication_new_joins_execution_and_rejects_late_lease() {
    let registry = Arc::new(CommunicationActivity::default());
    let id = Uuid::now_v7();
    let session = registry.session(id, 1).unwrap();
    let lease = session.enter().unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (dropped, observed) = tokio::sync::oneshot::channel();
    struct ExecutionDrop(Option<tokio::sync::oneshot::Sender<()>>);
    impl Drop for ExecutionDrop {
        fn drop(&mut self) {
            let _ = self.0.take().unwrap().send(());
        }
    }
    let active = session.clone();
    let task = tokio::spawn(async move {
        let work = async {
            let _drop = ExecutionDrop(Some(dropped));
            let _ = started.send(());
            std::future::pending::<()>().await;
        };
        tokio::select! { biased; _ = active.cancelled() => {}, _ = work => {} }
        drop(lease);
    });
    // Ensure the provider future has been polled before requesting /new.
    ready.await.unwrap();
    registry.cancel_session(id).await;
    observed.await.unwrap();
    task.await.unwrap();
    assert!(session.enter().is_err());
    assert!(registry.session(id, 1).is_err());
    let other = registry.session(Uuid::now_v7(), 1).unwrap();
    assert!(other.enter().is_ok());
}
