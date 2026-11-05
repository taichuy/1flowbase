use super::*;
use control_plane_contracts::ports::EventBusLimits;
use domain::event_center::EventTriggerPhase;

#[test]
fn expired_entries_are_filtered_without_snapshot_mutation_and_reclaimed_on_next_write() {
    let workspace_id = Uuid::now_v7();
    let policy = EventCenterRuntimePolicy {
        bus_limits: EventBusLimits {
            max_items: 1,
            max_json_bytes: 1024,
            max_waiters: 1,
            max_topics: 1,
            max_consumers: 1,
            max_topic_name_bytes: 1024,
        },
        observation_max_bytes: 2048,
        observation_ttl_ms: 1000,
    };
    let mut observation = Observation::new(Some(policy));
    let activity = EventRuntimeActivity {
        event_id: Uuid::now_v7(),
        workspace_id,
        contract_id: "custom.example".into(),
        contract_version: "1".into(),
        phase: EventTriggerPhase::Published,
        source_event_id: None,
        subscription_id: None,
        consumer_id: None,
        admission: None,
        observed_at_unix_ms: 1,
    };
    observation.record(activity.clone());
    assert_eq!(observation.snapshot(workspace_id).0.len(), 1);
    observation.entries[0].0 = Instant::now() - Duration::from_secs(2);
    let bytes = observation.bytes;
    assert!(observation.snapshot(workspace_id).0.is_empty());
    assert_eq!(observation.entries.len(), 1);
    assert_eq!(observation.bytes, bytes);
    observation.record(activity);
    assert_eq!(observation.entries.len(), 1);
    assert_eq!(observation.snapshot(workspace_id).0.len(), 1);
    assert!(observation.snapshot(Uuid::now_v7()).0.is_empty());
}
