use super::*;

#[tokio::test]
async fn hot_lifecycle_reclamation_shutdown_timeout_preserves_join_owner() {
    let worker = ReclamationWorker::default();
    let (release, blocked) = tokio::sync::oneshot::channel::<()>();
    *worker.task.lock().await = Some(tokio::spawn(async move {
        let _ = blocked.await;
    }));
    worker.stop();
    assert!(worker
        .wait_stopped(std::time::Duration::ZERO)
        .await
        .is_err());
    assert!(worker.task.lock().await.is_some());
    release.send(()).unwrap();
    worker
        .wait_stopped(std::time::Duration::from_secs(1))
        .await
        .unwrap();
    assert!(worker.task.lock().await.is_none());
}
