use std::{path::Path, sync::Arc};

use control_plane::errors::ControlPlaneError;
use plugin_framework::{
    extension_bus::{
        Cardinality, ContractVersion, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointId,
        ExtensionPointKind, FailureSemantics, LifecycleSemantics, OverridePolicy, Provenance,
        ScopeSemantics,
    },
    LegacyInstalledManifestEligibility,
};
use uuid::Uuid;

use crate::extension_bus::{
    BOOT_CORE_MODULE_ID, MODEL_PROVIDER_CONTRACT_ID, MODEL_PROVIDER_CONTRACT_VERSION,
    MODEL_PROVIDER_EXTENSION_POINT_ID,
};

#[derive(Debug, Clone)]
pub struct ModelProviderSlotResolver {
    graph: Arc<EffectiveExtensionGraph>,
}

impl ModelProviderSlotResolver {
    pub fn new(graph: Arc<EffectiveExtensionGraph>) -> Self {
        Self { graph }
    }

    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }

    pub fn graph_fingerprint(&self) -> &str {
        self.graph.fingerprint().as_str()
    }

    pub fn resolve(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<ModelProviderSlotBinding> {
        let point = self
            .graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id.as_str() == MODEL_PROVIDER_EXTENSION_POINT_ID)
            .ok_or(ControlPlaneError::Conflict(
                "model_provider_extension_slot_unavailable",
            ))?;
        let descriptor = point.descriptor();
        if descriptor.point_kind != ExtensionPointKind::Slot
            || descriptor.contract.contract_id.as_str() != MODEL_PROVIDER_CONTRACT_ID
            || descriptor.contract.contract_version.as_str() != MODEL_PROVIDER_CONTRACT_VERSION
            || descriptor.owner_module_id.as_str() != BOOT_CORE_MODULE_ID
            || descriptor.scope != ScopeSemantics::Global
            || descriptor.cardinality != Cardinality::Many
            || descriptor.lifecycle != LifecycleSemantics::RuntimeWorker
            || descriptor.delivery != DeliverySemantics::Synchronous
            || descriptor.failure != FailureSemantics::FailClosed
            || descriptor.override_policy != OverridePolicy::Sealed
        {
            return Err(ControlPlaneError::Conflict(
                "model_provider_extension_slot_contract_mismatch",
            )
            .into());
        }
        validate_dynamic_installation(installation)?;
        ModelProviderSlotBinding::from_installation(
            descriptor.point_id.clone(),
            descriptor.contract.contract_version.clone(),
            ModelProviderBindingProvenance::BootGraph(point.provenance().clone()),
            self.graph.fingerprint().as_str().to_string(),
            installation,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProviderSlotBinding {
    pub point_id: ExtensionPointId,
    pub installation_id: Uuid,
    pub plugin_id: String,
    pub provider_code: String,
    pub contract_version: ContractVersion,
    pub artifact_checksum: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub source_identity: String,
    pub source_kind: String,
    pub provenance: ModelProviderBindingProvenance,
    pub graph_fingerprint: String,
    package_root: String,
    legacy_manifest_eligibility: Option<LegacyInstalledManifestEligibility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProviderBindingProvenance {
    BootGraph(Provenance),
    LegacyTestOnly,
}

impl ModelProviderSlotBinding {
    fn from_installation(
        point_id: ExtensionPointId,
        contract_version: ContractVersion,
        provenance: ModelProviderBindingProvenance,
        graph_fingerprint: String,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<Self> {
        let package_root = installation
            .local_path()
            .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?
            .to_string();
        Ok(Self {
            point_id,
            installation_id: installation.id,
            plugin_id: installation.plugin_id.clone(),
            provider_code: installation.provider_code.clone(),
            contract_version,
            artifact_checksum: installation
                .artifact
                .local_checksum
                .clone()
                .or_else(|| installation.expected_checksum.clone()),
            manifest_fingerprint: installation.artifact.manifest_fingerprint.clone(),
            source_identity: provider_source_identity(installation),
            source_kind: installation.source_kind.clone(),
            provenance,
            graph_fingerprint,
            package_root,
            legacy_manifest_eligibility: legacy_manifest_eligibility(installation)?,
        })
    }

    pub(crate) fn legacy_for_tests(
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<Self> {
        Self::from_installation(
            ExtensionPointId::new(MODEL_PROVIDER_EXTENSION_POINT_ID)?,
            ContractVersion::new(installation.contract_version.clone())?,
            ModelProviderBindingProvenance::LegacyTestOnly,
            "legacy-test-only".to_string(),
            installation,
        )
    }

    pub(crate) fn package_root(&self) -> &str {
        &self.package_root
    }

    pub(crate) fn legacy_manifest_eligibility(
        &self,
    ) -> Option<&LegacyInstalledManifestEligibility> {
        self.legacy_manifest_eligibility.as_ref()
    }

    pub(crate) fn require_provider_code(&self, provider_code: &str) -> anyhow::Result<()> {
        if provider_code == self.provider_code {
            return Ok(());
        }
        Err(ControlPlaneError::InvalidInput("provider_code").into())
    }
}

fn validate_dynamic_installation(
    installation: &domain::LocalPluginInstallationRecord,
) -> anyhow::Result<()> {
    if installation.category != domain::ExtensionCategory::RuntimeExtensions
        || installation.contract_version != MODEL_PROVIDER_CONTRACT_VERSION
        || installation.artifact.installation_id != installation.id
    {
        return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
    }
    if installation.verification_status != domain::PluginVerificationStatus::Valid
        || installation.desired_state != domain::PluginDesiredState::ActiveRequested
        || installation.artifact.artifact_status != domain::PluginArtifactInstanceStatus::Ready
        || !installation.artifact.is_current
        || installation.availability_status() != domain::PluginAvailabilityStatus::Available
        || !matches!(
            installation.runtime_status(),
            domain::PluginRuntimeStatus::Inactive | domain::PluginRuntimeStatus::Active
        )
    {
        return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
    }
    let local_path = installation
        .local_path()
        .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?;
    if !Path::new(local_path).exists() {
        return Err(ControlPlaneError::Conflict("plugin_artifact_path_missing").into());
    }
    if installation.plugin_id.trim().is_empty() || installation.provider_code.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
    }
    Ok(())
}

fn legacy_manifest_eligibility(
    installation: &domain::LocalPluginInstallationRecord,
) -> anyhow::Result<Option<LegacyInstalledManifestEligibility>> {
    let Some(compatibility) = installation.legacy_manifest_compatibility.as_deref() else {
        return Ok(None);
    };
    if compatibility != "missing_publisher_namespace_v1" {
        return Err(
            ControlPlaneError::Conflict("plugin_manifest_compatibility_unsupported").into(),
        );
    }
    let fingerprint =
        installation
            .artifact
            .manifest_fingerprint
            .clone()
            .ok_or(ControlPlaneError::Conflict(
                "plugin_manifest_fingerprint_missing",
            ))?;
    Ok(Some(LegacyInstalledManifestEligibility {
        expected_publisher_namespace: installation.organization.clone(),
        expected_versioned_plugin_id: installation.plugin_id.clone(),
        expected_raw_manifest_fingerprint: fingerprint,
    }))
}

fn provider_source_identity(installation: &domain::LocalPluginInstallationRecord) -> String {
    format!(
        "installation_id={};checksum={};manifest_fingerprint={};updated_at={}",
        installation.id,
        installation.expected_checksum.as_deref().unwrap_or(""),
        installation
            .artifact
            .manifest_fingerprint
            .as_deref()
            .unwrap_or(""),
        installation.updated_at.unix_timestamp_nanos()
    )
}
