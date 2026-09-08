use super::*;
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_hash_shutdown() {
    let host = RuntimeExtensionHost::new(OffsetDateTime::now_utc()).unwrap();
    host.mark_ready().unwrap();
    let (started, mut entered) = tokio::sync::mpsc::channel(4);
    let mut releases = Vec::new();
    for _ in 0..4 {
        let permit = host.admit_managed_hash().unwrap();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let started = started.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            started.blocking_send(()).unwrap();
            wait.recv().unwrap();
        });
        drop(handle); // Dropped async waiter cannot return the blocking hash admission permit.
        releases.push(release);
    }
    for _ in 0..4 {
        entered.recv().await.unwrap();
    }
    assert!(host
        .admit_managed_hash()
        .unwrap_err()
        .to_string()
        .contains("capacity"));
    host.drain().await.unwrap();
    assert!(host.admit_managed_hash().is_err());
    assert!(tokio::time::timeout(std::time::Duration::ZERO, host.stop())
        .await
        .is_err());
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Draining);
    for release in releases {
        release.send(()).unwrap();
    }
    tokio::time::timeout(std::time::Duration::from_secs(2), host.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Stopped);
}
