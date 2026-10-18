use super::*;
use serde_json::json;
use std::time::{Duration, Instant};

fn limits() -> EventBusLimits {
    EventBusLimits {
        max_items: 4,
        max_json_bytes: 128,
        max_waiters: 4,
        max_topics: 4,
        max_consumers: 4,
        max_topic_name_bytes: 64,
    }
}
fn configured(policy: EventBusLimits) -> MemoryEventBus {
    let bus = MemoryEventBus::new();
    bus.configure_budget(policy).unwrap();
    bus
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}
async fn send(bus: &MemoryEventBus, topic: &str, value: serde_json::Value) {
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    bus.reserve(topic, bytes, deadline())
        .await
        .unwrap()
        .send(value)
        .unwrap();
}

#[tokio::test]
async fn no_consumer_or_budget_never_allocates_and_configuration_is_shared() {
    let bus = MemoryEventBus::new();
    assert!(matches!(
        bus.open_consumer("a"),
        Err(EventBusError::Unconfigured)
    ));
    assert!(matches!(
        bus.reserve("a", 4, deadline()).await,
        Err(EventBusError::Unconfigured)
    ));
    bus.configure_budget(limits()).unwrap();
    bus.clone().configure_budget(limits()).unwrap();
    assert!(matches!(
        bus.configure_budget(EventBusLimits {
            max_items: 5,
            ..limits()
        }),
        Err(EventBusError::BudgetAlreadyConfigured)
    ));
    for _ in 0..100 {
        assert!(matches!(
            bus.reserve("missing", 4, deadline()).await,
            Err(EventBusError::NoConsumer)
        ));
    }
    assert_eq!(
        bus.event_bus_snapshot(),
        EventBusSnapshot {
            limits: Some(limits()),
            admission_rejected: 101,
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn shared_item_and_byte_budget_includes_in_flight_and_releases_on_drop() {
    for policy in [
        EventBusLimits {
            max_items: 1,
            ..limits()
        },
        EventBusLimits {
            max_json_bytes: 4,
            ..limits()
        },
    ] {
        let bus = configured(policy);
        let a = bus.open_consumer("a").unwrap();
        let _b = bus.open_consumer("b").unwrap();
        send(&bus, "a", json!(null)).await;
        let delivery = a.recv().await.unwrap().unwrap();
        assert_eq!(bus.event_bus_snapshot().in_flight, 1);
        assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 4);
        let mut waiting = Box::pin(bus.reserve("b", 4, deadline()));
        tokio::select! {biased; _ = &mut waiting => panic!("in-flight delivery must retain capacity"), _ = tokio::task::yield_now() => {}}
        assert_eq!(bus.event_bus_snapshot().waiters, 1);
        drop(delivery);
        // The very same reservation must become admissible after guard release.
        waiting.await.unwrap().send(json!(null)).unwrap();
        assert_eq!(bus.event_bus_snapshot().waiters, 0);
        assert!(matches!(
            bus.reserve("a", 129, deadline()).await,
            Err(EventBusError::ResourceExhausted(_))
        ));
    }
}

#[tokio::test]
async fn reservation_cancel_deadline_and_waiter_budget_are_shared() {
    let bus = configured(EventBusLimits {
        max_items: 1,
        max_waiters: 1,
        ..limits()
    });
    let _a = bus.open_consumer("a").unwrap();
    let _b = bus.open_consumer("b").unwrap();
    let permit = bus.reserve("a", 4, deadline()).await.unwrap();
    let mut waiting = Box::pin(bus.reserve("b", 4, deadline()));
    tokio::select! { biased; _ = &mut waiting => panic!("must wait"), _ = tokio::task::yield_now() => {} }
    assert_eq!(bus.event_bus_snapshot().waiters, 1);
    assert!(matches!(
        bus.reserve("a", 4, deadline()).await,
        Err(EventBusError::ResourceExhausted("waiters"))
    ));
    drop(waiting);
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
    assert!(matches!(
        bus.reserve("b", 4, Instant::now() + Duration::from_millis(1))
            .await,
        Err(EventBusError::DeadlineElapsed)
    ));
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
    drop(permit);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
    let b = bus.open_consumer("b").unwrap();
    let mut receiving = Box::pin(b.recv());
    tokio::select! { biased; _ = &mut receiving => panic!("must wait"), _ = tokio::task::yield_now() => {} }
    assert_eq!(bus.event_bus_snapshot().waiters, 1);
    drop(bus.reserve("a", 4, deadline()).await.unwrap());
    drop(receiving);
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
}

#[tokio::test]
async fn close_drains_and_invalidates_permits_and_wakes_waiters() {
    let bus = configured(EventBusLimits {
        max_items: 2,
        ..limits()
    });
    let c = bus.open_consumer("a").unwrap();
    send(&bus, "a", json!(1)).await;
    let old = bus.reserve("a", 4, deadline()).await.unwrap();
    let mut waiting = Box::pin(bus.reserve("a", 4, deadline()));
    tokio::select! { biased; _ = &mut waiting => panic!("must wait"), _ = tokio::task::yield_now() => {} }
    c.close();
    assert!(matches!(waiting.await, Err(EventBusError::Closed)));
    assert!(matches!(old.send(json!(null)), Err(EventBusError::Closed)));
    assert_eq!(c.recv().await.unwrap().unwrap().body(), &json!(1));
    assert!(c.recv().await.unwrap().is_none());
    drop(c);
    assert_eq!(bus.event_bus_snapshot().topics, 0);
    assert_eq!(bus.event_bus_snapshot().abnormal_terminations, 0);
}

#[tokio::test]
async fn last_drop_invalidates_old_generation_and_preserves_in_flight_charge() {
    let bus = configured(limits());
    let c = bus.open_consumer("a").unwrap();
    send(&bus, "a", json!(null)).await;
    let delivery = c.recv().await.unwrap().unwrap();
    let old = bus.reserve("a", 4, deadline()).await.unwrap();
    drop(c);
    assert_eq!(bus.event_bus_snapshot().abnormal_terminations, 1);
    assert_eq!(bus.event_bus_snapshot().reserved, 1);
    assert_eq!(bus.event_bus_snapshot().in_flight, 1);
    let next = bus.open_consumer("a").unwrap();
    assert!(matches!(old.send(json!(null)), Err(EventBusError::Closed)));
    send(&bus, "a", json!(7)).await;
    assert_eq!(next.recv().await.unwrap().unwrap().body(), &json!(7));
    drop(delivery);
    next.close();
    drop(next);
    assert_eq!(bus.event_bus_snapshot().topics, 0);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn competing_consumers_dequeue_fifo_and_registration_is_atomic() {
    let bus = configured(limits());
    let a = bus.open_consumer("a").unwrap();
    let b = bus.open_consumer("a").unwrap();
    for i in 0..4 {
        send(&bus, "a", json!(i)).await;
    }
    for i in 0..4 {
        let c = if i % 2 == 0 { &a } else { &b };
        assert_eq!(c.recv().await.unwrap().unwrap().body(), &json!(i));
    }
    drop(a);
    send(&bus, "a", json!(9)).await;
    assert_eq!(b.recv().await.unwrap().unwrap().body(), &json!(9));
    b.close();
    drop(b);
    for _ in 0..100 {
        let left = bus.clone();
        let right = bus.clone();
        let (a, b) = tokio::join!(
            tokio::spawn(async move { left.open_consumer("race").unwrap() }),
            tokio::spawn(async move { right.open_consumer("race").unwrap() })
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        drop(a);
        send(&bus, "race", json!(1)).await;
        assert_eq!(b.recv().await.unwrap().unwrap().body(), &json!(1));
        b.close();
        drop(b);
    }
    assert_eq!(bus.event_bus_snapshot().topics, 0);
    assert_eq!(bus.event_bus_snapshot().topic_name_bytes, 0);
    let state = bus.inner.lock();
    assert_eq!(state.topics.capacity(), 0);
    assert_eq!(state.entries.capacity(), 0);
}

#[tokio::test]
async fn directory_consumer_and_name_byte_limits_are_explicit() {
    for policy in [
        EventBusLimits {
            max_topics: 1,
            ..limits()
        },
        EventBusLimits {
            max_consumers: 1,
            ..limits()
        },
        EventBusLimits {
            max_topic_name_bytes: 1,
            ..limits()
        },
    ] {
        let bus = configured(policy);
        let a = bus.open_consumer("a").unwrap();
        assert!(matches!(
            bus.open_consumer("b"),
            Err(EventBusError::ResourceExhausted(_))
        ));
        a.close();
        drop(a);
        let b = bus.open_consumer("b").unwrap();
        b.close();
        drop(b);
        assert_eq!(bus.event_bus_snapshot().topics, 0);
    }
    let bus = configured(EventBusLimits {
        max_consumers: 1,
        ..limits()
    });
    let _a = bus.open_consumer("a").unwrap();
    assert!(matches!(
        bus.open_consumer("a"),
        Err(EventBusError::ResourceExhausted("consumers"))
    ));
}

#[tokio::test]
async fn full_json_roundtrip_and_oversized_send_never_partially_delivers() {
    let body = json!({"nul\u{0}key":[u64::MAX,"原文\u{0}".repeat(200_000),null]});
    let bytes = serde_json::to_vec(&body).unwrap().len() as u64;
    assert!(bytes > 1024 * 1024);
    let bus = configured(EventBusLimits {
        max_json_bytes: bytes,
        ..limits()
    });
    let c = bus.open_consumer("large").unwrap();
    bus.reserve("large", bytes, deadline())
        .await
        .unwrap()
        .send(body.clone())
        .unwrap();
    let d = c.recv().await.unwrap().unwrap();
    assert_eq!(d.body(), &body);
    drop(d);
    assert!(matches!(
        bus.reserve("large", 1, deadline())
            .await
            .unwrap()
            .send(json!(null)),
        Err(EventBusError::BodyExceedsReservation)
    ));
    assert_eq!(bus.event_bus_snapshot().buffered, 0);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
}

#[tokio::test]
async fn inspection_observes_buffered_inflight_and_resources_without_consuming() {
    let bus = configured(limits());
    let c = bus.open_consumer("a").unwrap();
    send(&bus, "a", json!(1)).await;
    send(&bus, "a", json!(2)).await;
    let d = c.recv().await.unwrap().unwrap();
    for _ in 0..2 {
        let entries: Vec<_> = bus
            .list_ephemeral_entries()
            .await
            .unwrap()
            .into_iter()
            .filter(|entry| entry.entry_kind == "event")
            .collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].status, "in-flight");
        assert_eq!(entries[1].status, "buffered");
        let reveal = bus
            .reveal_ephemeral_entry(&entries[0].entry_ref, EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reveal.value, Some(json!(1)));
        assert_eq!(bus.event_bus_snapshot().in_flight, 1);
        assert_eq!(bus.event_bus_snapshot().buffered, 1);
    }
    assert_eq!(d.body(), &json!(1));
    drop(d);
    assert_eq!(c.recv().await.unwrap().unwrap().body(), &json!(2));
}

#[tokio::test]
async fn closed_live_permit_retains_budget_and_retired_directory_until_failed_send() {
    let bus = configured(EventBusLimits {
        max_items: 1,
        max_topics: 1,
        ..limits()
    });
    let c = bus.open_consumer("a").unwrap();
    let permit = bus.reserve("a", 64, deadline()).await.unwrap();
    c.close();
    assert!(c.recv().await.unwrap().is_none());
    assert_eq!(bus.event_bus_snapshot().reserved, 1);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 64);
    drop(c);
    assert_eq!(bus.event_bus_snapshot().retired_topics, 1);
    assert!(matches!(
        bus.open_consumer("a"),
        Err(EventBusError::ResourceExhausted("topics"))
    ));
    assert!(matches!(
        permit.send(json!(null)),
        Err(EventBusError::Closed)
    ));
    assert_eq!(bus.event_bus_snapshot().topics, 0);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
    let _next = bus.open_consumer("a").unwrap();
}

#[tokio::test]
async fn empty_receives_share_waiter_limit_and_close_wakes_all_consumers() {
    let bus = configured(EventBusLimits {
        max_waiters: 1,
        ..limits()
    });
    let a = bus.open_consumer("a").unwrap();
    let b = bus.open_consumer("b").unwrap();
    let mut waiting = Box::pin(a.recv());
    tokio::select! {biased; _ = &mut waiting => panic!("must wait"), _ = tokio::task::yield_now() => {}}
    assert!(matches!(
        b.recv().await,
        Err(EventBusError::ResourceExhausted("waiters"))
    ));
    a.close();
    assert!(waiting.await.unwrap().is_none());
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
    let same = bus.open_consumer("b").unwrap();
    let mut left = Box::pin(b.recv());
    tokio::select! {biased; _ = &mut left => panic!("must wait"), _ = tokio::task::yield_now() => {}}
    same.close();
    assert!(left.await.unwrap().is_none());
    assert!(same.recv().await.unwrap().is_none());
}

#[tokio::test]
async fn resource_inspection_is_available_without_body_and_includes_waiters_and_close() {
    let bus = MemoryEventBus::new();
    assert!(bus.list_ephemeral_entries().await.unwrap().is_empty());
    bus.configure_budget(EventBusLimits {
        max_items: 1,
        ..limits()
    })
    .unwrap();
    let c = bus.open_consumer("a").unwrap();
    let permit = bus.reserve("a", 32, deadline()).await.unwrap();
    let mut waiting = Box::pin(bus.reserve("a", 4, deadline()));
    tokio::select! {biased; _ = &mut waiting => panic!("must wait"), _ = tokio::task::yield_now() => {}}
    let entries = bus.list_ephemeral_entries().await.unwrap();
    let resource = entries
        .iter()
        .find(|e| e.entry_kind == "event_bus_resources")
        .unwrap();
    assert!(!resource.sensitive);
    assert_eq!(resource.value_size_bytes, 0);
    assert_eq!(resource.metadata["waiters"], json!(1));
    assert_eq!(resource.metadata["reserved"], json!(1));
    assert_eq!(resource.metadata["charged_json_bytes"], json!(32));
    assert!(entries.iter().any(|e| e.status == "reserved"));
    c.close();
    assert!(matches!(waiting.await, Err(EventBusError::Closed)));
    let resource = bus
        .list_ephemeral_entries()
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.entry_kind == "event_bus_resources")
        .unwrap();
    assert_eq!(resource.metadata["closed_topics"], json!(1));
    assert_eq!(resource.metadata["waiters"], json!(0));
    drop(permit);
    drop(c);
    let resource = bus.list_ephemeral_entries().await.unwrap().pop().unwrap();
    assert_eq!(resource.metadata["topics"], json!(0));
    assert_eq!(resource.metadata["charged_json_bytes"], json!(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_slow_consumer_backpressure_preserves_every_fifo_delivery() {
    let bus = configured(EventBusLimits {
        max_items: 1,
        ..limits()
    });
    let consumer = bus.open_consumer("a").unwrap();
    let producer = bus.clone();
    let sending = tokio::spawn(async move {
        for i in 0..128 {
            send(&producer, "a", json!(i)).await;
        }
    });
    for i in 0..128 {
        let delivery = consumer.recv().await.unwrap().unwrap();
        tokio::task::yield_now().await;
        assert_eq!(delivery.body(), &json!(i));
        assert_eq!(bus.event_bus_snapshot().in_flight, 1);
        drop(delivery);
    }
    sending.await.unwrap();
    consumer.close();
    drop(consumer);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
    assert_eq!(bus.event_bus_snapshot().topics, 0);
}

#[tokio::test]
async fn last_consumer_drop_wakes_reservation_without_retargeting_reopened_topic() {
    let bus = configured(EventBusLimits {
        max_items: 1,
        ..limits()
    });
    let old = bus.open_consumer("a").unwrap();
    let permit = bus.reserve("a", 4, deadline()).await.unwrap();
    let mut waiting = Box::pin(bus.reserve("a", 4, deadline()));
    tokio::select! {biased; _ = &mut waiting => panic!("must wait"), _ = tokio::task::yield_now() => {}}
    drop(old);
    let fresh = bus.open_consumer("a").unwrap();
    drop(permit);
    assert!(matches!(waiting.await, Err(EventBusError::Closed)));
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
    send(&bus, "a", json!(7)).await;
    assert_eq!(fresh.recv().await.unwrap().unwrap().body(), &json!(7));
    fresh.close();
    drop(fresh);
    assert_eq!(bus.event_bus_snapshot().topics, 0);
}

#[tokio::test]
async fn admission_rejection_counts_failed_reserve_and_send_calls_only() {
    let bus = configured(EventBusLimits {
        max_items: 1,
        max_waiters: 0,
        ..limits()
    });
    assert!(
        bus.configure_budget(EventBusLimits {
            max_items: 8,
            ..limits()
        })
        .is_err()
    );
    assert!(bus.open_consumer(&"x".repeat(65)).is_err());
    assert_eq!(bus.event_bus_snapshot().admission_rejected, 0);
    assert!(matches!(
        bus.reserve("missing", 4, deadline()).await,
        Err(EventBusError::NoConsumer)
    ));
    let consumer = bus.open_consumer("a").unwrap();
    assert!(matches!(
        bus.reserve("a", 129, deadline()).await,
        Err(EventBusError::ResourceExhausted(_))
    ));
    assert!(matches!(
        bus.reserve("a", 4, Instant::now()).await,
        Err(EventBusError::DeadlineElapsed)
    ));
    assert!(matches!(
        bus.reserve("a", 1, deadline())
            .await
            .unwrap()
            .send(json!(null)),
        Err(EventBusError::BodyExceedsReservation)
    ));
    let permit = bus.reserve("a", 4, deadline()).await.unwrap();
    assert!(matches!(
        bus.reserve("a", 4, deadline()).await,
        Err(EventBusError::ResourceExhausted("waiters"))
    ));
    consumer.close();
    assert!(matches!(
        permit.send(json!(null)),
        Err(EventBusError::Closed)
    ));
    assert!(matches!(
        bus.reserve("a", 4, deadline()).await,
        Err(EventBusError::Closed)
    ));
    assert!(consumer.recv().await.unwrap().is_none());
    drop(consumer);
    let snapshot = bus.event_bus_snapshot();
    assert_eq!(snapshot.admission_rejected, 7);
    assert_eq!(snapshot.accepted, 0);
    assert_eq!(snapshot.received, 0);
    assert_eq!(snapshot.charged_json_bytes, 0);
}

#[tokio::test]
async fn progress_counts_guard_release_and_exact_buffer_discard_without_business_ack() {
    let bus = configured(limits());
    let consumer = bus.open_consumer("a").unwrap();
    for id in 0..4 {
        send(&bus, "a", json!(id)).await;
    }
    let completed = consumer.recv().await.unwrap().unwrap();
    completed.complete();
    let failed_processing = consumer.recv().await.unwrap().unwrap();
    // A processing failure still releases the guard; this metric does not assert success.
    assert_eq!(failed_processing.body(), &json!(1));
    let processing_result: Result<(), &str> = Err("business handler rejected the event");
    assert!(processing_result.is_err());
    drop(failed_processing);
    let snapshot = bus.event_bus_snapshot();
    assert_eq!(
        (
            snapshot.accepted,
            snapshot.received,
            snapshot.processing_released
        ),
        (4, 2, 2)
    );
    assert_eq!(snapshot.discarded_buffered, 0);
    drop(consumer);
    let snapshot = bus.event_bus_snapshot();
    assert_eq!(snapshot.discarded_buffered, 2);
    assert_eq!(snapshot.abnormal_terminations, 1);
    assert_eq!(
        (
            snapshot.accepted,
            snapshot.received,
            snapshot.processing_released
        ),
        (4, 2, 2)
    );
    assert_eq!(snapshot.topics, 0);
    assert_eq!(snapshot.charged_json_bytes, 0);
    let resource = bus.list_ephemeral_entries().await.unwrap().pop().unwrap();
    assert_eq!(resource.metadata["accepted"], json!(4));
    assert_eq!(resource.metadata["received"], json!(2));
    assert_eq!(resource.metadata["processing_released"], json!(2));
    assert_eq!(resource.metadata["discarded_buffered"], json!(2));
}

#[tokio::test]
async fn current_wait_and_backlog_ages_are_observable_and_reset_after_release() {
    let bus = configured(EventBusLimits {
        max_items: 1,
        ..limits()
    });
    let consumer = bus.open_consumer("a").unwrap();
    let permit = bus.reserve("a", 4, deadline()).await.unwrap();
    let mut waiting = Box::pin(bus.reserve("a", 4, deadline()));
    tokio::select! {biased; _ = &mut waiting => panic!("must wait"), _ = tokio::task::yield_now() => {}}
    tokio::time::sleep(Duration::from_millis(2)).await;
    let resource = bus
        .list_ephemeral_entries()
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.entry_kind == "event_bus_resources")
        .unwrap();
    assert!(resource.metadata["oldest_waiter_age_ms"].as_u64().unwrap() >= 1);
    assert_eq!(resource.metadata["waiters"], json!(1));
    drop(waiting);
    assert_eq!(bus.event_bus_snapshot().oldest_waiter_age_ms, 0);
    assert_eq!(bus.event_bus_snapshot().admission_rejected, 0);
    assert_eq!(bus.inner.lock().waiters.capacity(), 0);
    permit.send(json!(null)).unwrap();
    tokio::time::sleep(Duration::from_millis(2)).await;
    let resource = bus
        .list_ephemeral_entries()
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.entry_kind == "event_bus_resources")
        .unwrap();
    assert!(
        resource.metadata["oldest_buffered_age_ms"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let delivery = consumer.recv().await.unwrap().unwrap();
    assert_eq!(bus.event_bus_snapshot().oldest_buffered_age_ms, 0);
    tokio::time::sleep(Duration::from_millis(2)).await;
    let resource = bus
        .list_ephemeral_entries()
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.entry_kind == "event_bus_resources")
        .unwrap();
    assert!(
        resource.metadata["oldest_in_flight_age_ms"]
            .as_u64()
            .unwrap()
            >= 1
    );
    delivery.complete();
    consumer.close();
    drop(consumer);
    let snapshot = bus.event_bus_snapshot();
    assert_eq!(
        (
            snapshot.oldest_waiter_age_ms,
            snapshot.oldest_buffered_age_ms,
            snapshot.oldest_in_flight_age_ms
        ),
        (0, 0, 0)
    );
    assert_eq!(
        (
            snapshot.accepted,
            snapshot.received,
            snapshot.processing_released
        ),
        (1, 1, 1)
    );
    assert_eq!(snapshot.admission_rejected, 0);
}
