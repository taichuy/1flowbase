use std::collections::BTreeSet;

use control_plane::ports::{
    ManagedSchemaApplyReceipt, ManagedSchemaFieldType, ManagedSchemaObjectKind,
    ManagedSchemaOperation, ManagedSchemaOwnershipRecord, ManagedSchemaPlan, ManagedSchemaPreview,
    ManagedSchemaRepository, ModelDefinitionRepository,
};
use extension_contracts::{PluginDataFieldType, PluginDataModelContribution};
use plugin_framework::{
    compile_managed_schema_plan, EffectiveManagedSchemaPlan, ExistingManagedSchemaOwnership,
    ManagedSchemaAction, ManagedSchemaObject, PluginManifestV1, PluginSchemaOwner,
};

use crate::error_response::ApiError;

use super::{
    ExtensionCenterDependencies, ManagedSchemaApplyReceiptResponse,
    ManagedSchemaPreviewEntryResponse, ManagedSchemaPreviewResponse,
};

const DEFAULT_MAX_TARGET_TABLE_BYTES: u64 = 1_073_741_824;
const DEFAULT_SCHEMA_LOCK_TIMEOUT_MS: u32 = 5_000;

#[derive(Debug, Clone)]
pub(super) struct ManagedSchemaDeclaration {
    owner: PluginSchemaOwner,
    contributions: Vec<PluginDataModelContribution>,
}

impl ManagedSchemaDeclaration {
    pub(super) fn from_manifest(manifest: &PluginManifestV1) -> Result<Option<Self>, ApiError> {
        if manifest.data_models.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            owner: PluginSchemaOwner {
                publisher_namespace: manifest.publisher_namespace.clone(),
                plugin_code: manifest.plugin_code()?.to_string(),
                plugin_version: manifest.version.clone(),
            },
            contributions: manifest.data_models.clone(),
        }))
    }

    fn retained(identity: &domain::ExtensionInstallationIdentity) -> Self {
        Self {
            owner: PluginSchemaOwner {
                publisher_namespace: identity.organization.clone(),
                plugin_code: identity.artifact_id.clone(),
                plugin_version: identity.version.clone(),
            },
            contributions: Vec::new(),
        }
    }
}

pub(super) struct PreparedManagedSchema {
    plan: ManagedSchemaPlan,
    preview: ManagedSchemaPreviewResponse,
}

impl PreparedManagedSchema {
    pub(super) fn preview(&self) -> ManagedSchemaPreviewResponse {
        self.preview.clone()
    }

    pub(super) async fn apply(
        self,
        dependencies: &ExtensionCenterDependencies,
    ) -> Result<ManagedSchemaApplyReceiptResponse, ApiError> {
        let receipt =
            ManagedSchemaRepository::apply_managed_schema(&dependencies.store, &self.plan).await?;
        Ok(receipt_response(receipt))
    }
}

pub(super) async fn prepare_managed_schema(
    dependencies: &ExtensionCenterDependencies,
    workspace_id: uuid::Uuid,
    declaration: Option<&ManagedSchemaDeclaration>,
) -> Result<Option<PreparedManagedSchema>, ApiError> {
    let Some(declaration) = declaration else {
        return Ok(None);
    };
    let registered_business_tables =
        ModelDefinitionRepository::list_model_definitions(&dependencies.store, workspace_id)
            .await?
            .into_iter()
            .map(|definition| definition.physical_table_name)
            .collect::<BTreeSet<_>>();
    let existing = ManagedSchemaRepository::list_managed_schema_ownership(&dependencies.store)
        .await?
        .iter()
        .map(existing_ownership)
        .collect::<Result<Vec<_>, _>>()?;
    let effective = compile_managed_schema_plan(
        declaration.owner.clone(),
        &declaration.contributions,
        &registered_business_tables,
        &existing,
    )
    .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("plugin_managed_schema"))?;
    let plan = repository_plan(&effective);
    let preview =
        ManagedSchemaRepository::preview_managed_schema(&dependencies.store, &plan).await?;
    Ok(Some(PreparedManagedSchema {
        plan,
        preview: preview_response(preview),
    }))
}

pub(super) async fn retain_managed_schema(
    dependencies: &ExtensionCenterDependencies,
    workspace_id: uuid::Uuid,
    identity: &domain::ExtensionInstallationIdentity,
) -> Result<Option<ManagedSchemaApplyReceiptResponse>, ApiError> {
    let declaration = ManagedSchemaDeclaration::retained(identity);
    let Some(prepared) =
        prepare_managed_schema(dependencies, workspace_id, Some(&declaration)).await?
    else {
        return Ok(None);
    };
    if prepared.plan.operations.is_empty() {
        return Ok(None);
    }
    prepared.apply(dependencies).await.map(Some)
}

fn existing_ownership(
    record: &ManagedSchemaOwnershipRecord,
) -> Result<ExistingManagedSchemaOwnership, ApiError> {
    let field_type = record.field_type.map(plugin_field_type);
    let object = match record.object_kind {
        ManagedSchemaObjectKind::OwnedCollection => ManagedSchemaObject::OwnedCollection {
            logical_collection: record.logical_name.clone(),
            physical_table: record.physical_table.clone(),
        },
        ManagedSchemaObjectKind::OwnedField => {
            let (logical_collection, logical_field) = record.logical_name.split_once('.').ok_or(
                control_plane::errors::ControlPlaneError::Conflict(
                    "plugin_managed_schema_ownership",
                ),
            )?;
            ManagedSchemaObject::OwnedField {
                logical_collection: logical_collection.to_string(),
                physical_table: record.physical_table.clone(),
                logical_field: logical_field.to_string(),
                physical_column: required_column(record)?,
                field_type: field_type.ok_or(
                    control_plane::errors::ControlPlaneError::Conflict(
                        "plugin_managed_schema_ownership",
                    ),
                )?,
                nullable: record.nullable.ok_or(
                    control_plane::errors::ControlPlaneError::Conflict(
                        "plugin_managed_schema_ownership",
                    ),
                )?,
            }
        }
        ManagedSchemaObjectKind::ExtensionField => ManagedSchemaObject::ExtensionField {
            target_table: record.physical_table.clone(),
            logical_field: record.logical_name.clone(),
            physical_column: required_column(record)?,
            field_type: field_type.ok_or(control_plane::errors::ControlPlaneError::Conflict(
                "plugin_managed_schema_ownership",
            ))?,
        },
    };
    Ok(ExistingManagedSchemaOwnership {
        owner_id: record.owner_id.clone(),
        object,
    })
}

fn required_column(record: &ManagedSchemaOwnershipRecord) -> Result<String, ApiError> {
    record.physical_column.clone().ok_or_else(|| {
        control_plane::errors::ControlPlaneError::Conflict("plugin_managed_schema_ownership").into()
    })
}

fn repository_plan(effective: &EffectiveManagedSchemaPlan) -> ManagedSchemaPlan {
    ManagedSchemaPlan {
        owner_id: effective.owner().stable_id(),
        owner_version: effective.owner().plugin_version.clone(),
        fingerprint: effective.fingerprint().to_string(),
        max_target_table_bytes: DEFAULT_MAX_TARGET_TABLE_BYTES,
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

fn preview_response(preview: ManagedSchemaPreview) -> ManagedSchemaPreviewResponse {
    ManagedSchemaPreviewResponse {
        owner_id: preview.owner_id,
        fingerprint: preview.fingerprint,
        entries: preview
            .entries
            .into_iter()
            .map(|entry| ManagedSchemaPreviewEntryResponse {
                ownership_key: entry.ownership_key,
                action: match entry.action {
                    control_plane::ports::ManagedSchemaPreviewAction::Create => "create",
                    control_plane::ports::ManagedSchemaPreviewAction::AlreadyPresent => {
                        "already_present"
                    }
                    control_plane::ports::ManagedSchemaPreviewAction::Retain => "retain",
                }
                .to_string(),
            })
            .collect(),
    }
}

fn receipt_response(receipt: ManagedSchemaApplyReceipt) -> ManagedSchemaApplyReceiptResponse {
    ManagedSchemaApplyReceiptResponse {
        receipt_id: receipt.receipt_id.to_string(),
        owner_id: receipt.owner_id,
        owner_version: receipt.owner_version,
        fingerprint: receipt.fingerprint,
        created_objects: receipt.created_objects,
        existing_objects: receipt.existing_objects,
        retained_objects: receipt.retained_objects,
        applied_at: receipt.applied_at.to_string(),
    }
}

#[cfg(test)]
mod _tests;
