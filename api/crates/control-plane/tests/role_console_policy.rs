use std::collections::BTreeSet;

use access_control::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory, ConsoleOperationInventoryEntry,
    ConsolePolicyGroup as RegisteredConsolePolicyGroup, ResourceAccessAction,
    ResourceAccessRegistration, ResourceAccessScopeKind, SettingsFeatureLifecycle,
    SettingsFeatureOwner, SettingsFeatureOwnerKind,
};
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    ConsolePolicyMode,
};
use uuid::Uuid;

use control_plane::application::console_policy_migration::{
    applications_console_policy_catalog, applications_legacy_console_grant_mappings,
};
use control_plane::role::console_policy_migration::{
    compile_console_policy_migration_plan, preview_console_policy_migration_actor_authorizations,
    project_legacy_role_console_policy, CompiledConsolePolicyCatalog, CompiledConsolePolicyGroup,
    ConsolePolicyMigrationActorProbeSet, ConsolePolicyMigrationActorRoleBinding,
    ConsolePolicyMigrationLegacyGrantMapping, ConsolePolicyMigrationLegacyGrantProjection,
    ConsolePolicyMigrationProbe, ConsolePolicyMigrationProbeKind, LegacyConsoleGrantMapping,
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

fn synthetic_compiled_inventory(reverse: bool) -> ConsoleOperationCompiledInventory {
    let owner = SettingsFeatureOwner {
        kind: SettingsFeatureOwnerKind::Core,
        owner_id: "migration-test".into(),
        version: "v1".into(),
    };
    let mut operations = vec![
        ConsoleOperationInventoryEntry {
            operation_id: "console.simple".into(),
            authorization_profile_id: "console.simple".into(),
            owner: owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: RegisteredConsolePolicyGroup::Other("migration".into()),
            order: 10,
            routes: vec![],
            authorization: ConsoleAuthorization::Simple,
        },
        ConsoleOperationInventoryEntry {
            operation_id: "console.create".into(),
            authorization_profile_id: "console.create".into(),
            owner: owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: RegisteredConsolePolicyGroup::Other("migration".into()),
            order: 20,
            routes: vec![],
            authorization: ConsoleAuthorization::Simple,
        },
        ConsoleOperationInventoryEntry {
            operation_id: "console.records.view".into(),
            authorization_profile_id: "console.records.view".into(),
            owner: owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: RegisteredConsolePolicyGroup::Other("migration".into()),
            order: 30,
            routes: vec![],
            authorization: ConsoleAuthorization::ResourceAction {
                resource_code: "records".into(),
                action_code: "view".into(),
            },
        },
    ];
    if reverse {
        operations.reverse();
    }

    ConsoleOperationCompiledInventory {
        schema_version: "test.console-policy-migration/v1",
        interfaces: vec![],
        operations,
        resources: vec![ResourceAccessRegistration {
            resource_code: "records".into(),
            owner,
            lifecycle: SettingsFeatureLifecycle::Active,
            scope_kind: ResourceAccessScopeKind::Workspace,
            identity_field: "id".into(),
            scope_field: Some("scope_id".into()),
            owner_field: Some("created_by".into()),
            label_ref: "records.label".into(),
            description_ref: Some("records.description".into()),
            actions: vec![ResourceAccessAction {
                action_code: "view".into(),
                label_ref: "records.view.label".into(),
                description_ref: Some("records.view.description".into()),
            }],
        }],
        locale_catalog: None,
    }
}

fn synthetic_mappings() -> Vec<ConsolePolicyMigrationLegacyGrantMapping> {
    vec![
        ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: "legacy.simple".into(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![simple(
                "console.simple",
            )]),
        },
        ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: "legacy.create".into(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![simple(
                "console.create",
            )]),
        },
        ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: "legacy.records.view.own".into(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![row(
                "console.records.view",
                ConsoleOperationRowScope::Own,
            )]),
        },
        ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: "legacy.records.view.all".into(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![row(
                "console.records.view",
                ConsoleOperationRowScope::ScopeAll,
            )]),
        },
        ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: "legacy.stale.non_console".into(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::NoProjection {
                evidence: "legacy route has no console operation owner".into(),
            },
        },
    ]
}

#[test]
fn ac_010_compiled_migration_plan_fingerprints_and_unions_actor_roles() {
    let mappings = synthetic_mappings();
    let plan =
        compile_console_policy_migration_plan(&synthetic_compiled_inventory(false), &mappings)
            .expect(
                "a complete compiled inventory and explicit mappings must form a migration plan",
            );
    let mut reversed_mappings = mappings.clone();
    reversed_mappings.reverse();
    let reordered = compile_console_policy_migration_plan(
        &synthetic_compiled_inventory(true),
        &reversed_mappings,
    )
    .expect("canonical fingerprints must not depend on input order");
    assert_eq!(plan.catalog_fingerprint(), reordered.catalog_fingerprint());
    assert_eq!(plan.mapping_fingerprint(), reordered.mapping_fingerprint());

    let own_role_id = Uuid::now_v7();
    let scope_role_id = Uuid::now_v7();
    let own_role = plan
        .project_legacy_role(
            own_role_id,
            &[
                "legacy.simple".into(),
                "legacy.records.view.own".into(),
                "legacy.stale.non_console".into(),
            ],
        )
        .expect("known own-row grants must project");
    let scope_role = plan
        .project_legacy_role(
            scope_role_id,
            &["legacy.create".into(), "legacy.records.view.all".into()],
        )
        .expect("known scope grants must project");
    let actor_user_id = Uuid::now_v7();
    let actor_previews = preview_console_policy_migration_actor_authorizations(
        &plan,
        &[ConsolePolicyMigrationActorProbeSet {
            binding: ConsolePolicyMigrationActorRoleBinding {
                actor_user_id,
                role_ids: vec![scope_role_id, own_role_id],
            },
            probes: vec![
                ConsolePolicyMigrationProbe {
                    operation_id: operation_id("console.simple"),
                    kind: ConsolePolicyMigrationProbeKind::Simple,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: operation_id("console.create"),
                    kind: ConsolePolicyMigrationProbeKind::Create,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: operation_id("console.records.view"),
                    kind: ConsolePolicyMigrationProbeKind::OwnRow,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: operation_id("console.records.view"),
                    kind: ConsolePolicyMigrationProbeKind::SameScopeOther,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: operation_id("console.records.view"),
                    kind: ConsolePolicyMigrationProbeKind::CrossScope,
                },
            ],
        }],
        &[own_role, scope_role],
    )
    .expect("multi-role allow union must be deterministic");

    let preview = &actor_previews[0];
    assert_eq!(preview.binding.actor_user_id, actor_user_id);
    assert_eq!(preview.binding.role_ids, vec![own_role_id, scope_role_id]);
    assert_eq!(preview.effective_before, preview.effective_after);
    assert!(preview.effective_delta.is_empty());
    assert!(preview.effective_before.iter().any(|result| {
        result.probe.kind == ConsolePolicyMigrationProbeKind::Simple && result.allowed
    }));
    assert!(preview.effective_before.iter().any(|result| {
        result.probe.kind == ConsolePolicyMigrationProbeKind::Create && result.allowed
    }));
    assert!(preview.effective_before.iter().any(|result| {
        result.probe.kind == ConsolePolicyMigrationProbeKind::OwnRow && result.allowed
    }));
    assert!(preview.effective_before.iter().any(|result| {
        result.probe.kind == ConsolePolicyMigrationProbeKind::SameScopeOther && result.allowed
    }));
    assert!(preview.effective_before.iter().any(|result| {
        result.probe.kind == ConsolePolicyMigrationProbeKind::CrossScope && !result.allowed
    }));

    let mut ambiguous = synthetic_mappings();
    ambiguous.push(ConsolePolicyMigrationLegacyGrantMapping {
        legacy_grant: "legacy.simple".into(),
        projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![simple(
            "console.simple",
        )]),
    });
    assert!(compile_console_policy_migration_plan(
        &synthetic_compiled_inventory(false),
        &ambiguous
    )
    .unwrap_err()
    .to_string()
    .contains("ambiguous legacy mapping"));
    assert!(plan
        .project_legacy_role(own_role_id, &["legacy.unknown".into()])
        .unwrap_err()
        .to_string()
        .contains("unknown legacy grant"));
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

    let serialized = serde_json::to_value(&preview).unwrap();
    assert_eq!(
        serialized["effective_before"],
        serialized["effective_after"]
    );
    assert_eq!(serialized["effective_delta"], serde_json::json!([]));
    assert_eq!(
        serialized["effective_before"],
        serde_json::json!([
            {
                "operation_id": "applications.create",
                "simple_enabled": true,
                "same_scope_own": null,
                "same_scope_other": null,
                "cross_scope": null
            },
            {
                "operation_id": "applications.view",
                "simple_enabled": null,
                "same_scope_own": true,
                "same_scope_other": true,
                "cross_scope": false
            }
        ])
    );
}
