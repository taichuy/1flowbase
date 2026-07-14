use access_control::{ConsoleAuthorization, ConsolePolicyGroup, SettingsFeatureOwnerKind};
use uuid::Uuid;

use crate::{
    app_state::{compile_core_console_operation_registry, compile_core_settings_feature_registry},
    console_policy_migration::{
        compile_core_console_policy_migration_plan, parse_command, write_evidence_report,
        ConsolePolicyMigrationCommand, ConsolePolicyMigrationEvidenceReport,
    },
};

#[test]
fn ac_010_live_core_crosswalk_disposes_each_of_175_operations() {
    let settings = compile_core_settings_feature_registry()
        .expect("the Core settings feature registry must compile before migration planning");
    let registry = compile_core_console_operation_registry(&settings)
        .expect("the live Core console registry must compile before migration planning");

    let migration = compile_core_console_policy_migration_plan(registry.inventory())
        .expect("the audited Core crosswalk must compile against the live registry");

    assert_eq!(migration.dispositions().len(), 175);
    assert!(migration
        .dispositions()
        .iter()
        .all(|disposition| disposition.operation_id() != "system_all"));
    assert!(migration
        .disposition("roles.console_policy.replace")
        .is_some_and(|disposition| disposition.is_default_disabled_new_operation()));
    assert!(migration
        .disposition("data_sources.secret.rotate")
        .is_some_and(|disposition| disposition
            .has_legacy_grant("settings_feature.access.system.data-models")));
    assert!(registry.inventory().operations.iter().any(|operation| {
        operation.operation_id == "core.authenticated"
            && operation.authorization == ConsoleAuthorization::Authenticated
    }));
}

#[test]
fn ac_010_feature_to_other_regroup_preserves_data_source_secret_rotation() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();

    let preview = migration
        .plan()
        .project_legacy_role(
            Uuid::now_v7(),
            &["settings_feature.access.system.data-models".to_string()],
        )
        .expect("the audited feature-to-Other regroup must project without an authorization delta");

    assert!(preview.authorization_delta.added.is_empty());
    assert!(preview.authorization_delta.removed.is_empty());
    assert!(preview.effective_delta.is_empty());
    assert!(preview.effective_after.iter().any(|entry| {
        entry.operation_id.as_str() == "data_sources.secret.rotate"
            && entry.simple_enabled == Some(true)
    }));
}

#[test]
fn ac_010_new_role_console_policy_operations_are_default_disabled() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();

    let preview = migration
        .plan()
        .project_legacy_role(
            Uuid::now_v7(),
            &["settings_feature.access.system.roles".to_string()],
        )
        .expect("the historic roles feature grant must remain projectable");

    for operation_id in [
        "roles.console_policy_catalog.view",
        "roles.console_policy.view",
        "roles.console_policy.replace",
    ] {
        assert!(preview
            .effective_after
            .iter()
            .all(|entry| entry.operation_id.as_str() != operation_id));
    }
}

#[test]
fn ac_010_group_or_operation_mapping_drift_hard_stops() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let mut inventory = registry.inventory().clone();
    inventory
        .operations
        .iter_mut()
        .find(|operation| operation.operation_id == "data_sources.secret.rotate")
        .expect("the live inventory must contain the audited regrouped operation")
        .policy_group = ConsolePolicyGroup::SettingsFeature("system.data-models".to_string());

    let error = compile_core_console_policy_migration_plan(&inventory)
        .expect_err("an operation group drift must not silently migrate grants");

    assert!(error
        .to_string()
        .contains("Core migration policy-group mismatch for data_sources.secret.rotate"));
}

#[test]
fn ac_010_dispositions_and_mappings_never_offer_system_all() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let serialized = serde_json::to_string(migration.dispositions()).unwrap();

    assert!(!serialized.contains("system_all"));
    assert!(migration
        .legacy_mappings()
        .iter()
        .all(|mapping| !mapping.legacy_grant.contains("system_all")));
}

#[test]
fn ac_010_active_host_operation_without_a_crosswalk_hard_stops() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let mut inventory = registry.inventory().clone();
    let operation = inventory
        .operations
        .iter_mut()
        .find(|operation| operation.operation_id == "workspace.update")
        .expect("the compiled Core inventory must contain workspace.update");
    operation.owner.kind = SettingsFeatureOwnerKind::HostExtension;
    operation.owner.owner_id = "fixture-host".to_string();
    operation.owner.version = "1.0.0".to_string();

    let error = compile_core_console_policy_migration_plan(&inventory)
        .expect_err("a linked HostExtension needs explicit migration metadata");

    assert!(error
        .to_string()
        .contains("active HostExtension fixture-host@1.0.0 contributes workspace.update"));
}

#[test]
fn ac_010_cli_commands_and_static_evidence_are_deterministic() {
    assert_eq!(
        parse_command("preview").unwrap(),
        ConsolePolicyMigrationCommand::Preview
    );
    assert_eq!(
        parse_command("apply").unwrap(),
        ConsolePolicyMigrationCommand::Apply
    );
    assert_eq!(
        parse_command("finalize").unwrap(),
        ConsolePolicyMigrationCommand::Finalize
    );
    assert_eq!(
        parse_command("rollback").unwrap(),
        ConsolePolicyMigrationCommand::Rollback
    );
    assert!(parse_command("delete").is_err());

    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let first = ConsolePolicyMigrationEvidenceReport::for_compiled(
        "preview",
        "00000000-0000-0000-0000-000000000001",
        &migration,
    );
    let second = ConsolePolicyMigrationEvidenceReport::for_compiled(
        "preview",
        "00000000-0000-0000-0000-000000000001",
        &migration,
    );
    let serialized = serde_json::to_string_pretty(&first).unwrap();

    assert_eq!(serialized, serde_json::to_string_pretty(&second).unwrap());
    assert!(serialized.contains(&first.catalog_fingerprint));
    assert!(serialized.contains(&first.mapping_fingerprint));
    assert!(serialized.contains("data_sources.secret.rotate"));
    assert!(!serialized.contains("system_all"));
    assert!(first
        .markdown()
        .contains("Runtime marker enforcement and service cutover are intentionally out of scope"));
    assert!(migration
        .source()
        .permission_resources
        .iter()
        .all(|resource| resource != "settings_route"));
    assert!(migration
        .legacy_mappings()
        .iter()
        .all(|mapping| !mapping.legacy_grant.starts_with("settings_route.visible.")));

    let paths = write_evidence_report(&first).unwrap();
    assert_eq!(std::fs::read_to_string(paths.json).unwrap(), serialized);
    assert!(std::fs::read_to_string(paths.markdown)
        .unwrap()
        .contains("Actor five-probe matrices"));
}
