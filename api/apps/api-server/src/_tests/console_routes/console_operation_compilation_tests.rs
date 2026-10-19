use access_control::{ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteBinding};

use crate::console_operation_compilation::{
    compile_console_operation_snapshot, ConsoleBindingOwnerKind,
    ConsoleBindingOwnershipContribution, ConsoleMigrationDisposition,
    ConsoleMigrationDispositionContribution, ConsoleOperationPolicyContribution,
};

fn route(path: &str) -> ConsoleRouteBinding {
    ConsoleRouteBinding {
        method: "GET".to_string(),
        path: path.to_string(),
    }
}

fn policy(operation_id: &str, path: &str) -> ConsoleOperationPolicyContribution {
    ConsoleOperationPolicyContribution {
        operation_id: operation_id.to_string(),
        authorization_profile_id: operation_id.to_string(),
        owner_id: "boot-core".to_string(),
        owner_active: true,
        policy_group: ConsolePolicyGroup::Other("other.fixture".to_string()),
        authorization: ConsoleAuthorization::Authenticated,
        routes: vec![route(path)],
    }
}

fn binding(
    operation_id: &str,
    path: &str,
    contribution_id: &str,
    binding_id: &str,
) -> ConsoleBindingOwnershipContribution {
    ConsoleBindingOwnershipContribution {
        contribution_id: contribution_id.to_string(),
        owner_kind: ConsoleBindingOwnerKind::Family,
        owner_id: contribution_id.to_string(),
        owner_active: true,
        interface_id: operation_id.to_string(),
        binding_id: binding_id.to_string(),
        protocol: "http".to_string(),
        route: route(path),
    }
}

fn migration(operation_id: &str) -> ConsoleMigrationDispositionContribution {
    ConsoleMigrationDispositionContribution {
        operation_id: operation_id.to_string(),
        disposition: ConsoleMigrationDisposition::NoProjection {
            evidence: "authenticated fixture".to_string(),
        },
    }
}

#[test]
fn compiler_reports_complete_duplicate_missing_extra_and_owner_failures() {
    let mut inactive = binding(
        "operation.present",
        "/api/console/present",
        "family.inactive",
        "http.console.duplicate.v1",
    );
    inactive.owner_active = false;
    let error = compile_console_operation_snapshot(
        [
            policy("operation.present", "/api/console/present"),
            policy("operation.present", "/api/console/present-again"),
            policy("operation.missing", "/api/console/missing"),
        ],
        [
            inactive,
            binding(
                "operation.present",
                "/api/console/present",
                "family.other",
                "http.console.duplicate.v1",
            ),
            binding(
                "operation.extra",
                "/api/console/extra",
                "family.unknown",
                "http.console.extra.v1",
            ),
        ],
        [
            migration("operation.present"),
            migration("operation.present"),
            migration("operation.extra"),
        ],
        ["family.inactive".to_string(), "family.other".to_string()],
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("duplicate operation contribution operation.present"));
    assert!(error.contains("duplicate binding identity http.console.duplicate.v1"));
    assert!(error.contains("conflicting binding owner"));
    assert!(error.contains("inactive binding owner family.inactive"));
    assert!(error.contains("unknown binding owner family.unknown"));
    assert!(error.contains("missing binding owner for operation operation.missing"));
    assert!(error.contains("missing migration disposition operation.missing"));
    assert!(error.contains("duplicate migration disposition operation.present"));
    assert!(error.contains("extra migration disposition operation.extra"));
}

#[test]
fn authenticated_operation_remains_authenticated_in_compiled_snapshot() {
    let snapshot = compile_console_operation_snapshot(
        [policy(
            "frontstage.blocks.delete",
            "/api/console/frontstage/blocks/:id",
        )],
        [binding(
            "frontstage.blocks.delete",
            "/api/console/frontstage/blocks/{id}",
            "api-server.console-frontstage-blocks",
            "http.console.frontstage.blocks.delete.v1",
        )],
        [migration("frontstage.blocks.delete")],
        ["api-server.console-frontstage-blocks".to_string()],
    )
    .unwrap();

    let operation = snapshot.operation("frontstage.blocks.delete").unwrap();
    assert_eq!(operation.authorization, ConsoleAuthorization::Authenticated);
    assert!(!matches!(
        operation.authorization,
        ConsoleAuthorization::Simple
    ));
    assert_eq!(operation.bindings.len(), 1);
}
