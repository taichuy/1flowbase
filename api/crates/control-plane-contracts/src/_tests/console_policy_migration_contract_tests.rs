use domain::{ConsoleOperationId, ConsoleOperationPolicy, ConsolePolicyGroup};

use crate::{
    CompiledConsolePolicyCatalog, CompiledConsolePolicyGroup,
    CompiledConsolePolicyMigrationInventory, CompiledConsolePolicyMigrationPlan,
    ConsolePolicyMigrationError, ConsolePolicyMigrationLegacyGrantMapping,
    ConsolePolicyMigrationLegacyGrantProjection,
};

#[test]
fn compiled_console_policy_contract_exposes_canonical_read_surface() {
    let operation = ConsoleOperationPolicy::simple(
        ConsoleOperationId::try_from("console.settings.read").expect("valid operation id"),
        true,
    );
    let catalog = CompiledConsolePolicyCatalog {
        complete: true,
        groups: vec![CompiledConsolePolicyGroup {
            group: ConsolePolicyGroup::other("settings").expect("valid policy group"),
            full_operations: vec![operation.clone()],
        }],
    };
    let inventory = CompiledConsolePolicyMigrationInventory::from_compiled_parts(
        catalog.clone(),
        "sha256:catalog".to_string(),
    );
    let mapping = ConsolePolicyMigrationLegacyGrantMapping {
        legacy_grant: "legacy.settings.read".to_string(),
        projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![operation]),
    };
    let plan = CompiledConsolePolicyMigrationPlan::from_compiled_parts(
        inventory,
        vec![mapping.clone()],
        "sha256:mapping".to_string(),
    );

    assert_eq!(plan.catalog(), &catalog);
    assert_eq!(plan.catalog_fingerprint(), "sha256:catalog");
    assert_eq!(plan.mapping_fingerprint(), "sha256:mapping");
    assert_eq!(plan.mappings(), &[mapping]);
}

#[test]
fn migration_projection_and_error_keep_adapter_facing_serialization_semantics() {
    let projection = ConsolePolicyMigrationLegacyGrantProjection::NoProjection {
        evidence: "grant has no console authorization effect".to_string(),
    };

    assert_eq!(
        serde_json::to_value(projection).expect("projection must serialize"),
        serde_json::json!({
            "kind": "no_projection",
            "value": { "evidence": "grant has no console authorization effect" }
        })
    );
    assert_eq!(
        ConsolePolicyMigrationError::new("catalog drift").to_string(),
        "catalog drift"
    );
}
