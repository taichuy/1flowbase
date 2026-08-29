use std::collections::BTreeSet;

use serde::Deserialize;

#[derive(Deserialize)]
struct Inventory {
    issue: u64,
    input_sha: String,
    frozen: bool,
    migration_scope: Vec<String>,
    entries: Vec<Entry>,
    acceptance_matrix: Vec<AcceptanceRow>,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    family: String,
    method_or_tool: String,
    request_response_stream_error: String,
    authentication: String,
    principal_actor_scope: String,
    authorization_csrf: String,
    deadline_cancel_idempotency_retry: String,
    handler_application_transaction_runtime: String,
    side_effects: String,
    tests_consumers: String,
    source_anchor: String,
    source_probe: String,
}

#[derive(Deserialize)]
struct AcceptanceRow {
    id: String,
    ac: Vec<String>,
    fixture: String,
}

fn inventory() -> Inventory {
    serde_json::from_str(include_str!(
        "fixtures/interface_coverage_inventory.1944.json"
    ))
    .expect("#1944 frozen coverage inventory must remain valid JSON")
}

#[test]
fn issue_1944_inventory_is_finite_frozen_and_source_anchored() {
    let inventory = inventory();
    assert_eq!(inventory.issue, 1944);
    assert_eq!(
        inventory.input_sha,
        "ff4cc74ab073256419884d3d96e0b3defcb36d45"
    );
    assert!(inventory.frozen);
    assert_eq!(inventory.entries.len(), 10);

    let required_families = BTreeSet::from([
        "console_http",
        "public_auth",
        "application_native_api",
        "compatibility_api",
        "sse_websocket",
        "mcp_json_rpc",
        "api_ex",
        "internal_background",
        "interface_invocation_kernel",
    ]);
    let actual_families = inventory
        .entries
        .iter()
        .map(|entry| entry.family.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_families, required_families);

    let mut ids = BTreeSet::new();
    for entry in &inventory.entries {
        assert!(ids.insert(entry.id.as_str()), "duplicate inventory id");
        for value in [
            &entry.method_or_tool,
            &entry.request_response_stream_error,
            &entry.authentication,
            &entry.principal_actor_scope,
            &entry.authorization_csrf,
            &entry.deadline_cancel_idempotency_retry,
            &entry.handler_application_transaction_runtime,
            &entry.side_effects,
            &entry.tests_consumers,
            &entry.source_anchor,
            &entry.source_probe,
        ] {
            assert!(!value.trim().is_empty(), "inventory fields are mandatory");
        }
        let source = std::fs::read_to_string(format!(
            "{}/src/{}",
            env!("CARGO_MANIFEST_DIR"),
            entry.source_anchor
        ))
        .unwrap_or_else(|error| panic!("missing source anchor {}: {error}", entry.source_anchor));
        assert!(
            source.contains(&entry.source_probe),
            "source anchor no longer represents {}",
            entry.id
        );
    }

    assert_eq!(
        inventory.migration_scope,
        [
            "public.auth.login-instances",
            "console.host-infrastructure.providers",
            "application.native.runs",
            "mcp.user-api-key.tools",
        ]
    );
}

#[test]
fn issue_1944_acceptance_matrix_covers_every_arc_acceptance_criterion_once_or_more() {
    let inventory = inventory();
    assert_eq!(inventory.acceptance_matrix.len(), 7);
    let mut row_ids = BTreeSet::new();
    let covered = inventory
        .acceptance_matrix
        .iter()
        .flat_map(|row| {
            assert!(row_ids.insert(row.id.as_str()));
            assert!(!row.fixture.trim().is_empty());
            row.ac.iter().map(String::as_str)
        })
        .collect::<BTreeSet<_>>();
    let expected = (1..=16)
        .map(|index| format!("ARC-AC-{index:03}"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        covered,
        expected.iter().map(String::as_str).collect::<BTreeSet<_>>()
    );
}
