use serde::Serialize;

use super::{
    ContributionDescriptor, ExtensionBusVersion, ExtensionPointDescriptor, ModuleId, ModuleKind,
    ModuleVersion,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    module_id: ModuleId,
    module_version: ModuleVersion,
    module_kind: ModuleKind,
}

impl Provenance {
    pub(crate) fn new(
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
pub struct EffectiveContribution {
    descriptor: ContributionDescriptor,
    provenance: Provenance,
}

impl EffectiveContribution {
    pub(crate) fn new(descriptor: ContributionDescriptor, provenance: Provenance) -> Self {
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
    pub(crate) fn new(
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
    pub(crate) fn new(value: String) -> Self {
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
    points: Vec<EffectiveExtensionPoint>,
    fingerprint: ExtensionGraphFingerprint,
}

impl EffectiveExtensionGraph {
    pub(crate) fn new(
        bus_version: ExtensionBusVersion,
        module_order: Vec<ModuleId>,
        module_provenance: Vec<Provenance>,
        points: Vec<EffectiveExtensionPoint>,
        fingerprint: ExtensionGraphFingerprint,
    ) -> Self {
        Self {
            bus_version,
            module_order,
            module_provenance,
            points,
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

    pub fn points(&self) -> &[EffectiveExtensionPoint] {
        &self.points
    }

    pub fn fingerprint(&self) -> &ExtensionGraphFingerprint {
        &self.fingerprint
    }
}
