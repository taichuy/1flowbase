use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn lease(
    deadline: Instant,
) -> (
    CallbackExecutionLease,
    watch::Sender<bool>,
    Arc<AtomicUsize>,
) {
    let (sender, lost) = watch::channel(false);
    let cleanups = Arc::new(AtomicUsize::new(0));
    let counted = cleanups.clone();
    let cleanup: Cleanup = Box::new(move || {
        Box::pin(async move {
            counted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });
    (
        CallbackExecutionLease {
            lost,
            heartbeat: tokio::spawn(std::future::pending()),
            deadline: Arc::new(std::sync::Mutex::new(deadline)),
            cleanup: Some(cleanup),
        },
        sender,
        cleanups,
    )
}

#[tokio::test]
async fn lost_prepared_lease_cancels_awaited_setup_before_side_effect() {
    struct ExecutionDrop(Arc<AtomicUsize>);
    impl Drop for ExecutionDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (lease, sender, cleanups) = lease(Instant::now() + std::time::Duration::from_secs(60));
    let effects = AtomicUsize::new(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let execution_drop = ExecutionDrop(drops.clone());
    sender.send_replace(true);
    assert!(lease
        .while_owned(Box::pin(async {
            let _execution_drop = execution_drop;
            effects.fetch_add(1, Ordering::SeqCst);
        }))
        .await
        .is_err());
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    lease.abort().await.unwrap();
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn deadline_cancels_blocked_setup_even_if_heartbeat_task_is_not_scheduled() {
    let (lease, _sender, cleanups) = lease(Instant::now() + std::time::Duration::from_millis(5));
    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(1),
        lease.while_owned(std::future::pending::<()>())
    )
    .await
    .unwrap()
    .is_err());
    lease.abort().await.unwrap();
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dropped_prepared_lease_runs_cleanup_once_and_settled_lease_disarms_it() {
    let (first, _sender, cleanups) = lease(Instant::now() + std::time::Duration::from_secs(60));
    drop(first);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while cleanups.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
    let (mut settled, _sender, cleanups) =
        lease(Instant::now() + std::time::Duration::from_secs(60));
    settled.disarm();
    drop(settled);
    tokio::task::yield_now().await;
    assert_eq!(cleanups.load(Ordering::SeqCst), 0);
}
