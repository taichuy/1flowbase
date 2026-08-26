use std::{collections::BTreeSet, error::Error, fmt::Display};

use domain::{ConsoleOperationId, ConsoleOperationPolicy, ConsolePolicyGroup, RoleConsolePolicy};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledConsolePolicyCatalog {
    pub complete: bool,
    pub groups: Vec<CompiledConsolePolicyGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledConsolePolicyGroup {
    pub group: ConsolePolicyGroup,
    pub full_operations: Vec<ConsoleOperationPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyConsoleGrantMapping {
    pub legacy_grant: String,
    pub operations: Vec<ConsoleOperationPolicy>,
}

/// This is intentionally distinct from the applications-only historical helper. A generic
/// migration may retain an old grant only when its zero projection is justified explicitly;
/// treating an omitted mapping as a no-op would hide an authorization mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ConsolePolicyMigrationLegacyGrantProjection {
    Operations(Vec<ConsoleOperationPolicy>),
    NoProjection { evidence: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationLegacyGrantMapping {
    pub legacy_grant: String,
    pub projection: ConsolePolicyMigrationLegacyGrantProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledConsolePolicyMigrationInventory {
    catalog: CompiledConsolePolicyCatalog,
    catalog_fingerprint: String,
}

impl CompiledConsolePolicyMigrationInventory {
    pub fn from_compiled_parts(
        catalog: CompiledConsolePolicyCatalog,
        catalog_fingerprint: String,
    ) -> Self {
        Self {
            catalog,
            catalog_fingerprint,
        }
    }

    pub fn catalog(&self) -> &CompiledConsolePolicyCatalog {
        &self.catalog
    }

    pub fn catalog_fingerprint(&self) -> &str {
        &self.catalog_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledConsolePolicyMigrationPlan {
    inventory: CompiledConsolePolicyMigrationInventory,
    mappings: Vec<ConsolePolicyMigrationLegacyGrantMapping>,
    mapping_fingerprint: String,
}

impl CompiledConsolePolicyMigrationPlan {
    pub fn from_compiled_parts(
        inventory: CompiledConsolePolicyMigrationInventory,
        mappings: Vec<ConsolePolicyMigrationLegacyGrantMapping>,
        mapping_fingerprint: String,
    ) -> Self {
        Self {
            inventory,
            mappings,
            mapping_fingerprint,
        }
    }

    pub fn catalog(&self) -> &CompiledConsolePolicyCatalog {
        self.inventory.catalog()
    }

    pub fn catalog_fingerprint(&self) -> &str {
        self.inventory.catalog_fingerprint()
    }

    pub fn mapping_fingerprint(&self) -> &str {
        &self.mapping_fingerprint
    }

    pub fn mappings(&self) -> &[ConsolePolicyMigrationLegacyGrantMapping] {
        &self.mappings
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyAuthorizationDelta {
    pub added: Vec<ConsoleOperationPolicy>,
    pub removed: Vec<ConsoleOperationPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyEffectiveAuthorization {
    pub operation_id: ConsoleOperationId,
    pub simple_enabled: Option<bool>,
    pub same_scope_own: Option<bool>,
    pub same_scope_other: Option<bool>,
    pub cross_scope: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyEffectiveAuthorizationDelta {
    pub operation_id: ConsoleOperationId,
    pub before: Option<ConsolePolicyEffectiveAuthorization>,
    pub after: Option<ConsolePolicyEffectiveAuthorization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationPreview {
    pub source_grants: BTreeSet<String>,
    pub policy: RoleConsolePolicy,
    pub authorization_delta: ConsolePolicyAuthorizationDelta,
    pub effective_before: Vec<ConsolePolicyEffectiveAuthorization>,
    pub effective_after: Vec<ConsolePolicyEffectiveAuthorization>,
    pub effective_delta: Vec<ConsolePolicyEffectiveAuthorizationDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyMigrationError(String);

impl ConsolePolicyMigrationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for ConsolePolicyMigrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ConsolePolicyMigrationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyMigrationProbeKind {
    Simple,
    Create,
    OwnRow,
    SameScopeOther,
    CrossScope,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ConsolePolicyMigrationProbe {
    pub operation_id: ConsoleOperationId,
    pub kind: ConsolePolicyMigrationProbeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationActorRoleBinding {
    pub actor_user_id: Uuid,
    pub role_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyMigrationActorProbeSet {
    pub binding: ConsolePolicyMigrationActorRoleBinding,
    pub probes: Vec<ConsolePolicyMigrationProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationProbeResult {
    pub probe: ConsolePolicyMigrationProbe,
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationProbeDelta {
    pub probe: ConsolePolicyMigrationProbe,
    pub before: bool,
    pub after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationActorPreview {
    pub binding: ConsolePolicyMigrationActorRoleBinding,
    pub probes: Vec<ConsolePolicyMigrationProbe>,
    pub effective_before: Vec<ConsolePolicyMigrationProbeResult>,
    pub effective_after: Vec<ConsolePolicyMigrationProbeResult>,
    pub effective_delta: Vec<ConsolePolicyMigrationProbeDelta>,
}
