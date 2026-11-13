use super::*;
use std::sync::mpsc;

#[tokio::test(flavor = "current_thread")]
async fn sampling_yields_without_holding_activity_lock_and_uses_native_bytes() {
    let tracker = ApplicationRuntimeActivityTracker::default();
    let application = Uuid::now_v7();
    let started_at = OffsetDateTime::now_utc();
    let runtime_thread = std::thread::current().id();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = mpsc::channel();
    let sampling_tracker = tracker.clone();
    let lock_probe = tracker.clone();
    let task = tokio::spawn(async move {
        sampling_tracker
            .snapshot_sampled(application, started_at, move || {
                assert_ne!(std::thread::current().id(), runtime_thread);
                assert!(lock_probe.inner.try_lock().is_ok());
                let _ = started.send(());
                released.recv_timeout(Duration::from_secs(5)).unwrap();
                Some(65_536)
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    let activity = tracker.start(application, ApplicationActivityKind::ModelRequest);
    activity.finish(ApplicationActivityFinish::Failed);
    assert!(!task.is_finished());
    release.send(()).unwrap();
    let snapshot = task.await.unwrap();
    assert_eq!(snapshot.resources.process_rss_bytes, Some(65_536));
    assert_eq!(snapshot.active.total, 0);
    assert_eq!(snapshot.rolling_minute.failed, 1);
}

#[tokio::test]
async fn absent_or_panicking_sampler_keeps_optional_telemetry_without_losing_activity() {
    let tracker = ApplicationRuntimeActivityTracker::default();
    let application = Uuid::now_v7();
    let guard = tracker.start(application, ApplicationActivityKind::HttpRequest);
    let started = OffsetDateTime::now_utc();
    let absent = tracker
        .snapshot_sampled(application, started, || None)
        .await;
    assert_eq!(absent.resources.process_rss_bytes, None);
    assert_eq!(absent.active.http_requests, 1);
    let failed = tracker
        .snapshot_sampled(application, started, || panic!("sampler failed"))
        .await;
    assert_eq!(failed.resources.process_rss_bytes, None);
    assert_eq!(failed.active.http_requests, 1);
    drop(guard);
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_snapshot_does_not_hold_tracker_ownership() {
    let tracker = ApplicationRuntimeActivityTracker::default();
    let sampling_tracker = tracker.clone();
    let application = Uuid::now_v7();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = mpsc::channel();
    let (finished, finished_sampling) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        sampling_tracker
            .snapshot_sampled(application, OffsetDateTime::now_utc(), move || {
                let _ = started.send(());
                released.recv_timeout(Duration::from_secs(5)).unwrap();
                let _ = finished.send(());
                Some(1024)
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(tracker.start(application, ApplicationActivityKind::HttpRequest));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), finished_sampling)
        .await
        .unwrap()
        .unwrap();
    assert!(tracker.inner.try_lock().is_ok());
}
