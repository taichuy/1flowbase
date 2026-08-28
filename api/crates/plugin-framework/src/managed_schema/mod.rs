use std::collections::{BTreeMap, BTreeSet};

use extension_contracts::{PluginDataFieldType, PluginDataModelContribution};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

const RESERVED_FIELDS: &[&str] = &["id", "scope_id", "created_at", "updated_at"];
const GOVERNANCE_TABLES: &[&str] = &[
    "_sqlx_migrations",
    "lifecycle_outbox",
    "plugin_schema_ownership",
    "plugin_schema_reconcile_receipts",
    "plugin_data_idempotency_receipts",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginSchemaOwner {
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
}

impl PluginSchemaOwner {
    pub fn stable_id(&self) -> String {
        format!("{}/{}", self.publisher_namespace, self.plugin_code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ManagedSchemaObject {
    OwnedCollection {
        logical_collection: String,
        physical_table: String,
    },
    OwnedField {
        logical_collection: String,
        physical_table: String,
        logical_field: String,
        physical_column: String,
        field_type: PluginDataFieldType,
        nullable: bool,
    },
    ExtensionField {
        target_table: String,
        logical_field: String,
        physical_column: String,
        field_type: PluginDataFieldType,
    },
}

impl ManagedSchemaObject {
    pub fn ownership_key(&self) -> String {
        match self {
            Self::OwnedCollection { physical_table, .. } => format!("table:{physical_table}"),
            Self::OwnedField {
                physical_table,
                physical_column,
                ..
            }
            | Self::ExtensionField {
                target_table: physical_table,
                physical_column,
                ..
            } => format!("column:{physical_table}.{physical_column}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ManagedSchemaAction {
    EnsurePresent,
    RetainInactive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedSchemaPlanEntry {
    pub action: ManagedSchemaAction,
    pub object: ManagedSchemaObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingManagedSchemaOwnership {
    pub owner_id: String,
    pub object: ManagedSchemaObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveManagedSchemaPlan {
    owner: PluginSchemaOwner,
    entries: Vec<ManagedSchemaPlanEntry>,
    fingerprint: String,
}

impl EffectiveManagedSchemaPlan {
    pub fn owner(&self) -> &PluginSchemaOwner {
        &self.owner
    }

    pub fn entries(&self) -> &[ManagedSchemaPlanEntry] {
        &self.entries
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ManagedSchemaCompilationError {
    #[error("plugin schema owner identity is invalid")]
    InvalidOwner,
    #[error("multiple plugin data-model desired states are not allowed")]
    MultipleDesiredStates,
    #[error("plugin data-model desired state is invalid: {0}")]
    InvalidDesiredState(String),
    #[error("field name {0} is reserved by the Host")]
    ReservedField(String),
    #[error("target table {0} is not a registered business table")]
    UnknownTargetTable(String),
    #[error("target table {0} is a Host governance table")]
    GovernanceTarget(String),
    #[error("physical schema ownership {key} belongs to {actual_owner}, not {expected_owner}")]
    OwnershipConflict {
        key: String,
        expected_owner: String,
        actual_owner: String,
    },
    #[error("owned schema object {0} changed incompatibly")]
    IncompatibleOwnedObject(String),
}

pub fn compile_managed_schema_plan(
    owner: PluginSchemaOwner,
    contributions: &[PluginDataModelContribution],
    registered_business_tables: &BTreeSet<String>,
    existing_ownership: &[ExistingManagedSchemaOwnership],
) -> Result<EffectiveManagedSchemaPlan, ManagedSchemaCompilationError> {
    if owner.publisher_namespace.trim().is_empty()
        || owner.plugin_code.trim().is_empty()
        || owner.plugin_version.trim().is_empty()
    {
        return Err(ManagedSchemaCompilationError::InvalidOwner);
    }
    if contributions.len() > 1 {
        return Err(ManagedSchemaCompilationError::MultipleDesiredStates);
    }
    let owner_id = owner.stable_id();
    let prefix = owner_prefix(&owner_id);
    let mut desired = BTreeMap::new();
    if let Some(contribution) = contributions.first() {
        contribution.validate_additive_v1().map_err(|error| {
            ManagedSchemaCompilationError::InvalidDesiredState(error.to_string())
        })?;
        for collection in &contribution.owned_collections {
            let table = bounded_identifier(&format!("plg_{prefix}_{}", collection.collection_code));
            insert_desired(
                &mut desired,
                ManagedSchemaObject::OwnedCollection {
                    logical_collection: collection.collection_code.clone(),
                    physical_table: table.clone(),
                },
            );
            for field in &collection.fields {
                reject_reserved(&field.field_code)?;
                insert_desired(
                    &mut desired,
                    ManagedSchemaObject::OwnedField {
                        logical_collection: collection.collection_code.clone(),
                        physical_table: table.clone(),
                        logical_field: field.field_code.clone(),
                        physical_column: bounded_identifier(&field.field_code),
                        field_type: field.field_type,
                        nullable: field.nullable,
                    },
                );
            }
        }
        for field in &contribution.extension_fields {
            reject_reserved(&field.field_code)?;
            if GOVERNANCE_TABLES.contains(&field.target_table.as_str()) {
                return Err(ManagedSchemaCompilationError::GovernanceTarget(
                    field.target_table.clone(),
                ));
            }
            if !registered_business_tables.contains(&field.target_table) {
                return Err(ManagedSchemaCompilationError::UnknownTargetTable(
                    field.target_table.clone(),
                ));
            }
            insert_desired(
                &mut desired,
                ManagedSchemaObject::ExtensionField {
                    target_table: field.target_table.clone(),
                    logical_field: field.field_code.clone(),
                    physical_column: bounded_identifier(&format!(
                        "ext_{prefix}_{}",
                        field.field_code
                    )),
                    field_type: field.field_type,
                },
            );
        }
    }

    let existing = existing_ownership
        .iter()
        .map(|entry| (entry.object.ownership_key(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut entries = Vec::new();
    for (key, object) in &desired {
        if let Some(current) = existing.get(key) {
            if current.owner_id != owner_id {
                return Err(ManagedSchemaCompilationError::OwnershipConflict {
                    key: key.clone(),
                    expected_owner: owner_id.clone(),
                    actual_owner: current.owner_id.clone(),
                });
            }
            if current.object != *object {
                return Err(ManagedSchemaCompilationError::IncompatibleOwnedObject(
                    key.clone(),
                ));
            }
        }
        entries.push(ManagedSchemaPlanEntry {
            action: ManagedSchemaAction::EnsurePresent,
            object: object.clone(),
        });
    }
    for (key, current) in existing {
        if current.owner_id == owner_id && !desired.contains_key(&key) {
            entries.push(ManagedSchemaPlanEntry {
                action: ManagedSchemaAction::RetainInactive,
                object: current.object.clone(),
            });
        }
    }
    entries.sort_by_key(|entry| {
        let dependency_rank = match (&entry.action, &entry.object) {
            (ManagedSchemaAction::EnsurePresent, ManagedSchemaObject::OwnedCollection { .. }) => 0,
            (ManagedSchemaAction::EnsurePresent, ManagedSchemaObject::OwnedField { .. }) => 1,
            (ManagedSchemaAction::EnsurePresent, ManagedSchemaObject::ExtensionField { .. }) => 2,
            (ManagedSchemaAction::RetainInactive, _) => 3,
        };
        (dependency_rank, entry.object.ownership_key())
    });
    let fingerprint = fingerprint(&owner, &entries);
    Ok(EffectiveManagedSchemaPlan {
        owner,
        entries,
        fingerprint,
    })
}

fn insert_desired(index: &mut BTreeMap<String, ManagedSchemaObject>, object: ManagedSchemaObject) {
    index.insert(object.ownership_key(), object);
}

fn reject_reserved(field: &str) -> Result<(), ManagedSchemaCompilationError> {
    if RESERVED_FIELDS.contains(&field) {
        return Err(ManagedSchemaCompilationError::ReservedField(
            field.to_string(),
        ));
    }
    Ok(())
}

fn owner_prefix(owner_id: &str) -> String {
    let digest = Sha256::digest(owner_id.as_bytes());
    format!("{digest:x}")[..12].to_string()
}

fn bounded_identifier(value: &str) -> String {
    value.chars().take(63).collect()
}

fn fingerprint(owner: &PluginSchemaOwner, entries: &[ManagedSchemaPlanEntry]) -> String {
    let canonical =
        serde_json::to_vec(&(owner, entries)).expect("managed schema plan is always serializable");
    format!("{:x}", Sha256::digest(canonical))
}
