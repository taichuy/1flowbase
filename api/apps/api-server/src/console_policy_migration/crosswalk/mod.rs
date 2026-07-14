mod application;
mod contracts;
mod exclusions;
mod other_operations;
mod settings_features;

use std::collections::{BTreeMap, BTreeSet};

use access_control::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory, ConsolePolicyGroup,
    SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
};
use anyhow::{Result, anyhow, bail};
use control_plane::{
    ports::RoleConsolePolicyMigrationSource,
    role::console_policy_migration::{
        CompiledConsolePolicyMigrationPlan, ConsolePolicyMigrationLegacyGrantMapping,
        ConsolePolicyMigrationLegacyGrantProjection, compile_console_policy_migration_plan,
    },
};
use domain::{ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope};
use serde::Serialize;

use contracts::{
    AUTHENTICATED_OPERATION_EVIDENCE, CORE_OPERATION_GROUPS, DEFAULT_DISABLED_EVIDENCE,
    DEFAULT_DISABLED_NEW_OPERATION_IDS,
};
use exclusions::{LEGACY_NO_PROJECTIONS, LEGACY_SOURCE_RESOURCES};

/// Persisted with every rehearsal so an artifact cannot be reused under a different audit.
pub const LIVE_CORE_MIGRATION_SOURCE_CONTRACT: &str =
    "1flowbase.console-policy-live-core-crosswalk/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedAuthorization {
    Authenticated,
    Simple,
    Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedPolicyGroup {
    SettingsFeature(&'static str),
    Other(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ExpectedOperationGroup {
    group: ExpectedPolicyGroup,
    authorization: ExpectedAuthorization,
    operation_ids: &'static [&'static str],
}

pub(super) const fn core_authenticated_other(
    group_id: &'static str,
    operation_ids: &'static [&'static str],
) -> ExpectedOperationGroup {
    ExpectedOperationGroup {
        group: ExpectedPolicyGroup::Other(group_id),
        authorization: ExpectedAuthorization::Authenticated,
        operation_ids,
    }
}

pub(super) const fn core_simple_settings(
    feature_id: &'static str,
    operation_ids: &'static [&'static str],
) -> ExpectedOperationGroup {
    ExpectedOperationGroup {
        group: ExpectedPolicyGroup::SettingsFeature(feature_id),
        authorization: ExpectedAuthorization::Simple,
        operation_ids,
    }
}

pub(super) const fn core_row_settings(
    feature_id: &'static str,
    operation_ids: &'static [&'static str],
) -> ExpectedOperationGroup {
    ExpectedOperationGroup {
        group: ExpectedPolicyGroup::SettingsFeature(feature_id),
        authorization: ExpectedAuthorization::Row,
        operation_ids,
    }
}

pub(super) const fn core_simple_other(
    group_id: &'static str,
    operation_ids: &'static [&'static str],
) -> ExpectedOperationGroup {
    ExpectedOperationGroup {
        group: ExpectedPolicyGroup::Other(group_id),
        authorization: ExpectedAuthorization::Simple,
        operation_ids,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LegacyGrantMappingSpec {
    legacy_grant: &'static str,
    simple_operations: &'static [&'static str],
    own_row_operations: &'static [&'static str],
    scope_all_row_operations: &'static [&'static str],
}

pub(super) const fn legacy_mapping(
    legacy_grant: &'static str,
    simple_operations: &'static [&'static str],
    own_row_operations: &'static [&'static str],
    scope_all_row_operations: &'static [&'static str],
) -> LegacyGrantMappingSpec {
    LegacyGrantMappingSpec {
        legacy_grant,
        simple_operations,
        own_row_operations,
        scope_all_row_operations,
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LegacyNoProjectionSpec {
    legacy_grant: &'static str,
    evidence: &'static str,
}

pub(super) const fn no_projection(
    legacy_grant: &'static str,
    evidence: &'static str,
) -> LegacyNoProjectionSpec {
    LegacyNoProjectionSpec {
        legacy_grant,
        evidence,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsolePolicyMigrationOperationDispositionKind {
    Operations { legacy_grants: Vec<String> },
    NoProjection { evidence: String },
    DefaultDisabledNewOperation { evidence: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePolicyMigrationOperationDisposition {
    pub operation_id: String,
    pub policy_group_kind: String,
    pub policy_group_id: String,
    pub authorization: String,
    pub disposition: ConsolePolicyMigrationOperationDispositionKind,
}

impl ConsolePolicyMigrationOperationDisposition {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn is_default_disabled_new_operation(&self) -> bool {
        matches!(
            self.disposition,
            ConsolePolicyMigrationOperationDispositionKind::DefaultDisabledNewOperation { .. }
        )
    }

    pub fn has_legacy_grant(&self, legacy_grant: &str) -> bool {
        matches!(
            &self.disposition,
            ConsolePolicyMigrationOperationDispositionKind::Operations { legacy_grants }
                if legacy_grants.iter().any(|grant| grant == legacy_grant)
        )
    }
}

#[derive(Debug, Clone)]
pub struct CompiledCoreConsolePolicyMigration {
    plan: CompiledConsolePolicyMigrationPlan,
    source: RoleConsolePolicyMigrationSource,
    dispositions: Vec<ConsolePolicyMigrationOperationDisposition>,
    legacy_mappings: Vec<ConsolePolicyMigrationLegacyGrantMapping>,
}

impl CompiledCoreConsolePolicyMigration {
    pub fn plan(&self) -> &CompiledConsolePolicyMigrationPlan {
        &self.plan
    }

    pub fn source(&self) -> &RoleConsolePolicyMigrationSource {
        &self.source
    }

    pub fn dispositions(&self) -> &[ConsolePolicyMigrationOperationDisposition] {
        &self.dispositions
    }

    pub fn disposition(
        &self,
        operation_id: &str,
    ) -> Option<&ConsolePolicyMigrationOperationDisposition> {
        self.dispositions
            .iter()
            .find(|disposition| disposition.operation_id == operation_id)
    }

    pub fn legacy_mappings(&self) -> &[ConsolePolicyMigrationLegacyGrantMapping] {
        &self.legacy_mappings
    }
}

pub fn live_legacy_migration_source() -> RoleConsolePolicyMigrationSource {
    RoleConsolePolicyMigrationSource {
        permission_resources: LEGACY_SOURCE_RESOURCES
            .iter()
            .map(|resource| (*resource).to_string())
            .collect(),
        exact_permission_codes: Vec::new(),
    }
}

pub fn compile_core_console_policy_migration_plan(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledCoreConsolePolicyMigration> {
    let expected_operations = expected_operation_index()?;
    let live_operations = validate_live_core_inventory(inventory, &expected_operations)?;
    let (legacy_mappings, projected_grants) = compile_legacy_mappings(&expected_operations)?;
    let plan = compile_console_policy_migration_plan(inventory, &legacy_mappings)
        .map_err(|error| anyhow!("cannot compile live console-policy migration plan: {error}"))?;
    let dispositions =
        compile_operation_dispositions(&expected_operations, &live_operations, &projected_grants)?;

    Ok(CompiledCoreConsolePolicyMigration {
        plan,
        source: live_legacy_migration_source(),
        dispositions,
        legacy_mappings,
    })
}

fn expected_operation_index()
-> Result<BTreeMap<&'static str, (ExpectedPolicyGroup, ExpectedAuthorization)>> {
    let mut operations = BTreeMap::new();
    for entry in CORE_OPERATION_GROUPS {
        for operation_id in entry.operation_ids {
            if operations
                .insert(*operation_id, (entry.group, entry.authorization))
                .is_some()
            {
                bail!("ambiguous Core migration disposition for {operation_id}");
            }
        }
    }
    Ok(operations)
}

fn validate_live_core_inventory(
    inventory: &ConsoleOperationCompiledInventory,
    expected_operations: &BTreeMap<&'static str, (ExpectedPolicyGroup, ExpectedAuthorization)>,
) -> Result<BTreeMap<String, ()>> {
    let mut seen = BTreeMap::new();
    for operation in inventory
        .operations
        .iter()
        .filter(|operation| operation.lifecycle == SettingsFeatureLifecycle::Active)
    {
        match operation.owner.kind {
            SettingsFeatureOwnerKind::Core => {
                let (expected_group, expected_authorization) = expected_operations
                    .get(operation.operation_id.as_str())
                    .ok_or_else(|| {
                        anyhow!(
                            "unknown active Core console operation {} has no migration disposition",
                            operation.operation_id
                        )
                    })?;
                if !expected_group.matches(&operation.policy_group) {
                    bail!(
                        "Core migration policy-group mismatch for {}",
                        operation.operation_id
                    );
                }
                if *expected_authorization
                    != ExpectedAuthorization::from_live(&operation.authorization)
                {
                    bail!(
                        "Core migration operation-type mismatch for {}",
                        operation.operation_id
                    );
                }
                if seen.insert(operation.operation_id.clone(), ()).is_some() {
                    bail!(
                        "ambiguous active Core console operation {}",
                        operation.operation_id
                    );
                }
            }
            SettingsFeatureOwnerKind::HostExtension => bail!(
                "active HostExtension {}@{} contributes {} but has no explicit console-policy migration metadata",
                operation.owner.owner_id,
                operation.owner.version,
                operation.operation_id
            ),
        }
    }
    for operation_id in expected_operations.keys() {
        if !seen.contains_key(*operation_id) {
            bail!(
                "audited Core console operation {operation_id} is absent or inactive in the live compiled registry"
            );
        }
    }
    Ok(seen)
}

impl ExpectedPolicyGroup {
    fn matches(self, actual: &ConsolePolicyGroup) -> bool {
        match (self, actual) {
            (Self::SettingsFeature(expected), ConsolePolicyGroup::SettingsFeature(actual)) => {
                expected == actual
            }
            (Self::Other(expected), ConsolePolicyGroup::Other(actual)) => expected == actual,
            _ => false,
        }
    }

    fn kind_and_id(self) -> (&'static str, &'static str) {
        match self {
            Self::SettingsFeature(id) => ("settings_feature", id),
            Self::Other(id) => ("other", id),
        }
    }
}

impl ExpectedAuthorization {
    fn from_live(authorization: &ConsoleAuthorization) -> Self {
        match authorization {
            ConsoleAuthorization::Authenticated => Self::Authenticated,
            ConsoleAuthorization::Simple => Self::Simple,
            ConsoleAuthorization::ResourceAction { .. } => Self::Row,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Authenticated => "authenticated",
            Self::Simple => "simple",
            Self::Row => "row",
        }
    }
}

fn compile_legacy_mappings(
    expected_operations: &BTreeMap<&'static str, (ExpectedPolicyGroup, ExpectedAuthorization)>,
) -> Result<(
    Vec<ConsolePolicyMigrationLegacyGrantMapping>,
    BTreeMap<String, BTreeSet<String>>,
)> {
    let mut mappings = BTreeMap::new();
    for mapping in LEGACY_NO_PROJECTIONS {
        if mapping.evidence.trim().is_empty() {
            bail!(
                "legacy no-projection mapping {} lacks evidence",
                mapping.legacy_grant
            );
        }
        insert_legacy_mapping(
            &mut mappings,
            mapping.legacy_grant,
            ConsolePolicyMigrationLegacyGrantProjection::NoProjection {
                evidence: mapping.evidence.to_string(),
            },
        )?;
    }

    let mut projected_grants = BTreeMap::<String, BTreeSet<String>>::new();
    for mapping in application::LEGACY_OPERATION_MAPPINGS
        .iter()
        .chain(settings_features::LEGACY_OPERATION_MAPPINGS)
        .chain(other_operations::LEGACY_OPERATION_MAPPINGS)
    {
        let mut operations = Vec::new();
        for operation_id in mapping.simple_operations {
            validate_projection_operation(
                expected_operations,
                mapping.legacy_grant,
                operation_id,
                ExpectedAuthorization::Simple,
            )?;
            operations.push(simple_operation(operation_id)?);
            projected_grants
                .entry((*operation_id).to_string())
                .or_default()
                .insert(mapping.legacy_grant.to_string());
        }
        for operation_id in mapping.own_row_operations {
            validate_projection_operation(
                expected_operations,
                mapping.legacy_grant,
                operation_id,
                ExpectedAuthorization::Row,
            )?;
            operations.push(row_operation(operation_id, ConsoleOperationRowScope::Own)?);
            projected_grants
                .entry((*operation_id).to_string())
                .or_default()
                .insert(mapping.legacy_grant.to_string());
        }
        for operation_id in mapping.scope_all_row_operations {
            validate_projection_operation(
                expected_operations,
                mapping.legacy_grant,
                operation_id,
                ExpectedAuthorization::Row,
            )?;
            operations.push(row_operation(
                operation_id,
                ConsoleOperationRowScope::ScopeAll,
            )?);
            projected_grants
                .entry((*operation_id).to_string())
                .or_default()
                .insert(mapping.legacy_grant.to_string());
        }
        if operations.is_empty() {
            bail!("legacy mapping {} has no projection", mapping.legacy_grant);
        }
        insert_legacy_mapping(
            &mut mappings,
            mapping.legacy_grant,
            ConsolePolicyMigrationLegacyGrantProjection::Operations(operations),
        )?;
    }

    Ok((mappings.into_values().collect(), projected_grants))
}

fn validate_projection_operation(
    expected_operations: &BTreeMap<&'static str, (ExpectedPolicyGroup, ExpectedAuthorization)>,
    legacy_grant: &str,
    operation_id: &str,
    expected_authorization: ExpectedAuthorization,
) -> Result<()> {
    let (_, actual_authorization) = expected_operations.get(operation_id).ok_or_else(|| {
        anyhow!("legacy mapping {legacy_grant} references unknown live operation {operation_id}")
    })?;
    if *actual_authorization != expected_authorization {
        bail!("legacy mapping {legacy_grant} changes operation type for {operation_id}");
    }
    if DEFAULT_DISABLED_NEW_OPERATION_IDS.contains(&operation_id) {
        bail!("legacy mapping {legacy_grant} grants default-disabled new operation {operation_id}");
    }
    Ok(())
}

fn insert_legacy_mapping(
    mappings: &mut BTreeMap<String, ConsolePolicyMigrationLegacyGrantMapping>,
    legacy_grant: &str,
    projection: ConsolePolicyMigrationLegacyGrantProjection,
) -> Result<()> {
    if mappings
        .insert(
            legacy_grant.to_string(),
            ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: legacy_grant.to_string(),
                projection,
            },
        )
        .is_some()
    {
        bail!("ambiguous legacy mapping for {legacy_grant}");
    }
    Ok(())
}

fn simple_operation(operation_id: &str) -> Result<ConsoleOperationPolicy> {
    let operation_id = ConsoleOperationId::try_from(operation_id)
        .map_err(|_| anyhow!("invalid audited console operation id {operation_id}"))?;
    Ok(ConsoleOperationPolicy::simple(operation_id, true))
}

fn row_operation(
    operation_id: &str,
    scope: ConsoleOperationRowScope,
) -> Result<ConsoleOperationPolicy> {
    let operation_id = ConsoleOperationId::try_from(operation_id)
        .map_err(|_| anyhow!("invalid audited console operation id {operation_id}"))?;
    Ok(ConsoleOperationPolicy::row(operation_id, scope))
}

fn compile_operation_dispositions(
    expected_operations: &BTreeMap<&'static str, (ExpectedPolicyGroup, ExpectedAuthorization)>,
    live_operations: &BTreeMap<String, ()>,
    projected_grants: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<ConsolePolicyMigrationOperationDisposition>> {
    let mut dispositions = Vec::with_capacity(expected_operations.len());
    for (operation_id, (group, authorization)) in expected_operations {
        if !live_operations.contains_key(*operation_id) {
            bail!("missing live operation disposition for {operation_id}");
        }
        let (policy_group_kind, policy_group_id) = group.kind_and_id();
        let disposition = if *authorization == ExpectedAuthorization::Authenticated {
            ConsolePolicyMigrationOperationDispositionKind::NoProjection {
                evidence: AUTHENTICATED_OPERATION_EVIDENCE.to_string(),
            }
        } else if DEFAULT_DISABLED_NEW_OPERATION_IDS.contains(operation_id) {
            ConsolePolicyMigrationOperationDispositionKind::DefaultDisabledNewOperation {
                evidence: DEFAULT_DISABLED_EVIDENCE.to_string(),
            }
        } else {
            let legacy_grants = projected_grants
                .get(*operation_id)
                .ok_or_else(|| {
                    anyhow!(
                        "active configurable operation {operation_id} has no migration disposition"
                    )
                })?
                .iter()
                .cloned()
                .collect();
            ConsolePolicyMigrationOperationDispositionKind::Operations { legacy_grants }
        };
        dispositions.push(ConsolePolicyMigrationOperationDisposition {
            operation_id: (*operation_id).to_string(),
            policy_group_kind: policy_group_kind.to_string(),
            policy_group_id: policy_group_id.to_string(),
            authorization: authorization.as_str().to_string(),
            disposition,
        });
    }
    Ok(dispositions)
}
