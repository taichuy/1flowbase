use std::collections::BTreeSet;

use serde::Deserialize;

#[derive(Deserialize)]
struct EquivalenceFixture {
    issue: u64,
    input_sha: String,
    approved_route_count: usize,
    dimensions: Vec<String>,
    routes: Vec<RouteEvidence>,
    gap_ledger: Vec<String>,
}

#[derive(Deserialize)]
struct RouteEvidence {
    id: String,
    route: String,
    source: String,
    kernel_probe: String,
    behavior_test: String,
    behavior_probe: String,
    status: String,
}

#[test]
fn issue_1944_approved_routes_have_complete_equivalence_evidence() {
    let fixture: EquivalenceFixture = serde_json::from_str(include_str!(
        "fixtures/interface_route_equivalence.1944.json"
    ))
    .unwrap();
    assert_eq!(fixture.issue, 1944);
    assert_eq!(
        fixture.input_sha,
        "ff4cc74ab073256419884d3d96e0b3defcb36d45"
    );
    assert_eq!(fixture.routes.len(), fixture.approved_route_count);
    assert_eq!(
        fixture
            .dimensions
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "allow_deny",
            "row_scope",
            "state_mutation",
            "dto",
            "error_status",
            "stream_event_order",
            "transaction_outbox",
            "runtime_dispatch",
            "audit_receipt",
        ])
    );
    let mut route_ids = BTreeSet::new();
    for route in fixture.routes {
        assert!(route_ids.insert(route.id));
        assert!(!route.route.is_empty());
        assert_eq!(route.status, "equivalent");
        let source = std::fs::read_to_string(format!(
            "{}/src/{}",
            env!("CARGO_MANIFEST_DIR"),
            route.source
        ))
        .unwrap();
        assert!(source.contains(&route.kernel_probe));
        let behavior = std::fs::read_to_string(format!(
            "{}/src/{}",
            env!("CARGO_MANIFEST_DIR"),
            route.behavior_test
        ))
        .unwrap();
        assert!(behavior.contains(&route.behavior_probe));
    }
    assert_eq!(fixture.gap_ledger.len(), 5);
}
