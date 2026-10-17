use super::*;
use serde_json::json;

#[tokio::test]
async fn drained_topics_release_containers_without_losing_other_topics_or_sequence() {
    let bus = MemoryEventBus::new();
    for id in 0..256 {
        bus.publish("burst", json!(id)).await.unwrap();
    }
    bus.publish("retained", json!("pending")).await.unwrap();
    for id in 0..256 {
        assert_eq!(bus.poll("burst").await.unwrap(), Some(json!(id)));
    }
    {
        let state = bus.inner.lock().await;
        assert!(!state.topics.contains_key("burst"));
        assert_eq!(state.topics["retained"].len(), 1);
    }
    let entries = bus.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].entry_ref, "257");
    assert!(
        bus.reveal_ephemeral_entry("1", EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(bus.poll("retained").await.unwrap(), Some(json!("pending")));
    {
        let state = bus.inner.lock().await;
        assert!(state.topics.is_empty());
        assert_eq!(state.topics.capacity(), 0);
    }
    assert_eq!(bus.poll("missing").await.unwrap(), None);
    bus.publish("burst", json!("new")).await.unwrap();
    let entries = bus.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries[0].entry_ref, "258");
    let revealed = bus
        .reveal_ephemeral_entry("258", EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.value, Some(json!("new")));
    assert_eq!(bus.poll("burst").await.unwrap(), Some(json!("new")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_publish_and_final_poll_preserve_every_event() {
    let bus = MemoryEventBus::new();
    for id in 0..256 {
        bus.publish("race", json!([id, "first"])).await.unwrap();
        let publisher = bus.clone();
        let consumer = bus.clone();
        let (published, polled) = tokio::join!(
            tokio::spawn(async move { publisher.publish("race", json!([id, "next"])).await }),
            tokio::spawn(async move { consumer.poll("race").await }),
        );
        published.unwrap().unwrap();
        assert_eq!(polled.unwrap().unwrap(), Some(json!([id, "first"])));
        assert_eq!(bus.poll("race").await.unwrap(), Some(json!([id, "next"])));
        assert_eq!(bus.poll("race").await.unwrap(), None);
    }
    let state = bus.inner.lock().await;
    assert!(state.topics.is_empty());
    assert_eq!(state.next_sequence, 512);
}
