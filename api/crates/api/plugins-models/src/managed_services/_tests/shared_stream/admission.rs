use super::super::subscribers::concurrent_admissions;
use super::*;
use futures_util::StreamExt;

#[tokio::test]
async fn fast_source_can_publish_while_earlier_resolve_is_still_pending() {
    let broker = Broker::new();
    let fake = Fake::new();
    let mut slow = key();
    slow.record = "slow".into();
    let mut fast = key();
    fast.record = "fast".into();
    let (finish_slow, blocked) = oneshot::channel::<()>();
    let admissions = concurrent_admissions(
        [(slow.clone(), Some(blocked)), (fast.clone(), None)]
            .into_iter()
            .map(|(key, blocked)| {
                let fake = fake.clone();
                let broker = Arc::clone(&broker);
                async move {
                    if let Some(blocked) = blocked {
                        blocked.await.unwrap();
                    }
                    add(&broker, key, &fake, member(5000), "r1").await
                }
            }),
    );
    let mut admissions = Box::pin(admissions);
    let lease = tokio::time::timeout(Duration::from_secs(3), admissions.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.entry.key, fast);
    fake.event(1, json!({"type":"sample","record":"fast"}))
        .await;
    assert_eq!(
        lease.entry.notices.borrow().event.as_ref().unwrap()["record"],
        "fast"
    );
    assert!(broker.active(&slow).is_none());
    drop(admissions); // superseded `set`
    assert!(finish_slow.send(()).is_err());
    lease.leave().await;
    assert!(broker.active(&fast).is_none());
    assert_eq!(fake.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cold_join_replays_current_snapshot_but_live_entry_does_not_reseed() {
    let broker = Broker::new();
    let fake = Fake::new();
    let key = key();
    let saved = json!({"type":"sample","stream_id":"cached"});
    *fake.snapshot.lock().unwrap() = Some(("r1".into(), saved.clone()));
    let first = add(&broker, key.clone(), &fake, member(5000), "r1").await;
    let notice = first.entry.notices.borrow().clone();
    assert_eq!(notice.sequence, 1);
    assert_eq!(notice.revision, "r1");
    assert_eq!(notice.event, Some(saved));
    let live = json!({"type":"sample","stream_id":"live"});
    fake.event(1, live.clone()).await;
    let second = add(&broker, key.clone(), &fake, member(5000), "r1").await;
    assert_eq!(second.entry.notices.borrow().event, Some(live));
    assert_eq!(fake.reads.load(Ordering::SeqCst), 1);
    first.leave().await;
    second.leave().await;
    assert_eq!(fake.active.load(Ordering::SeqCst), 0);
    let different = add(&broker, key, &fake, member(5000), "r2").await;
    assert!(different.entry.notices.borrow().event.is_none());
    different.leave().await;
}

async fn cold_source_with_startup_status() -> (Arc<Fake>, Lease, Value, Value) {
    let broker = Broker::new();
    let fake = Fake::new();
    let seed = json!({"type":"sample","stream_id":"cached"});
    let status = json!({"type":"status","state":"connecting"});
    *fake.snapshot.lock().unwrap() = Some(("r1".into(), seed.clone()));
    *fake.startup_event.lock().unwrap() = Some(status.clone());
    let lease = add(&broker, key(), &fake, member(5000), "r1").await;
    let mut notices = lease.entry.notices.subscribe();
    let published = tokio::time::timeout(
        Duration::from_secs(3),
        notices.wait_for(|notice| notice.sequence == 2),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        published.event.as_ref(),
        Some(&status),
        "startup status already replaced the broker watch slot before viewer attachment"
    );
    (fake, lease, seed, status)
}

#[tokio::test]
async fn cold_viewer_delivers_seed_once_before_an_already_published_startup_status() {
    use super::super::subscribers::viewer;
    let (fake, lease, seed, status) = cold_source_with_startup_status().await;
    let (latest, changes) = watch::channel(Default::default());
    let record = lease.entry.key.record.clone();
    let mut viewer = viewer(lease, record.clone(), latest);
    let mut notice = changes.borrow()[&record].clone();
    assert_eq!(viewer.take_events(&mut notice), [Some(seed), Some(status)]);
    assert_eq!(
        viewer.take_events(&mut changes.borrow()[&record].clone())[0],
        None
    );
    viewer.stop().await;
    assert_eq!(fake.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn newer_ready_sample_supersedes_the_pending_cold_replay() {
    use super::super::subscribers::viewer;
    for coalesce_ready in [false, true] {
        let (fake, lease, _, status) = cold_source_with_startup_status().await;
        let ready = json!({"type":"sample","stream_id":"live"});
        fake.event(1, ready.clone()).await;
        let expected_event = if coalesce_ready {
            // A later status must not resurrect the seed even if the ready event
            // itself was overwritten in the latest-only broker watch slot.
            fake.event(1, status.clone()).await;
            status
        } else {
            ready
        };
        let (latest, changes) = watch::channel(Default::default());
        let record = lease.entry.key.record.clone();
        let mut viewer = viewer(lease, record.clone(), latest);
        assert_eq!(
            viewer.take_events(&mut changes.borrow()[&record].clone()),
            [None, Some(expected_event)]
        );
        viewer.stop().await;
    }
}
