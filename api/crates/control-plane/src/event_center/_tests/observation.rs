use super::*;
use control_plane_contracts::ports::EventBusLimits;
use domain::event_center::EventTriggerPhase;

fn policy(max_bytes: u64) -> EventCenterRuntimePolicy {
    EventCenterRuntimePolicy {
        bus_limits: EventBusLimits {
            max_items: 1,
            max_json_bytes: 1024,
            max_waiters: 1,
            max_topics: 1,
            max_consumers: 1,
            max_topic_name_bytes: 1024,
        },
        observation_max_bytes: max_bytes,
        observation_ttl_ms: 1000,
    }
}

fn activity(workspace_id: Uuid) -> EventRuntimeActivity {
    EventRuntimeActivity {
        source_context: None,
        original_subscriber_id: None,
        original_consumer_id: None,
        event_id: Uuid::now_v7(),
        workspace_id,
        contract_id: "custom.example".into(),
        phase: EventTriggerPhase::Published,
        source_event_id: None,
        subscription_id: None,
        consumer_id: None,
        admission: None,
        observed_at_unix_ms: 100,
    }
}

fn charge(activity: &EventRuntimeActivity) -> u64 {
    serde_json::to_vec(activity).unwrap().len() as u64 + size_of::<Entry>() as u64
}

#[test]
fn exact_ttl_expiry_is_visible_without_activity_or_snapshot_mutation() {
    let workspace_id = Uuid::now_v7();
    let mut observation = Observation::new(Some(policy(2048)));
    let now = Instant::now();
    let item = activity(workspace_id);
    observation.record_at(item.clone(), now);
    assert_eq!(observation.snapshot_at(workspace_id, now).0, vec![item]);
    let retained_bytes = observation.bytes;
    let expired_at = now + Duration::from_millis(1000);
    let expired = observation.snapshot_at(workspace_id, expired_at);
    assert!(expired.0.is_empty());
    assert_eq!(expired.1, ACCOUNTING_BYTES);
    assert!(expired.2.accounting_available);
    assert_eq!(expired.2.expired, 1);
    assert_eq!(expired.2.window_started_at_unix_ms, Some(100));
    assert_eq!(observation.snapshot_at(workspace_id, expired_at), expired);
    assert_eq!(observation.entries.len(), 1);
    assert_eq!(observation.bytes, retained_bytes);
    // Mutation materializes projected expiry exactly once.
    observation.record_at(activity(workspace_id), expired_at);
    let after_write = observation.snapshot_at(workspace_id, expired_at);
    assert_eq!(after_write.0.len(), 1);
    assert_eq!(after_write.2.expired, 1);
    assert_eq!(observation.entries.len(), 1);
}

#[test]
fn accounting_window_ends_distinguishably_and_restarts_on_next_record() {
    let workspace_id = Uuid::now_v7();
    let mut observation = Observation::new(Some(policy(2048)));
    let now = Instant::now();
    observation.record_at(activity(workspace_id), now);
    let end = now + Duration::from_millis(2000);
    let unavailable = observation.snapshot_at(workspace_id, end);
    assert!(unavailable.0.is_empty());
    assert_eq!(unavailable.1, 0);
    assert!(!unavailable.2.accounting_available);
    assert_eq!(unavailable.2.window_started_at_unix_ms, None);
    assert_eq!(unavailable.2.window_duration_ms, 2000);
    // Reading did not clear the actual storage; subsequent mutation reclaims it.
    assert_eq!(observation.accounting.len(), 1);
    let mut fresh = activity(workspace_id);
    fresh.observed_at_unix_ms = 2100;
    observation.record_at(fresh, end);
    let new_window = observation.snapshot_at(workspace_id, end).2;
    assert!(new_window.accounting_available);
    assert_eq!(new_window.window_started_at_unix_ms, Some(2100));
    assert_eq!(new_window.expired, 0);
    assert_eq!(observation.accounting.len(), 1);
    assert_eq!(observation.entries.len(), 1);
}

#[test]
fn budget_eviction_is_counted_in_the_victims_workspace_only() {
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    let item = activity(first);
    // Two ledgers and exactly one activity fit. The second activity evicts the first.
    let budget = 2 * ACCOUNTING_BYTES + charge(&item);
    let mut observation = Observation::new(Some(policy(budget)));
    let now = Instant::now();
    observation.record_at(item, now);
    observation.record_at(activity(second), now);
    let victim = observation.snapshot_at(first, now);
    assert!(victim.0.is_empty());
    assert!(victim.2.accounting_available);
    assert_eq!(victim.2.evicted, 1);
    let newcomer = observation.snapshot_at(second, now);
    assert_eq!(newcomer.0.len(), 1);
    assert_eq!(newcomer.2.evicted, 0);
    assert_eq!(victim.1 + newcomer.1, observation.bytes);
    assert!(observation.bytes <= budget);
    let unrelated = observation.snapshot_at(Uuid::now_v7(), now);
    assert!(!unrelated.2.accounting_available);
    assert_eq!(unrelated.2.evicted, 0);
}

#[test]
fn oversized_records_keep_only_bounded_payload_free_loss_accounting() {
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    let mut observation = Observation::new(Some(policy(2 * ACCOUNTING_BYTES)));
    let now = Instant::now();
    observation.record_at(activity(first), now);
    observation.record_at(activity(second), now);
    let snapshot = observation.snapshot_at(first, now);
    assert!(snapshot.0.is_empty());
    assert_eq!(snapshot.1, ACCOUNTING_BYTES);
    assert!(snapshot.2.accounting_available);
    assert_eq!(snapshot.2.oversize_dropped, 1);
    assert_eq!(snapshot.2.evicted, 0);
    assert_eq!(observation.snapshot_at(second, now).2.oversize_dropped, 1);
    let encoded = serde_json::to_string(&snapshot.2).unwrap();
    assert!(!encoded.contains("custom.example"));
    assert!(!encoded.contains("payload"));
    assert_eq!(observation.bytes, 2 * ACCOUNTING_BYTES);
}

#[test]
fn pressure_evicts_accounting_without_a_lifetime_workspace_map_or_global_loss_leak() {
    let first = Uuid::now_v7();
    let budget = 2 * ACCOUNTING_BYTES;
    let mut observation = Observation::new(Some(policy(budget)));
    let now = Instant::now();
    observation.record_at(activity(first), now);
    for _ in 0..100 {
        observation.record_at(activity(Uuid::now_v7()), now);
        assert!(observation.bytes <= budget);
        assert!(observation.accounting.len() <= 2);
    }
    assert_eq!(observation.accounting.len(), 2);
    assert!(observation.entries.is_empty());
    let lost_coverage = observation.snapshot_at(first, now).2;
    assert!(!lost_coverage.accounting_available);
    assert_eq!(lost_coverage.window_started_at_unix_ms, None);
    assert_eq!(lost_coverage.oversize_dropped, 0);
    // The same workspace can be admitted again; this is a new window, not a lifetime total.
    let mut fresh = activity(first);
    fresh.observed_at_unix_ms = 200;
    observation.record_at(fresh, now + Duration::from_millis(100));
    let reset = observation
        .snapshot_at(first, now + Duration::from_millis(100))
        .2;
    assert!(reset.accounting_available);
    assert_eq!(reset.window_started_at_unix_ms, Some(200));
    assert_eq!(reset.oversize_dropped, 1);
}

#[test]
fn entry_admission_preserves_its_own_ledger_when_other_ledgers_fill_the_budget() {
    let workspace = Uuid::now_v7();
    let item = activity(workspace);
    let budget = ACCOUNTING_BYTES + charge(&item);
    let mut observation = Observation::new(Some(policy(budget)));
    let now = Instant::now();
    // Oversize-only workspaces first fill the budget with ledgers.
    for _ in 0..10 {
        let mut oversized = activity(Uuid::now_v7());
        oversized.contract_id = "x".repeat(budget as usize);
        observation.record_at(oversized, now);
    }
    observation.record_at(item, now);
    let snapshot = observation.snapshot_at(workspace, now);
    assert_eq!(snapshot.0.len(), 1);
    assert!(snapshot.2.accounting_available);
    assert_eq!(snapshot.1, budget);
    assert_eq!(observation.bytes, budget);
}

#[test]
fn zero_ttl_or_insufficient_metadata_budget_explicitly_has_no_accounting() {
    let workspace = Uuid::now_v7();
    let mut zero_ttl = policy(2048);
    zero_ttl.observation_ttl_ms = 0;
    for policy in [zero_ttl, policy(ACCOUNTING_BYTES - 1)] {
        let mut observation = Observation::new(Some(policy));
        observation.record(activity(workspace));
        let snapshot = observation.snapshot(workspace);
        assert!(snapshot.0.is_empty());
        assert_eq!(snapshot.1, 0);
        assert!(!snapshot.2.accounting_available);
        assert!(observation.accounting.is_empty());
    }
}

#[test]
fn continuously_active_workspaces_still_have_finite_accounting_windows() {
    let workspace = Uuid::now_v7();
    let mut observation = Observation::new(Some(policy(4096)));
    let now = Instant::now();
    observation.record_at(activity(workspace), now);
    observation.record_at(activity(workspace), now + Duration::from_millis(1500));
    // Fresh activity survives the window boundary; the old counters do not.
    let boundary = observation.snapshot_at(workspace, now + Duration::from_millis(2000));
    assert_eq!(boundary.0.len(), 1);
    assert!(!boundary.2.accounting_available);
    let mut fresh = activity(workspace);
    fresh.observed_at_unix_ms = 2200;
    observation.record_at(fresh, now + Duration::from_millis(2100));
    let later = observation.snapshot_at(workspace, now + Duration::from_millis(2500));
    assert!(later.2.accounting_available);
    assert_eq!(later.2.window_started_at_unix_ms, Some(2200));
    // The 1500ms record expires during the new window, so the loss belongs there.
    assert_eq!(later.2.expired, 1);
    assert_eq!(later.0.len(), 1);
}
