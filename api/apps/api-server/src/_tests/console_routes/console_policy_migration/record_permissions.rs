use super::*;
use crate::console_policy_migration::ConsolePolicyMigrationOperationDispositionKind;

const RECORD_OPERATIONS: &[&str] = &[
    "roles.record_permissions.view",
    "roles.record_permissions.replace",
    "roles.record_capabilities.get",
];

#[test]
fn record_permissions_have_complete_migration_dispositions() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory())
        .expect("all three active record operations need audited migration dispositions");
    assert_eq!(
        migration.dispositions().len(),
        registry.inventory().operations.len()
    );
    for operation in &RECORD_OPERATIONS[..2] {
        let disposition = migration.disposition(operation).unwrap();
        assert_eq!(disposition.policy_group_kind, "settings_feature");
        assert_eq!(disposition.policy_group_id, "system.roles");
        assert_eq!(disposition.authorization, "simple");
        assert!(disposition.is_default_disabled_new_operation());
    }
    let capabilities = migration.disposition(RECORD_OPERATIONS[2]).unwrap();
    assert_eq!(capabilities.policy_group_kind, "other");
    assert_eq!(capabilities.policy_group_id, "core.authenticated");
    assert_eq!(capabilities.authorization, "authenticated");
    assert!(matches!(
        capabilities.disposition,
        ConsolePolicyMigrationOperationDispositionKind::NoProjection { .. }
    ));
    let operation = registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == RECORD_OPERATIONS[2])
        .unwrap();
    assert_eq!(operation.authorization, ConsoleAuthorization::Authenticated);
}

#[test]
fn record_permissions_never_inherit_legacy_role_authority() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let legacy_grants = migration
        .legacy_mappings()
        .iter()
        .map(|mapping| mapping.legacy_grant.clone())
        .collect::<Vec<_>>();
    for mapping in migration.legacy_mappings() {
        for operation in RECORD_OPERATIONS {
            assert!(
                !migration
                    .disposition(operation)
                    .unwrap()
                    .has_legacy_grant(&mapping.legacy_grant),
                "{} must not grant {operation}",
                mapping.legacy_grant
            );
        }
        if let ConsolePolicyMigrationLegacyGrantProjection::Operations(operations) =
            &mapping.projection
        {
            assert!(
                operations.iter().all(
                    |operation| !RECORD_OPERATIONS.contains(&operation.operation_id().as_str())
                ),
                "{} must not create record management or capability grants",
                mapping.legacy_grant
            );
        }
    }
    let legacy_roles_feature = vec!["settings_feature.access.system.roles".to_string()];
    for grants in [
        &[][..],
        legacy_roles_feature.as_slice(),
        legacy_grants.as_slice(),
    ] {
        let preview = project_compiled_console_policy_migration_plan(
            migration.plan(),
            Uuid::now_v7(),
            grants,
        )
        .unwrap();
        assert!(preview
            .effective_after
            .iter()
            .all(|operation| !RECORD_OPERATIONS.contains(&operation.operation_id.as_str())));
        assert!(preview.authorization_delta.added.is_empty());
        assert!(preview.authorization_delta.removed.is_empty());
        assert!(preview.effective_delta.is_empty());
    }
}
