use std::collections::BTreeSet;

use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    ConsolePolicyMode,
};
use uuid::Uuid;

use control_plane::application::console_policy_migration::{
    applications_console_policy_catalog, applications_legacy_console_grant_mappings,
};
use control_plane::role::console_policy_migration::{
    project_legacy_role_console_policy, CompiledConsolePolicyCatalog, CompiledConsolePolicyGroup,
    LegacyConsoleGrantMapping,
};

fn operation_id(value: &str) -> ConsoleOperationId {
    ConsoleOperationId::try_from(value).expect("test operation id must be valid")
}

fn applications_group() -> ConsolePolicyGroup {
    ConsolePolicyGroup::settings_feature("system.applications")
        .expect("test group id must be valid")
}

fn simple(value: &str) -> ConsoleOperationPolicy {
    ConsoleOperationPolicy::simple(operation_id(value), true)
}

fn row(value: &str, scope: ConsoleOperationRowScope) -> ConsoleOperationPolicy {
    ConsoleOperationPolicy::row(operation_id(value), scope)
}

fn catalog(
    complete: bool,
    full_operations: Vec<ConsoleOperationPolicy>,
) -> CompiledConsolePolicyCatalog {
    CompiledConsolePolicyCatalog {
        complete,
        groups: vec![CompiledConsolePolicyGroup {
            group: applications_group(),
            full_operations,
        }],
    }
}

fn mappings() -> Vec<LegacyConsoleGrantMapping> {
    vec![
        LegacyConsoleGrantMapping {
            legacy_grant: "application.create.all".into(),
            operations: vec![simple("applications.create")],
        },
        LegacyConsoleGrantMapping {
            legacy_grant: "application.view.all".into(),
            operations: vec![row("applications.view", ConsoleOperationRowScope::ScopeAll)],
        },
        LegacyConsoleGrantMapping {
            legacy_grant: "application.view.own".into(),
            operations: vec![row("applications.view", ConsoleOperationRowScope::Own)],
        },
    ]
}

#[test]
fn ac_010_applications_legacy_projection_is_exact_and_never_expands_partial_roles() {
    let role_id = Uuid::now_v7();
    let catalog = applications_console_policy_catalog();
    let mappings = applications_legacy_console_grant_mappings();
    let exact = project_legacy_role_console_policy(
        role_id,
        &[
            access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION.into(),
            "application.create.all".into(),
            "application.view.all".into(),
            "application.edit.all".into(),
            "application.delete.all".into(),
        ],
        &catalog,
        &mappings,
    )
    .expect("known exact applications profile must migrate");
    assert_eq!(exact.policy.groups()[0].mode(), ConsolePolicyMode::Full);
    assert!(exact.authorization_delta.added.is_empty());
    assert!(exact.authorization_delta.removed.is_empty());

    let partial = project_legacy_role_console_policy(
        role_id,
        &["application.view.own".into()],
        &catalog,
        &mappings,
    )
    .expect("known partial applications profile must migrate");
    assert_eq!(partial.policy.groups()[0].mode(), ConsolePolicyMode::Custom);
    assert_eq!(partial.policy.groups()[0].operations().len(), 1);
    assert_eq!(
        partial.policy.groups()[0].operations()[0].row_scope(),
        Some(ConsoleOperationRowScope::Own)
    );

    let unknown = project_legacy_role_console_policy(
        role_id,
        &["application.publish.all".into()],
        &catalog,
        &mappings,
    )
    .unwrap_err();
    assert!(unknown.to_string().contains("unknown legacy grant"));
}

#[test]
fn ac_004_console_policy_full_requires_exact_profile_and_custom_does_not_expand() {
    let role_id = Uuid::now_v7();
    let initial_catalog = catalog(
        true,
        vec![
            simple("applications.create"),
            row("applications.view", ConsoleOperationRowScope::ScopeAll),
        ],
    );
    let full = project_legacy_role_console_policy(
        role_id,
        &[
            "application.create.all".into(),
            "application.view.all".into(),
        ],
        &initial_catalog,
        &mappings(),
    )
    .expect("complete explicit projection should succeed");
    assert_eq!(full.policy.groups()[0].mode(), ConsolePolicyMode::Full);
    assert!(full.policy.groups()[0].operations().is_empty());

    let expanded_catalog = catalog(
        true,
        vec![
            simple("applications.create"),
            row("applications.view", ConsoleOperationRowScope::ScopeAll),
            row("applications.delete", ConsoleOperationRowScope::ScopeAll),
        ],
    );
    let custom = project_legacy_role_console_policy(
        role_id,
        &[
            "application.create.all".into(),
            "application.view.all".into(),
        ],
        &expanded_catalog,
        &mappings(),
    )
    .expect("known grants should project against the expanded catalog");
    assert_eq!(custom.policy.groups()[0].mode(), ConsolePolicyMode::Custom);
    assert_eq!(custom.policy.groups()[0].operations().len(), 2);
    assert!(!custom.policy.groups()[0]
        .operations()
        .iter()
        .any(|policy| policy.operation_id().as_str() == "applications.delete"));
}

#[test]
fn ac_006_console_policy_row_scope_excludes_system_all() {
    assert_eq!(
        ConsoleOperationRowScope::parse("disabled"),
        Some(ConsoleOperationRowScope::Disabled)
    );
    assert_eq!(
        ConsoleOperationRowScope::parse("own"),
        Some(ConsoleOperationRowScope::Own)
    );
    assert_eq!(
        ConsoleOperationRowScope::parse("scope_all"),
        Some(ConsoleOperationRowScope::ScopeAll)
    );
    assert_eq!(ConsoleOperationRowScope::parse("system_all"), None);
    assert!(serde_json::from_str::<ConsoleOperationRowScope>("\"system_all\"").is_err());
}

#[test]
fn ac_010_console_policy_projection_stops_on_unknown_ambiguous_or_incomplete_inputs() {
    let role_id = Uuid::now_v7();
    let complete_catalog = catalog(
        true,
        vec![
            simple("applications.create"),
            row("applications.view", ConsoleOperationRowScope::ScopeAll),
        ],
    );
    let unknown = project_legacy_role_console_policy(
        role_id,
        &["application.unknown.all".into()],
        &complete_catalog,
        &mappings(),
    )
    .unwrap_err();
    assert!(unknown.to_string().contains("unknown legacy grant"));

    let mut ambiguous_mappings = mappings();
    ambiguous_mappings.push(LegacyConsoleGrantMapping {
        legacy_grant: "application.create.all".into(),
        operations: vec![simple("applications.create")],
    });
    let ambiguous = project_legacy_role_console_policy(
        role_id,
        &["application.create.all".into()],
        &complete_catalog,
        &ambiguous_mappings,
    )
    .unwrap_err();
    assert!(ambiguous.to_string().contains("ambiguous legacy mapping"));

    let incomplete = project_legacy_role_console_policy(
        role_id,
        &["application.create.all".into()],
        &catalog(false, vec![simple("applications.create")]),
        &mappings(),
    )
    .unwrap_err();
    assert!(incomplete
        .to_string()
        .contains("operation catalog is incomplete"));
}

#[test]
fn ac_010_console_policy_projection_unions_legacy_own_and_all_to_scope_all() {
    let view_mappings = mappings()
        .into_iter()
        .filter(|mapping| mapping.legacy_grant.starts_with("application.view."))
        .collect::<Vec<_>>();
    let preview = project_legacy_role_console_policy(
        Uuid::now_v7(),
        &["application.view.own".into(), "application.view.all".into()],
        &catalog(
            true,
            vec![row("applications.view", ConsoleOperationRowScope::ScopeAll)],
        ),
        &view_mappings,
    )
    .expect("legacy allow union should be deterministic");

    assert_eq!(preview.policy.groups()[0].mode(), ConsolePolicyMode::Full);
    assert!(preview.authorization_delta.added.is_empty());
    assert!(preview.authorization_delta.removed.is_empty());
}

#[test]
fn ac_011_console_policy_preview_delta_is_deterministic_and_full_is_not_materialized() {
    let role_id = Uuid::now_v7();
    let preview = project_legacy_role_console_policy(
        role_id,
        &[
            "application.view.all".into(),
            "application.create.all".into(),
        ],
        &catalog(
            true,
            vec![
                simple("applications.create"),
                row("applications.view", ConsoleOperationRowScope::ScopeAll),
            ],
        ),
        &mappings(),
    )
    .expect("complete explicit projection should succeed");

    assert_eq!(preview.policy.groups()[0].mode(), ConsolePolicyMode::Full);
    assert!(preview.policy.groups()[0].operations().is_empty());
    assert!(preview.authorization_delta.added.is_empty());
    assert!(preview.authorization_delta.removed.is_empty());
    assert_eq!(
        serde_json::to_value(&preview.authorization_delta).unwrap(),
        serde_json::json!({"added": [], "removed": []})
    );
    assert_eq!(
        preview.source_grants,
        BTreeSet::from([
            "application.create.all".to_string(),
            "application.view.all".to_string(),
        ])
    );
}
