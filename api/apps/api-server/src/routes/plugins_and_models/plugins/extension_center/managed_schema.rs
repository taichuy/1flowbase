use super::{
    ExtensionCenterDependencies, ManagedSchemaApplyReceiptResponse,
    ManagedSchemaPreviewEntryResponse, ManagedSchemaPreviewResponse,
};
use crate::error_response::ApiError;
api_route_tests! {
#[cfg(test)]
use control_plane::ports::ManagedSchemaOperation;
}
use control_plane::ports::{
    ManagedSchemaApplyReceipt, ManagedSchemaPlan, ManagedSchemaPreview, ManagedSchemaRepository,
};
use extension_contracts::PluginDataModelContribution;
use plugin_framework::{PluginManifestV1, PluginSchemaOwner};

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
    let (plan, preview) = control_plane::plugin_management::prepare_managed_schema_plan(
        &dependencies.store,
        workspace_id,
        declaration.owner.clone(),
        &declaration.contributions,
    )
    .await?;
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

api_route_tests! {
#[cfg(test)]
mod _tests;
}
