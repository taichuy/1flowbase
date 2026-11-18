use super::*;
use domain::event_center::{EventSourceQueueSnapshot, EventSourceScope};

#[test]
fn native_identity_causation_and_unknown_queue_counts_round_trip() {
    let event = EventSourceObservation {
        attributes: Default::default(),

        original_event_id: Some("non-uuid/native-id".into()),
        producer_id: Some("native-producer".into()),
        occurred_at_unix_ms: 42,
        workflow_ancestor_ids: vec![Uuid::now_v7()],
        fact: EventSourceFact::Consumed {
            subscriber_id: "frozen.subscriber".into(),
            consumer_id: "handler.native".into(),
        },
    };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["fact"]["phase"], "consumed");
    assert_eq!(
        serde_json::from_value::<EventSourceObservation>(value).unwrap(),
        event
    );
    let queue = EventSourceQueueSnapshot {
        queue_id: "outbox".into(),
        subscriber_id: "frozen.subscriber".into(),
        consumer_ids: vec![],
        buffered: None,
        in_flight: None,
        accepted: None,
        dequeued: None,
        dropped: None,
        observed_at_unix_ms: 42,
    };
    assert!(serde_json::to_value(queue).unwrap()["dequeued"].is_null());
    assert_ne!(
        EventSourceScope::System,
        EventSourceScope::Workspace {
            workspace_id: domain::SYSTEM_SCOPE_ID
        }
    );
    assert_ne!(EventSourceScope::System, EventSourceScope::Unbound);
}
