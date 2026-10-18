use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Lease(Arc<AtomicUsize>);
impl EventDeliveryLease for Lease {}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn delivery_owns_exactly_one_lease_through_body_access_and_explicit_completion() {
    let released = Arc::new(AtomicUsize::new(0));
    for explicit in [false, true] {
        let body = serde_json::json!({"nul\u{0}":[u64::MAX,"中文\u{0}",null]});
        let delivery =
            EventDelivery::new(Arc::new(body.clone()), Box::new(Lease(released.clone())));
        assert_eq!(delivery.body(), &body);
        assert_eq!(released.load(Ordering::SeqCst), usize::from(explicit));
        if explicit {
            delivery.complete();
        } else {
            drop(delivery);
        }
    }
    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn resource_projection_roundtrips_full_width_explicit_budget_and_phase_counts() {
    let limits = EventBusLimits {
        max_items: 3,
        max_json_bytes: u64::MAX,
        max_waiters: 0,
        max_topics: 1,
        max_consumers: 2,
        max_topic_name_bytes: 4,
    };
    let snapshot = EventBusSnapshot {
        limits: Some(limits),
        reserved: 1,
        buffered: 1,
        in_flight: 1,
        charged_json_bytes: u64::MAX,
        ..Default::default()
    };
    let wire = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        wire["limits"]["max_json_bytes"],
        serde_json::json!(u64::MAX)
    );
    assert_eq!(wire["charged_json_bytes"], serde_json::json!(u64::MAX));
    assert_eq!(
        serde_json::from_value::<EventBusSnapshot>(wire).unwrap(),
        snapshot
    );
}
