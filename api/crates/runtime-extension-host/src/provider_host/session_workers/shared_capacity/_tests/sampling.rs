use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

const PID: u32 = 123;
const RECHECK: Duration = Duration::from_millis(1);

fn available() -> FrameworkResult<u64> {
    Ok(u64::MAX)
}

#[tokio::test(flavor = "current_thread")]
async fn admission_samples_off_runtime_and_without_accounting_lock() {
    let capacity = SharedWorkerCapacity::default();
    let runtime_thread = std::thread::current().id();
    let probe = capacity.clone();
    let available_probe = capacity.clone();
    let permit = capacity
        .acquire_sampled(
            PID,
            16,
            tokio::time::Instant::now() + Duration::from_secs(5),
            move || {
                assert_ne!(std::thread::current().id(), runtime_thread);
                assert!(available_probe.workers.try_lock().is_ok());
                available()
            },
            move |_| {
                assert_ne!(std::thread::current().id(), runtime_thread);
                assert!(probe.workers.try_lock().is_ok());
                Some(1024)
            },
            RECHECK,
        )
        .await
        .unwrap();
    assert_eq!(capacity.workers.lock().unwrap()[&PID].active.len(), 1);
    drop(permit);
}

#[tokio::test(flavor = "current_thread")]
async fn missing_target_sample_fails_closed_without_a_reservation() {
    let capacity = SharedWorkerCapacity::default();
    let error = capacity
        .acquire_sampled(
            PID,
            16,
            tokio::time::Instant::now() + Duration::from_secs(5),
            available,
            |_| None,
            RECHECK,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("RSS is unavailable"));
    assert!(capacity.workers.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_during_blocking_sample_cannot_create_a_reservation() {
    let capacity = SharedWorkerCapacity::default();
    let acquiring = capacity.clone();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let started = Mutex::new(Some(started));
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let (finished, finished_sampling) = tokio::sync::oneshot::channel();
    let finished = Mutex::new(Some(finished));
    let task = tokio::spawn(async move {
        acquiring
            .acquire_sampled(
                PID,
                16,
                tokio::time::Instant::now() + Duration::from_secs(5),
                available,
                move |_| {
                    if let Some(started) = started.lock().unwrap().take() {
                        let _ = started.send(());
                    }
                    released
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    if let Some(finished) = finished.lock().unwrap().take() {
                        let _ = finished.send(());
                    }
                    Some(1024)
                },
                RECHECK,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), finished_sampling)
        .await
        .unwrap()
        .unwrap();
    assert!(capacity.workers.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn drop_keeps_reservation_until_unlocked_background_sample_and_notifies_release() {
    let capacity = SharedWorkerCapacity::default();
    let probe = capacity.clone();
    let samples = Arc::new(AtomicUsize::new(0));
    let sampling = samples.clone();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let started = Mutex::new(Some(started));
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let permit = capacity
        .acquire_sampled(
            PID,
            16,
            tokio::time::Instant::now() + Duration::from_secs(5),
            available,
            move |_| {
                assert!(probe.workers.try_lock().is_ok());
                if sampling.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Some(100);
                }
                if let Some(started) = started.lock().unwrap().take() {
                    let _ = started.send(());
                }
                released
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                Some(300)
            },
            RECHECK,
        )
        .await
        .unwrap();
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(capacity.workers.lock().unwrap()[&PID].active.len(), 1);
    let changed = capacity.changed.notified();
    tokio::pin!(changed);
    changed.as_mut().enable();
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), changed)
        .await
        .unwrap();
    let workers = capacity.workers.lock().unwrap();
    assert!(workers[&PID].active.is_empty());
    assert_eq!(workers[&PID].observed_increment, 200);
    assert_eq!(samples.load(Ordering::SeqCst), 2);
}

#[test]
fn drop_without_runtime_retires_accounting_without_sampling() {
    let capacity = SharedWorkerCapacity::default();
    let samples = Arc::new(AtomicUsize::new(0));
    let sampling = samples.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let permit = runtime
        .block_on(capacity.acquire_sampled(
            PID,
            16,
            tokio::time::Instant::now() + Duration::from_secs(5),
            available,
            move |_| {
                sampling.fetch_add(1, Ordering::SeqCst);
                Some(1024)
            },
            RECHECK,
        ))
        .unwrap();
    drop(runtime);
    drop(permit);
    assert!(capacity.workers.lock().unwrap()[&PID].active.is_empty());
    assert_eq!(samples.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_concurrent_worker_is_kept_and_its_reservation_gets_no_rss_credit() {
    let capacity = SharedWorkerCapacity::default();
    let acquiring = capacity.clone();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let started = Mutex::new(Some(started));
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let task = tokio::spawn(async move {
        acquiring
            .acquire_sampled(
                PID,
                16,
                tokio::time::Instant::now() + Duration::from_secs(5),
                // Enough for one call, never two complete reservations.
                || {
                    Ok((MULTIPLEX_OUTPUT_BUDGET_BYTES as u64) * 4
                        + MULTIPLEX_CALL_EVENT_BUDGET_BYTES as u64
                        + 32)
                },
                move |_| {
                    if let Some(started) = started.lock().unwrap().take() {
                        let _ = started.send(());
                        released
                            .lock()
                            .unwrap()
                            .recv_timeout(Duration::from_secs(5))
                            .unwrap();
                    }
                    Some(1024)
                },
                RECHECK,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .unwrap()
        .unwrap();
    capacity.workers.lock().unwrap().insert(
        PID + 1,
        WorkerMemory {
            next_ticket: 1,
            observed_increment: 0,
            active: HashMap::from([(
                1,
                Reservation {
                    bytes: MULTIPLEX_CALL_EVENT_BUDGET_BYTES as u64 + 32,
                    rss_at_admission: 1024,
                },
            )]),
        },
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if capacity.workers.lock().unwrap().contains_key(&PID) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !task.is_finished(),
        "unmeasured reservation must prevent admission"
    );
    {
        let workers = capacity.workers.lock().unwrap();
        assert_eq!(workers[&(PID + 1)].active.len(), 1);
        assert!(workers[&PID].active.is_empty());
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn panicking_release_sampler_still_retires_and_notifies_via_cleanup_owner() {
    let capacity = SharedWorkerCapacity::default();
    let samples = AtomicUsize::new(0);
    let permit = capacity
        .acquire_sampled(
            PID,
            16,
            tokio::time::Instant::now() + Duration::from_secs(5),
            available,
            move |_| {
                if samples.fetch_add(1, Ordering::SeqCst) == 0 {
                    Some(1024)
                } else {
                    panic!("release sampler failed")
                }
            },
            RECHECK,
        )
        .await
        .unwrap();
    let changed = capacity.changed.notified();
    tokio::pin!(changed);
    changed.as_mut().enable();
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), changed)
        .await
        .unwrap();
    assert!(capacity.workers.lock().unwrap()[&PID].active.is_empty());
}
