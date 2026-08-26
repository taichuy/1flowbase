use serde::Serialize;

use super::{
    ContributionDescriptor, ContributionId, ExtensionBusVersion, ExtensionPointDescriptor,
    ModuleDisableReason, ModuleId, ModuleKind, ModuleVersion,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    module_id: ModuleId,
    module_version: ModuleVersion,
    module_kind: ModuleKind,
}

impl Provenance {
    #[doc(hidden)]
    pub fn new(
        module_id: ModuleId,
        module_version: ModuleVersion,
        module_kind: ModuleKind,
    ) -> Self {
        Self {
            module_id,
            module_version,
            module_kind,
        }
    }

    pub fn module_id(&self) -> &ModuleId {
        &self.module_id
    }

    pub fn module_version(&self) -> &ModuleVersion {
        &self.module_version
    }

    pub fn module_kind(&self) -> ModuleKind {
        self.module_kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ModuleResolutionStatus {
    Active,
    Inactive { reason: ModuleInactivityReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModuleInactivityReason {
    Disabled { reason: ModuleDisableReason },
    DependencyInactive { dependency_module_id: ModuleId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleResolutionReceipt {
    provenance: Provenance,
    status: ModuleResolutionStatus,
}

impl ModuleResolutionReceipt {
    #[doc(hidden)]
    pub fn new(provenance: Provenance, status: ModuleResolutionStatus) -> Self {
        Self { provenance, status }
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    pub fn status(&self) -> &ModuleResolutionStatus {
        &self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContributionInactivityReason {
    ModuleInactive {
        reason: ModuleInactivityReason,
    },
    PointOwnerInactive {
        owner_module_id: ModuleId,
        reason: ModuleInactivityReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ContributionResolutionStatus {
    Active,
    SupersededBy {
        contribution_id: ContributionId,
    },
    Inactive {
        reason: ContributionInactivityReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContributionResolutionReceipt {
    descriptor: ContributionDescriptor,
    provenance: Provenance,
    status: ContributionResolutionStatus,
}

impl ContributionResolutionReceipt {
    #[doc(hidden)]
    pub fn new(
        descriptor: ContributionDescriptor,
        provenance: Provenance,
        status: ContributionResolutionStatus,
    ) -> Self {
        Self {
            descriptor,
            provenance,
            status,
        }
    }

    pub fn descriptor(&self) -> &ContributionDescriptor {
        &self.descriptor
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    pub fn status(&self) -> &ContributionResolutionStatus {
        &self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectiveContribution {
    descriptor: ContributionDescriptor,
    provenance: Provenance,
}

impl EffectiveContribution {
    #[doc(hidden)]
    pub fn new(descriptor: ContributionDescriptor, provenance: Provenance) -> Self {
        Self {
            descriptor,
            provenance,
        }
    }

    pub fn descriptor(&self) -> &ContributionDescriptor {
        &self.descriptor
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectiveExtensionPoint {
    descriptor: ExtensionPointDescriptor,
    provenance: Provenance,
    contributions: Vec<EffectiveContribution>,
}

impl EffectiveExtensionPoint {
    #[doc(hidden)]
    pub fn new(
        descriptor: ExtensionPointDescriptor,
        provenance: Provenance,
        contributions: Vec<EffectiveContribution>,
    ) -> Self {
        Self {
            descriptor,
            provenance,
            contributions,
        }
    }

    pub fn descriptor(&self) -> &ExtensionPointDescriptor {
        &self.descriptor
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    pub fn contributions(&self) -> &[EffectiveContribution] {
        &self.contributions
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ExtensionGraphFingerprint(String);

impl ExtensionGraphFingerprint {
    #[doc(hidden)]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveExtensionGraph {
    bus_version: ExtensionBusVersion,
    module_order: Vec<ModuleId>,
    module_provenance: Vec<Provenance>,
    module_receipts: Vec<ModuleResolutionReceipt>,
    points: Vec<EffectiveExtensionPoint>,
    contribution_receipts: Vec<ContributionResolutionReceipt>,
    fingerprint: ExtensionGraphFingerprint,
}

impl EffectiveExtensionGraph {
    #[doc(hidden)]
    pub fn new(
        bus_version: ExtensionBusVersion,
        module_order: Vec<ModuleId>,
        module_provenance: Vec<Provenance>,
        module_receipts: Vec<ModuleResolutionReceipt>,
        points: Vec<EffectiveExtensionPoint>,
        contribution_receipts: Vec<ContributionResolutionReceipt>,
        fingerprint: ExtensionGraphFingerprint,
    ) -> Self {
        Self {
            bus_version,
            module_order,
            module_provenance,
            module_receipts,
            points,
            contribution_receipts,
            fingerprint,
        }
    }

    pub fn bus_version(&self) -> ExtensionBusVersion {
        self.bus_version
    }

    pub fn module_order(&self) -> &[ModuleId] {
        &self.module_order
    }

    pub fn module_provenance(&self) -> &[Provenance] {
        &self.module_provenance
    }

    pub fn module_receipts(&self) -> &[ModuleResolutionReceipt] {
        &self.module_receipts
    }

    pub fn points(&self) -> &[EffectiveExtensionPoint] {
        &self.points
    }

    pub fn contribution_receipts(&self) -> &[ContributionResolutionReceipt] {
        &self.contribution_receipts
    }

    pub fn fingerprint(&self) -> &ExtensionGraphFingerprint {
        &self.fingerprint
    }
}
