use std::collections::BTreeSet;

use extension_contracts::{
    PluginDataFieldType, PluginDataModelContribution, PluginExtensionField, PluginOwnedCollection,
    PluginOwnedField, PluginStorageBinding,
};

use crate::{
    compile_managed_schema_plan, ExistingManagedSchemaOwnership, ManagedSchemaAction,
    ManagedSchemaCompilationError, ManagedSchemaObject, PluginSchemaOwner,
};

fn owner(version: &str) -> PluginSchemaOwner {
    PluginSchemaOwner {
        publisher_namespace: "taichuy".to_string(),
        plugin_code: "session_retry_distribution".to_string(),
        plugin_version: version.to_string(),
    }
}

fn desired() -> PluginDataModelContribution {
    PluginDataModelContribution {
        contribution_version: "1flowbase.plugin-data-model/v1".to_string(),
        storage_binding: PluginStorageBinding::Main,
        owned_collections: vec![PluginOwnedCollection {
            collection_code: "affinity".to_string(),
            fields: vec![PluginOwnedField {
                field_code: "conversation_id".to_string(),
                field_type: PluginDataFieldType::Uuid,
                nullable: false,
            }],
        }],
        extension_fields: vec![PluginExtensionField {
            target_table: "model_provider_instances".to_string(),
            field_code: "affinity_hint".to_string(),
            field_type: PluginDataFieldType::String,
            nullable: true,
        }],
    }
}

#[test]
fn pdm_002_pdm_006_compiles_namespaced_plan_without_target_opt_in() {
    let plan = compile_managed_schema_plan(
        owner("1.0.0"),
        &[desired()],
        &BTreeSet::from(["model_provider_instances".to_string()]),
        &[],
    )
    .unwrap();
    assert_eq!(plan.entries().len(), 3);
    assert!(matches!(
        plan.entries().first().map(|entry| &entry.object),
        Some(ManagedSchemaObject::OwnedCollection { .. })
    ));
    assert!(plan.entries().iter().all(|entry| {
        entry.action == ManagedSchemaAction::EnsurePresent
            && entry.object.ownership_key().len() < 128
    }));
    assert_eq!(plan.fingerprint().len(), 64);
}

#[test]
fn pdm_008_unknown_or_governance_target_fails_closed() {
    let unknown = compile_managed_schema_plan(owner("1"), &[desired()], &BTreeSet::new(), &[]);
    assert!(matches!(
        unknown,
        Err(ManagedSchemaCompilationError::UnknownTargetTable(_))
    ));

    let mut governance = desired();
    governance.extension_fields[0].target_table = "lifecycle_outbox".to_string();
    assert!(matches!(
        compile_managed_schema_plan(
            owner("1"),
            &[governance],
            &BTreeSet::from(["lifecycle_outbox".to_string()]),
            &[]
        ),
        Err(ManagedSchemaCompilationError::GovernanceTarget(_))
    ));
}

#[test]
fn pdm_009_upgrade_retains_removed_objects_instead_of_dropping() {
    let first = compile_managed_schema_plan(
        owner("1.0.0"),
        &[desired()],
        &BTreeSet::from(["model_provider_instances".to_string()]),
        &[],
    )
    .unwrap();
    let ownership = first
        .entries()
        .iter()
        .map(|entry| ExistingManagedSchemaOwnership {
            owner_id: first.owner().stable_id(),
            object: entry.object.clone(),
        })
        .collect::<Vec<_>>();
    let disabled = compile_managed_schema_plan(
        owner("2.0.0"),
        &[],
        &BTreeSet::from(["model_provider_instances".to_string()]),
        &ownership,
    )
    .unwrap();
    assert!(disabled
        .entries()
        .iter()
        .all(|entry| entry.action == ManagedSchemaAction::RetainInactive));
}
