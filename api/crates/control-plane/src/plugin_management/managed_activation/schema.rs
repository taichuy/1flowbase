use std::collections::BTreeSet;

use crate::ports::{
    ManagedSchemaFieldType, ManagedSchemaObjectKind, ManagedSchemaOperation,
    ManagedSchemaOwnershipRecord, ManagedSchemaPlan, ManagedSchemaPreview, ManagedSchemaRepository,
    ModelDefinitionRepository,
};
use extension_contracts::{PluginDataFieldType, PluginDataModelContribution};
use plugin_framework::{
    compile_managed_schema_plan, EffectiveManagedSchemaPlan, ExistingManagedSchemaOwnership,
    ManagedSchemaAction, ManagedSchemaObject, PluginSchemaOwner,
};

use anyhow::Result;

const DEFAULT_SCHEMA_LOCK_TIMEOUT_MS: u32 = 5_000;

/// Compile the same ownership-aware schema plan for install, enable and version selection.
pub async fn prepare_managed_schema_plan<R: ManagedSchemaRepository + ModelDefinitionRepository>(
    repository: &R,
    workspace_id: uuid::Uuid,
    owner: PluginSchemaOwner,
    contributions: &[PluginDataModelContribution],
) -> Result<(ManagedSchemaPlan, ManagedSchemaPreview)> {
    let registered_business_tables = repository
        .list_model_definitions(workspace_id)
        .await?
        .into_iter()
        .map(|definition| definition.physical_table_name)
        .collect::<BTreeSet<_>>();
    let existing = repository
        .list_managed_schema_ownership()
        .await?
        .iter()
        .map(existing_ownership)
        .collect::<Result<Vec<_>>>()?;
    let effective =
        compile_managed_schema_plan(owner, contributions, &registered_business_tables, &existing)
            .map_err(|_| crate::errors::ControlPlaneError::Conflict("plugin_managed_schema"))?;
    let plan = repository_plan(&effective);
    let preview = repository.preview_managed_schema(&plan).await?;
    Ok((plan, preview))
}

fn existing_ownership(
    record: &ManagedSchemaOwnershipRecord,
) -> Result<ExistingManagedSchemaOwnership, anyhow::Error> {
    let field_type = record.field_type.map(plugin_field_type);
    let object = match record.object_kind {
        ManagedSchemaObjectKind::OwnedCollection => ManagedSchemaObject::OwnedCollection {
            logical_collection: record.logical_name.clone(),
            physical_table: record.physical_table.clone(),
        },
        ManagedSchemaObjectKind::OwnedField => {
            let (logical_collection, logical_field) = record.logical_name.split_once('.').ok_or(
                crate::errors::ControlPlaneError::Conflict("plugin_managed_schema_ownership"),
            )?;
            ManagedSchemaObject::OwnedField {
                logical_collection: logical_collection.to_string(),
                physical_table: record.physical_table.clone(),
                logical_field: logical_field.to_string(),
                physical_column: required_column(record)?,
                field_type: field_type.ok_or(crate::errors::ControlPlaneError::Conflict(
                    "plugin_managed_schema_ownership",
                ))?,
                nullable: record
                    .nullable
                    .ok_or(crate::errors::ControlPlaneError::Conflict(
                        "plugin_managed_schema_ownership",
                    ))?,
            }
        }
        ManagedSchemaObjectKind::ExtensionField => ManagedSchemaObject::ExtensionField {
            target_table: record.physical_table.clone(),
            logical_field: record.logical_name.clone(),
            physical_column: required_column(record)?,
            field_type: field_type.ok_or(crate::errors::ControlPlaneError::Conflict(
                "plugin_managed_schema_ownership",
            ))?,
        },
    };
    Ok(ExistingManagedSchemaOwnership {
        owner_id: record.owner_id.clone(),
        object,
    })
}

fn required_column(record: &ManagedSchemaOwnershipRecord) -> Result<String, anyhow::Error> {
    record.physical_column.clone().ok_or_else(|| {
        crate::errors::ControlPlaneError::Conflict("plugin_managed_schema_ownership").into()
    })
}

fn repository_plan(effective: &EffectiveManagedSchemaPlan) -> ManagedSchemaPlan {
    ManagedSchemaPlan {
        owner_id: effective.owner().stable_id(),
        owner_version: effective.owner().plugin_version.clone(),
        fingerprint: effective.fingerprint().to_string(),
        lock_timeout_ms: DEFAULT_SCHEMA_LOCK_TIMEOUT_MS,
        operations: effective
            .entries()
            .iter()
            .map(repository_operation)
            .collect(),
    }
}

fn repository_operation(
    entry: &plugin_framework::ManagedSchemaPlanEntry,
) -> ManagedSchemaOperation {
    if entry.action == ManagedSchemaAction::RetainInactive {
        return ManagedSchemaOperation::RetainInactive {
            ownership_key: entry.object.ownership_key(),
        };
    }
    match &entry.object {
        ManagedSchemaObject::OwnedCollection {
            logical_collection,
            physical_table,
        } => ManagedSchemaOperation::EnsureOwnedCollection {
            logical_collection: logical_collection.clone(),
            physical_table: physical_table.clone(),
        },
        ManagedSchemaObject::OwnedField {
            logical_collection,
            physical_table,
            logical_field,
            physical_column,
            field_type,
            nullable,
        } => ManagedSchemaOperation::EnsureOwnedField {
            logical_collection: logical_collection.clone(),
            logical_field: logical_field.clone(),
            physical_table: physical_table.clone(),
            physical_column: physical_column.clone(),
            field_type: repository_field_type(*field_type),
            nullable: *nullable,
        },
        ManagedSchemaObject::ExtensionField {
            target_table,
            logical_field,
            physical_column,
            field_type,
        } => ManagedSchemaOperation::EnsureExtensionField {
            target_table: target_table.clone(),
            logical_field: logical_field.clone(),
            physical_column: physical_column.clone(),
            field_type: repository_field_type(*field_type),
        },
    }
}

fn repository_field_type(value: PluginDataFieldType) -> ManagedSchemaFieldType {
    match value {
        PluginDataFieldType::String => ManagedSchemaFieldType::String,
        PluginDataFieldType::Text => ManagedSchemaFieldType::Text,
        PluginDataFieldType::Number => ManagedSchemaFieldType::Number,
        PluginDataFieldType::Boolean => ManagedSchemaFieldType::Boolean,
        PluginDataFieldType::Datetime => ManagedSchemaFieldType::Datetime,
        PluginDataFieldType::Json => ManagedSchemaFieldType::Json,
        PluginDataFieldType::Uuid => ManagedSchemaFieldType::Uuid,
    }
}

fn plugin_field_type(value: ManagedSchemaFieldType) -> PluginDataFieldType {
    match value {
        ManagedSchemaFieldType::String => PluginDataFieldType::String,
        ManagedSchemaFieldType::Text => PluginDataFieldType::Text,
        ManagedSchemaFieldType::Number => PluginDataFieldType::Number,
        ManagedSchemaFieldType::Boolean => PluginDataFieldType::Boolean,
        ManagedSchemaFieldType::Datetime => PluginDataFieldType::Datetime,
        ManagedSchemaFieldType::Json => PluginDataFieldType::Json,
        ManagedSchemaFieldType::Uuid => PluginDataFieldType::Uuid,
    }
}

#[cfg(test)]
#[path = "_tests/schema.rs"]
mod tests;
