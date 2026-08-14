use std::{path::Path, sync::Arc};

use plugin_framework::extension_bus::{
    Cardinality, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointKind, FailureSemantics,
    LifecycleSemantics, ModuleKind, PermissionCode, Provenance, ScopeSemantics,
};
use uuid::Uuid;

pub const FRONTEND_BLOCK_CONTRIBUTION_POINT_ID: &str =
    "1flowbase.workspace.frontend-block-contribution";
pub const FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID: &str = "frontend-block-contribution";
pub const FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION: &str = "1";
pub const FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION: &str = "frontend-block.ui-mount.trusted-host";
pub const FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION: &str =
    "frontend-block.ui-mount.isolated-realm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendContributionRuntimeKind {
    TrustedNative,
    Isolated,
}

impl FrontendContributionRuntimeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrustedNative => "trusted_native",
            Self::Isolated => "isolated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendContributionExecutionKind {
    UiMount,
}

impl FrontendContributionExecutionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UiMount => "ui_mount",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendContributionIsolationRequirement {
    TrustedHostRealm,
    IndependentRealm,
}

impl FrontendContributionIsolationRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrustedHostRealm => "trusted_host_realm",
            Self::IndependentRealm => "independent_realm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendContributionAssetIntegrity {
    VerifiedSha256,
}

impl FrontendContributionAssetIntegrity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedSha256 => "verified_sha256",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendContributionAssetBinding {
    pub url: String,
    pub digest: String,
    pub media_type: String,
    pub integrity: FrontendContributionAssetIntegrity,
}

#[derive(Debug, Clone)]
pub struct FrontendContributionBinding {
    pub contribution_id: String,
    pub block_id: String,
    pub block_version: String,
    pub runtime_kind: FrontendContributionRuntimeKind,
    pub execution_kind: FrontendContributionExecutionKind,
    pub isolation_requirement: FrontendContributionIsolationRequirement,
    pub assets: Vec<FrontendContributionAssetBinding>,
    pub requested_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub workspace_id: Uuid,
    pub lifecycle: LifecycleSemantics,
    pub graph_fingerprint: String,
    pub provenance: Provenance,
    pub disable_reason: Option<FrontendContributionDisableReason>,
    pub catalog_entry: domain::FrontendBlockCatalogEntry,
    graph: Arc<EffectiveExtensionGraph>,
}

impl FrontendContributionBinding {
    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendContributionDisableReason {
    VerificationInvalid,
    DesiredStateInactive,
    ArtifactUnavailable,
    VersionMismatch,
    AssignmentMissing,
    AssignmentWorkspaceMismatch,
    AssignmentStale,
    CatalogIdentityMismatch,
    UnsupportedRuntime,
    PermissionDenied,
    AssetInvalid,
}

impl FrontendContributionDisableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerificationInvalid => "verification_invalid",
            Self::DesiredStateInactive => "desired_state_inactive",
            Self::ArtifactUnavailable => "artifact_unavailable",
            Self::VersionMismatch => "version_mismatch",
            Self::AssignmentMissing => "assignment_missing",
            Self::AssignmentWorkspaceMismatch => "assignment_workspace_mismatch",
            Self::AssignmentStale => "assignment_stale",
            Self::CatalogIdentityMismatch => "catalog_identity_mismatch",
            Self::UnsupportedRuntime => "unsupported_runtime",
            Self::PermissionDenied => "permission_denied",
            Self::AssetInvalid => "asset_invalid",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrontendContributionDisabledReceipt {
    pub installation_id: Uuid,
    pub contribution_code: String,
    pub workspace_id: Uuid,
    pub graph_fingerprint: String,
    pub provenance: Provenance,
    pub reason: FrontendContributionDisableReason,
    graph: Arc<EffectiveExtensionGraph>,
}

impl FrontendContributionDisabledReceipt {
    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }
}

#[derive(Debug, Clone)]
pub enum FrontendContributionResolution {
    Active(FrontendContributionBinding),
    Disabled(FrontendContributionDisabledReceipt),
}

#[derive(Debug, Clone)]
pub struct FrontendContributionCandidate {
    pub workspace_id: Uuid,
    pub installation: domain::PluginInstallationRecord,
    pub artifact: domain::PluginArtifactInstanceRecord,
    pub assignment: Option<domain::PluginAssignmentRecord>,
    pub catalog_entry: domain::FrontendBlockCatalogEntry,
}

#[derive(Debug, Clone)]
pub struct FrontendContributionResolver {
    graph: Arc<EffectiveExtensionGraph>,
    point_provenance: Provenance,
    allowed_permissions: Vec<PermissionCode>,
}

impl FrontendContributionResolver {
    pub fn compile(graph: Arc<EffectiveExtensionGraph>) -> anyhow::Result<Self> {
        let point = graph
            .points()
            .iter()
            .find(|point| {
                point.descriptor().point_id.as_str() == FRONTEND_BLOCK_CONTRIBUTION_POINT_ID
            })
            .ok_or_else(|| anyhow::anyhow!("frontend block contribution point is absent"))?;
        let descriptor = point.descriptor();
        if descriptor.point_kind != ExtensionPointKind::Contribution
            || descriptor.contract.contract_id.as_str() != FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID
            || descriptor.contract.contract_version.as_str()
                != FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION
            || descriptor.scope != ScopeSemantics::Workspace
            || descriptor.cardinality != Cardinality::Many
            || descriptor.failure != FailureSemantics::FailClosed
            || descriptor.delivery != DeliverySemantics::Synchronous
            || descriptor.lifecycle != LifecycleSemantics::WorkspaceAssignment
        {
            anyhow::bail!("frontend block contribution point contract is incompatible");
        }
        if point.provenance().module_kind() != ModuleKind::BootCore {
            anyhow::bail!("frontend block contribution point is not owned by Boot Core");
        }
        let point_provenance = point.provenance().clone();
        let allowed_permissions = descriptor.allowed_permissions.iter().cloned().collect();
        Ok(Self {
            graph,
            point_provenance,
            allowed_permissions,
        })
    }

    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }

    pub fn resolve(
        &self,
        candidate: FrontendContributionCandidate,
    ) -> FrontendContributionResolution {
        let disabled = |reason| self.disabled_receipt(&candidate, reason);
        if candidate.installation.verification_status != domain::PluginVerificationStatus::Valid {
            return disabled(FrontendContributionDisableReason::VerificationInvalid);
        }
        if candidate.installation.desired_state != domain::PluginDesiredState::ActiveRequested {
            return disabled(FrontendContributionDisableReason::DesiredStateInactive);
        }
        if candidate.artifact.installation_id != candidate.installation.id
            || candidate.artifact.artifact_status != domain::PluginArtifactInstanceStatus::Ready
            || candidate.artifact.availability_status != domain::PluginAvailabilityStatus::Available
        {
            return disabled(FrontendContributionDisableReason::ArtifactUnavailable);
        }
        if candidate.artifact.local_version.as_deref()
            != Some(candidate.installation.plugin_version.as_str())
            || candidate
                .installation
                .expected_checksum
                .as_deref()
                .is_some_and(|expected| {
                    candidate.artifact.local_checksum.as_deref() != Some(expected)
                })
        {
            return disabled(FrontendContributionDisableReason::VersionMismatch);
        }
        if candidate.catalog_entry.installation_id != candidate.installation.id
            || candidate.catalog_entry.provider_code != candidate.installation.provider_code
            || candidate.catalog_entry.plugin_id != candidate.installation.plugin_id
            || candidate.catalog_entry.plugin_version != candidate.installation.plugin_version
        {
            return disabled(FrontendContributionDisableReason::CatalogIdentityMismatch);
        }
        if !is_system_builtin(&candidate.installation) {
            let Some(assignment) = candidate.assignment.as_ref() else {
                return disabled(FrontendContributionDisableReason::AssignmentMissing);
            };
            if assignment.workspace_id != candidate.workspace_id {
                return disabled(FrontendContributionDisableReason::AssignmentWorkspaceMismatch);
            }
            if assignment.installation_id != candidate.installation.id
                || assignment.provider_code != candidate.installation.provider_code
            {
                return disabled(FrontendContributionDisableReason::AssignmentStale);
            }
        }
        let Some((runtime_kind, isolation_requirement, requested_permission)) =
            runtime_contract(&candidate.catalog_entry.runtime)
        else {
            return disabled(FrontendContributionDisableReason::UnsupportedRuntime);
        };
        if !self
            .allowed_permissions
            .iter()
            .any(|permission| permission.as_str() == requested_permission)
        {
            return disabled(FrontendContributionDisableReason::PermissionDenied);
        }
        let Some(assets) = verified_assets(&candidate) else {
            return disabled(FrontendContributionDisableReason::AssetInvalid);
        };
        let contribution_id = format!(
            "frontend-block.{}.{}",
            candidate.installation.id, candidate.catalog_entry.contribution_code
        );
        let block_id = format!(
            "{}:{}",
            candidate.installation.id, candidate.catalog_entry.contribution_code
        );
        FrontendContributionResolution::Active(FrontendContributionBinding {
            contribution_id,
            block_id,
            block_version: candidate.catalog_entry.plugin_version.clone(),
            runtime_kind,
            execution_kind: FrontendContributionExecutionKind::UiMount,
            isolation_requirement,
            assets,
            requested_permissions: vec![requested_permission.to_string()],
            granted_permissions: vec![requested_permission.to_string()],
            workspace_id: candidate.workspace_id,
            lifecycle: LifecycleSemantics::WorkspaceAssignment,
            graph_fingerprint: self.graph.fingerprint().as_str().to_string(),
            provenance: self.point_provenance.clone(),
            disable_reason: None,
            catalog_entry: candidate.catalog_entry,
            graph: Arc::clone(&self.graph),
        })
    }

    fn disabled_receipt(
        &self,
        candidate: &FrontendContributionCandidate,
        reason: FrontendContributionDisableReason,
    ) -> FrontendContributionResolution {
        FrontendContributionResolution::Disabled(FrontendContributionDisabledReceipt {
            installation_id: candidate.installation.id,
            contribution_code: candidate.catalog_entry.contribution_code.clone(),
            workspace_id: candidate.workspace_id,
            graph_fingerprint: self.graph.fingerprint().as_str().to_string(),
            provenance: self.point_provenance.clone(),
            reason,
            graph: Arc::clone(&self.graph),
        })
    }
}

fn is_system_builtin(installation: &domain::PluginInstallationRecord) -> bool {
    installation.is_system_reserved && installation.source_kind == "builtin"
}

fn runtime_contract(
    runtime: &str,
) -> Option<(
    FrontendContributionRuntimeKind,
    FrontendContributionIsolationRequirement,
    &'static str,
)> {
    match runtime {
        "native_react" => Some((
            FrontendContributionRuntimeKind::TrustedNative,
            FrontendContributionIsolationRequirement::TrustedHostRealm,
            FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
        )),
        "isolated_iframe" => Some((
            FrontendContributionRuntimeKind::Isolated,
            FrontendContributionIsolationRequirement::IndependentRealm,
            FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
        )),
        _ => None,
    }
}

fn verified_assets(
    candidate: &FrontendContributionCandidate,
) -> Option<Vec<FrontendContributionAssetBinding>> {
    let asset_count = candidate
        .catalog_entry
        .code_modules
        .iter()
        .map(|module| module.assets.len())
        .sum::<usize>();
    let root = candidate.artifact.local_path.as_deref().map(Path::new);
    if asset_count > 0 && root.is_none() {
        return None;
    }
    let mut bindings = Vec::with_capacity(asset_count);
    for asset in candidate
        .catalog_entry
        .code_modules
        .iter()
        .flat_map(|module| module.assets.iter())
    {
        if asset.media_type.trim().is_empty()
            || asset.sha256.len() != 64
            || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return None;
        }
        let manifest = plugin_framework::FrontendModuleAssetManifest {
            path: asset.path.clone(),
            role: match asset.role {
                domain::FrontendModuleAssetRole::BrowserModule => {
                    plugin_framework::FrontendModuleAssetRoleManifest::BrowserModule
                }
                domain::FrontendModuleAssetRole::ShadowStyle => {
                    plugin_framework::FrontendModuleAssetRoleManifest::ShadowStyle
                }
                domain::FrontendModuleAssetRole::Support => {
                    plugin_framework::FrontendModuleAssetRoleManifest::Support
                }
            },
            media_type: asset.media_type.clone(),
            sha256: asset.sha256.clone(),
        };
        plugin_framework::load_frontend_module_asset(root?, &manifest).ok()?;
        bindings.push(FrontendContributionAssetBinding {
            url: format!(
                "/api/console/frontstage/{}/component-module-assets/{}",
                candidate.workspace_id, asset.sha256
            ),
            digest: asset.sha256.clone(),
            media_type: asset.media_type.clone(),
            integrity: FrontendContributionAssetIntegrity::VerifiedSha256,
        });
    }
    Some(bindings)
}
