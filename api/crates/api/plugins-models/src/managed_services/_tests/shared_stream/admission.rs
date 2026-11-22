use super::super::subscribers::concurrent_admissions;
use super::*;
use futures_util::StreamExt;

#[tokio::test]
async fn fast_source_can_publish_while_earlier_resolve_is_still_pending() {
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
                async move {
                    if let Some(blocked) = blocked {
                        blocked.await.unwrap();
                    }
                    add(key, &fake, member(5000), "r1").await
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
    assert!(active(&slow).is_none());
    drop(admissions); // superseded `set`
    assert!(finish_slow.send(()).is_err());
    lease.leave().await;
    assert!(active(&fast).is_none());
    assert_eq!(fake.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cold_join_replays_current_snapshot_but_live_entry_does_not_reseed() {
    let fake = Fake::new();
    let key = key();
    let saved = json!({"type":"sample","stream_id":"cached"});
    *fake.snapshot.lock().unwrap() = Some(("r1".into(), saved.clone()));
    let first = add(key.clone(), &fake, member(5000), "r1").await;
    let notice = first.entry.notices.borrow().clone();
    assert_eq!(notice.sequence, 1);
    assert_eq!(notice.revision, "r1");
    assert_eq!(notice.event, Some(saved));
    let live = json!({"type":"sample","stream_id":"live"});
    fake.event(1, live.clone()).await;
    let second = add(key.clone(), &fake, member(5000), "r1").await;
    assert_eq!(second.entry.notices.borrow().event, Some(live));
    assert_eq!(fake.reads.load(Ordering::SeqCst), 1);
    first.leave().await;
    second.leave().await;
    assert_eq!(fake.active.load(Ordering::SeqCst), 0);
    let different = add(key, &fake, member(5000), "r2").await;
    assert!(different.entry.notices.borrow().event.is_none());
    different.leave().await;
}
