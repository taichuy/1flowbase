use super::*;

#[tokio::test]
async fn inspection_coalesces_same_source_without_holding_its_live_producer_or_retaining_lanes() {
    let key = key();
    let fake = Fake::new();
    let lease = add(key.clone(), &fake, member(1000), "r1").await;
    fake.started(1).await;
    let inspection = inspection(&key);
    let weak = Arc::downgrade(&inspection);
    let held = tokio::time::timeout(Duration::from_secs(1), inspection.clone().lock_owned())
        .await
        .expect("a live producer must not hold the separate inspection lane");
    assert!(
        super::super::broker::inspection(&key)
            .try_lock_owned()
            .is_err(),
        "a second same-source inspection must queue"
    );
    assert!(
        serial(&key).try_lock_owned().is_err(),
        "producer still owns only its separate lifetime permit"
    );
    let mut other = key.clone();
    other.record = "independent-record".into();
    assert!(
        super::super::broker::inspection(&other)
            .try_lock_owned()
            .is_ok(),
        "unrelated records inspect independently"
    );
    let queued = super::super::broker::inspection(&key);
    drop(held);
    let guard = tokio::time::timeout(Duration::from_secs(1), queued.lock_owned())
        .await
        .unwrap();
    drop(guard);
    drop(inspection);
    assert!(
        weak.upgrade().is_none(),
        "the inspection registry must not retain completed lane ownership"
    );
    lease.leave().await;
}

#[tokio::test]
async fn a_terminal_source_waits_for_explicit_membership_refresh_instead_of_retrying_permanently() {
    let key = key();
    let fake = Fake::new();
    let first = add(key.clone(), &fake, member(1000), "r1").await;
    let mut notices = first.entry.notices.subscribe();
    fake.started(1)
        .await
        .finish
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .send(false)
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let notice = notices.borrow_and_update().clone();
            if notice.error.is_some() {
                break;
            }
            notices.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(fake.starts.lock().unwrap().len(), 1);
    let second = add(key.clone(), &fake, member(1000), "r1").await;
    fake.started(2).await;
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    drop(first);
    drop(second);
    fake.until(|| fake.active.load(Ordering::SeqCst) == 0).await;
}

#[tokio::test]
async fn worker_failures_retry_with_one_producer_and_last_leave_cancels_backoff() {
    let key = key();
    let fake = Fake::new();
    let lease = add(key.clone(), &fake, member(1000), "r1").await;
    fake.started(1)
        .await
        .finish
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .send(true)
        .unwrap();
    fake.started(2)
        .await
        .finish
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .send(true)
        .unwrap();
    fake.started(3).await;
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    lease.leave().await;
    assert!(active(&key).is_none());
    assert_eq!(fake.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn two_principals_share_one_producer_and_slow_viewers_do_not_hold_source_credit() {
    let key = key();
    let fake = Fake::new();
    let first = add(key.clone(), &fake, member(5000), "r1").await;
    let second = add(key.clone(), &fake, member(5000), "r1").await;
    let _unread = first.entry.notices.subscribe();
    let mut latest = second.entry.notices.subscribe();
    fake.event(1, json!({"type":"sample","value":1})).await;
    fake.event(1, json!({"type":"sample","value":2})).await;
    assert_eq!(
        latest.borrow_and_update().event.as_ref().unwrap()["value"],
        2
    );
    assert_eq!(fake.starts.lock().unwrap().len(), 1);
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    drop(first);
    drop(second);
    fake.until(|| fake.active.load(Ordering::SeqCst) == 0).await;
    assert!(active(&key).is_none());
}

#[tokio::test]
async fn fastest_cadence_revision_and_owner_rotation_join_old_completion_before_restart() {
    let key = key();
    let fake = Fake::new();
    let first = add(key.clone(), &fake, member(5000), "r1").await;
    let owner = fake.started(1).await.actor;
    let second_member = member(1000);
    let second_actor = second_member.principal.actor().user_id;
    let second = add(key.clone(), &fake, second_member, "r1").await;
    assert_eq!(fake.started(2).await.interval, 1000);
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    drop(first);
    assert_ne!(owner, second_actor);
    assert_eq!(fake.started(3).await.actor, second_actor);
    second.entry.refresh_revision("r2");
    assert_eq!(fake.started(4).await.interval, 1000);
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    drop(second);
    fake.until(|| fake.active.load(Ordering::SeqCst) == 0).await;
}

#[tokio::test]
async fn denied_members_cannot_join_or_read_and_revoked_owner_rotates_without_losing_other_member()
{
    let key = key();
    let fake = Fake::new();
    let first_member = member(1000);
    let first_actor = first_member.principal.actor().user_id;
    let first = add(key.clone(), &fake, first_member, "r1").await;
    fake.started(1).await;
    let second_member = member(1000);
    let second_actor = second_member.principal.actor().user_id;
    let second = add(key.clone(), &fake, second_member, "r1").await;
    fake.denied.lock().unwrap().insert(first_actor);
    assert!(first.authorize().await.is_err());
    let denied = member(1000);
    fake.denied
        .lock()
        .unwrap()
        .insert(denied.principal.actor().user_id);
    assert!(
        join(key.clone(), Arc::new(fake.clone()), denied, "r1".into())
            .await
            .is_err()
    );
    // An arriving raw event forces live owner authority; it cannot be accepted under the old actor.
    let (consumed, received) = oneshot::channel();
    fake.started(1)
        .await
        .events
        .send(StreamEvent(
            runtime_core::runtime_backend::RuntimeManagedDuplexEvent {
                event: json!({"type":"sample"}),
                consumed,
            },
        ))
        .await
        .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(3), received)
        .await
        .unwrap()
        .is_err());
    assert_eq!(fake.started(2).await.actor, second_actor);
    assert!(second.authorize().await.is_ok());
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    drop(first);
    drop(second);
    fake.until(|| fake.active.load(Ordering::SeqCst) == 0).await;
}

#[tokio::test]
async fn generation_retirement_and_unary_fallback_use_the_same_serial_permit() {
    let old_key = key();
    let fake = Fake::new();
    // A raw unary fallback cannot overlap a collector arriving after its cache miss.
    let unary = serial(&old_key).lock_owned().await;
    let old = add(old_key.clone(), &fake, member(1000), "r1").await;
    assert_eq!(fake.starts.lock().unwrap().len(), 0);
    drop(unary);
    fake.started(1).await;
    fake.drain.send_replace(false);
    retire(Some(old_key.installation));
    fake.until(|| fake.stopping.load(Ordering::SeqCst) == 1)
        .await;
    let mut new_key = old_key.clone();
    new_key.generation = "generation-2".into();
    let new = add(new_key.clone(), &fake, member(1000), "r2").await;
    assert_eq!(
        fake.starts.lock().unwrap().len(),
        1,
        "new generation must await old worker acknowledgement"
    );
    assert!(old.authorize().await.is_err());
    fake.drain.send_replace(true);
    fake.started(2).await;
    assert_eq!(fake.max_active.load(Ordering::SeqCst), 1);
    drop(old);
    drop(new);
    fake.until(|| fake.active.load(Ordering::SeqCst) == 0).await;
}
