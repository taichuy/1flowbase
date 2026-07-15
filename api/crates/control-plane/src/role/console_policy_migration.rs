use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::Display,
};

use access_control::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory,
    ConsolePolicyGroup as RegisteredConsolePolicyGroup, ResourceAccessScopeKind,
    SettingsFeatureLifecycle,
};
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    RoleConsoleGroupPolicy, RoleConsolePolicy,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const LEGACY_CATALOG_SCHEMA_VERSION: &str = "1flowbase.role-console-policy-catalog/v1";
const LEGACY_MAPPING_SCHEMA_VERSION: &str = "1flowbase.role-console-policy-mapping/v1";

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

    pub fn project_legacy_role(
        &self,
        role_id: Uuid,
        legacy_grants: &[String],
    ) -> Result<ConsolePolicyMigrationPreview, ConsolePolicyMigrationError> {
        project_legacy_role_console_policy_with_explicit_mappings(
            role_id,
            legacy_grants,
            self.catalog(),
            &self.mappings,
        )
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
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for ConsolePolicyMigrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ConsolePolicyMigrationError {}

type CatalogGroupIndex = BTreeMap<ConsolePolicyGroup, Vec<ConsoleOperationPolicy>>;
type CatalogOperationIndex =
    BTreeMap<ConsoleOperationId, (ConsolePolicyGroup, ConsoleOperationPolicy)>;

pub fn compile_console_policy_migration_plan(
    inventory: &ConsoleOperationCompiledInventory,
    mappings: &[ConsolePolicyMigrationLegacyGrantMapping],
) -> Result<CompiledConsolePolicyMigrationPlan, ConsolePolicyMigrationError> {
    let catalog = compiled_catalog_from_inventory(inventory)?;
    compile_console_policy_migration_plan_with_schema(inventory.schema_version, catalog, mappings)
}

pub fn compile_console_policy_migration_plan_from_catalog(
    catalog: CompiledConsolePolicyCatalog,
    mappings: &[ConsolePolicyMigrationLegacyGrantMapping],
) -> Result<CompiledConsolePolicyMigrationPlan, ConsolePolicyMigrationError> {
    compile_console_policy_migration_plan_with_schema(
        LEGACY_CATALOG_SCHEMA_VERSION,
        catalog,
        mappings,
    )
}

fn compile_console_policy_migration_plan_with_schema(
    schema_version: &str,
    catalog: CompiledConsolePolicyCatalog,
    mappings: &[ConsolePolicyMigrationLegacyGrantMapping],
) -> Result<CompiledConsolePolicyMigrationPlan, ConsolePolicyMigrationError> {
    if schema_version.is_empty() || schema_version.trim() != schema_version {
        return Err(ConsolePolicyMigrationError::new(
            "compiled inventory schema version is invalid",
        ));
    }
    let (catalog_groups, catalog_operations) = canonical_catalog_indexes(&catalog)?;
    let canonical_catalog = CompiledConsolePolicyCatalog {
        complete: true,
        groups: catalog_groups
            .into_iter()
            .map(|(group, full_operations)| CompiledConsolePolicyGroup {
                group,
                full_operations,
            })
            .collect(),
    };
    let canonical_mappings = canonical_explicit_mappings(&catalog_operations, mappings)?;
    let catalog_fingerprint = fingerprint(
        "catalog",
        &CatalogFingerprintPayload {
            schema_version,
            catalog: &canonical_catalog,
        },
    )?;
    let mapping_fingerprint = fingerprint(
        "mapping",
        &MappingFingerprintPayload {
            schema_version: LEGACY_MAPPING_SCHEMA_VERSION,
            mappings: &canonical_mappings,
        },
    )?;
    Ok(CompiledConsolePolicyMigrationPlan {
        inventory: CompiledConsolePolicyMigrationInventory {
            catalog: canonical_catalog,
            catalog_fingerprint,
        },
        mappings: canonical_mappings,
        mapping_fingerprint,
    })
}

#[derive(Serialize)]
struct CatalogFingerprintPayload<'a> {
    schema_version: &'a str,
    catalog: &'a CompiledConsolePolicyCatalog,
}

#[derive(Serialize)]
struct MappingFingerprintPayload<'a> {
    schema_version: &'a str,
    mappings: &'a [ConsolePolicyMigrationLegacyGrantMapping],
}

fn fingerprint(
    kind: &str,
    payload: &impl Serialize,
) -> Result<String, ConsolePolicyMigrationError> {
    let payload = serde_json::to_vec(payload).map_err(|error| {
        ConsolePolicyMigrationError::new(format!(
            "cannot serialize canonical {kind} fingerprint: {error}"
        ))
    })?;
    let digest = Sha256::digest(payload);
    Ok(format!("sha256:{digest:x}"))
}

fn compiled_catalog_from_inventory(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledConsolePolicyCatalog, ConsolePolicyMigrationError> {
    let mut resources = BTreeMap::new();
    for resource in &inventory.resources {
        if resource.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(ConsolePolicyMigrationError::new(format!(
                "compiled resource {} is inactive",
                resource.resource_code
            )));
        }
        if resources
            .insert(resource.resource_code.as_str(), resource)
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "duplicate compiled resource {}",
                resource.resource_code
            )));
        }
    }

    let mut groups =
        BTreeMap::<ConsolePolicyGroup, BTreeMap<ConsoleOperationId, ConsoleOperationPolicy>>::new();
    for operation in &inventory.operations {
        if operation.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(ConsolePolicyMigrationError::new(format!(
                "compiled operation {} is inactive",
                operation.operation_id
            )));
        }
        let group = match &operation.policy_group {
            RegisteredConsolePolicyGroup::SettingsFeature(feature_id) => {
                ConsolePolicyGroup::settings_feature(feature_id).map_err(|_| {
                    ConsolePolicyMigrationError::new("compiled settings feature group is invalid")
                })?
            }
            RegisteredConsolePolicyGroup::Other(group_id) => ConsolePolicyGroup::other(group_id)
                .map_err(|_| ConsolePolicyMigrationError::new("compiled Other group is invalid"))?,
        };
        let operation_id =
            ConsoleOperationId::try_from(operation.operation_id.as_str()).map_err(|_| {
                ConsolePolicyMigrationError::new(format!(
                    "compiled operation {} is invalid",
                    operation.operation_id
                ))
            })?;
        let full_operation = match &operation.authorization {
            ConsoleAuthorization::Authenticated => continue,
            ConsoleAuthorization::Simple => {
                ConsoleOperationPolicy::simple(operation_id.clone(), true)
            }
            ConsoleAuthorization::ResourceAction {
                resource_code,
                action_code,
            } => {
                let resource = resources.get(resource_code.as_str()).ok_or_else(|| {
                    ConsolePolicyMigrationError::new(format!(
                        "compiled operation {} references an unknown resource {}",
                        operation.operation_id, resource_code
                    ))
                })?;
                if resource.scope_kind != ResourceAccessScopeKind::Workspace
                    || resource.scope_field.as_deref() != Some("scope_id")
                    || resource.owner_field.as_deref() != Some("created_by")
                {
                    return Err(ConsolePolicyMigrationError::new(format!(
                        "compiled resource {} does not support console own/scope_all authorization",
                        resource_code
                    )));
                }
                if !resource
                    .actions
                    .iter()
                    .any(|action| action.action_code == *action_code)
                {
                    return Err(ConsolePolicyMigrationError::new(format!(
                        "compiled operation {} references an unknown action {}.{}",
                        operation.operation_id, resource_code, action_code
                    )));
                }
                ConsoleOperationPolicy::row(
                    operation_id.clone(),
                    ConsoleOperationRowScope::ScopeAll,
                )
            }
        };
        if groups
            .entry(group)
            .or_default()
            .insert(operation_id.clone(), full_operation)
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "duplicate compiled operation {}",
                operation_id.as_str()
            )));
        }
    }
    if groups.is_empty() {
        return Err(ConsolePolicyMigrationError::new(
            "compiled inventory has no configurable console operations",
        ));
    }
    Ok(CompiledConsolePolicyCatalog {
        complete: true,
        groups: groups
            .into_iter()
            .map(|(group, operations)| CompiledConsolePolicyGroup {
                group,
                full_operations: operations.into_values().collect(),
            })
            .collect(),
    })
}

fn canonical_catalog_indexes(
    catalog: &CompiledConsolePolicyCatalog,
) -> Result<(CatalogGroupIndex, CatalogOperationIndex), ConsolePolicyMigrationError> {
    if !catalog.complete {
        return Err(ConsolePolicyMigrationError::new(
            "operation catalog is incomplete",
        ));
    }
    let mut catalog_groups = BTreeMap::new();
    let mut catalog_operations = BTreeMap::new();
    for group in &catalog.groups {
        let mut full_operations = group.full_operations.clone();
        full_operations.sort_by(|left, right| left.operation_id().cmp(right.operation_id()));
        if full_operations.is_empty() {
            return Err(ConsolePolicyMigrationError::new(format!(
                "compiled policy group {}:{} has no operations",
                group.group.kind().as_str(),
                group.group.group_id().as_str()
            )));
        }
        if catalog_groups
            .insert(group.group.clone(), full_operations.clone())
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "duplicate compiled policy group {}:{}",
                group.group.kind().as_str(),
                group.group.group_id().as_str()
            )));
        }
        for operation in &full_operations {
            validate_full_operation(operation)?;
            if catalog_operations
                .insert(
                    operation.operation_id().clone(),
                    (group.group.clone(), operation.clone()),
                )
                .is_some()
            {
                return Err(ConsolePolicyMigrationError::new(format!(
                    "duplicate compiled operation {}",
                    operation.operation_id().as_str()
                )));
            }
        }
    }
    if catalog_groups.is_empty() {
        return Err(ConsolePolicyMigrationError::new(
            "operation catalog has no policy groups",
        ));
    }
    Ok((catalog_groups, catalog_operations))
}

fn canonical_explicit_mappings(
    catalog_operations: &CatalogOperationIndex,
    mappings: &[ConsolePolicyMigrationLegacyGrantMapping],
) -> Result<Vec<ConsolePolicyMigrationLegacyGrantMapping>, ConsolePolicyMigrationError> {
    let mut canonical = BTreeMap::new();
    for mapping in mappings {
        validate_legacy_grant_code(&mapping.legacy_grant)?;
        let projection = match &mapping.projection {
            ConsolePolicyMigrationLegacyGrantProjection::Operations(operations) => {
                if operations.is_empty() {
                    return Err(ConsolePolicyMigrationError::new(format!(
                        "legacy mapping for {} has no operations",
                        mapping.legacy_grant
                    )));
                }
                let mut operations = operations.clone();
                operations.sort_by(|left, right| left.operation_id().cmp(right.operation_id()));
                let mut operation_ids = BTreeSet::new();
                for operation in &operations {
                    if !operation_ids.insert(operation.operation_id().clone()) {
                        return Err(ConsolePolicyMigrationError::new(format!(
                            "legacy mapping for {} repeats operation {}",
                            mapping.legacy_grant,
                            operation.operation_id().as_str()
                        )));
                    }
                    validate_mapping_operation(catalog_operations, operation)?;
                }
                ConsolePolicyMigrationLegacyGrantProjection::Operations(operations)
            }
            ConsolePolicyMigrationLegacyGrantProjection::NoProjection { evidence } => {
                if evidence.is_empty() || evidence.trim() != evidence {
                    return Err(ConsolePolicyMigrationError::new(format!(
                        "legacy no-projection mapping for {} lacks evidence",
                        mapping.legacy_grant
                    )));
                }
                ConsolePolicyMigrationLegacyGrantProjection::NoProjection {
                    evidence: evidence.clone(),
                }
            }
        };
        if canonical
            .insert(
                mapping.legacy_grant.clone(),
                ConsolePolicyMigrationLegacyGrantMapping {
                    legacy_grant: mapping.legacy_grant.clone(),
                    projection,
                },
            )
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "ambiguous legacy mapping for {}",
                mapping.legacy_grant
            )));
        }
    }
    Ok(canonical.into_values().collect())
}

fn validate_legacy_grant_code(legacy_grant: &str) -> Result<(), ConsolePolicyMigrationError> {
    if legacy_grant.is_empty() || legacy_grant.trim() != legacy_grant {
        return Err(ConsolePolicyMigrationError::new(
            "legacy mapping contains an invalid grant code",
        ));
    }
    Ok(())
}

fn validate_mapping_operation(
    catalog_operations: &CatalogOperationIndex,
    operation: &ConsoleOperationPolicy,
) -> Result<(), ConsolePolicyMigrationError> {
    let Some((_, full_operation)) = catalog_operations.get(operation.operation_id()) else {
        return Err(ConsolePolicyMigrationError::new(format!(
            "legacy mapping references unknown operation {}",
            operation.operation_id().as_str()
        )));
    };
    if !operation.same_kind(full_operation) {
        return Err(ConsolePolicyMigrationError::new(format!(
            "legacy mapping changes policy kind for {}",
            operation.operation_id().as_str()
        )));
    }
    validate_granted_operation(operation)
}

pub fn project_legacy_role_console_policy(
    role_id: Uuid,
    legacy_grants: &[String],
    catalog: &CompiledConsolePolicyCatalog,
    mappings: &[LegacyConsoleGrantMapping],
) -> Result<ConsolePolicyMigrationPreview, ConsolePolicyMigrationError> {
    let explicit_mappings = mappings
        .iter()
        .map(|mapping| ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: mapping.legacy_grant.clone(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(
                mapping.operations.clone(),
            ),
        })
        .collect::<Vec<_>>();
    project_legacy_role_console_policy_with_explicit_mappings(
        role_id,
        legacy_grants,
        catalog,
        &explicit_mappings,
    )
}

fn project_legacy_role_console_policy_with_explicit_mappings(
    role_id: Uuid,
    legacy_grants: &[String],
    catalog: &CompiledConsolePolicyCatalog,
    mappings: &[ConsolePolicyMigrationLegacyGrantMapping],
) -> Result<ConsolePolicyMigrationPreview, ConsolePolicyMigrationError> {
    let (catalog_groups, catalog_operations) = canonical_catalog_indexes(catalog)?;
    let mappings = canonical_explicit_mappings(&catalog_operations, mappings)?;
    let mapping_by_grant = mappings
        .iter()
        .map(|mapping| (mapping.legacy_grant.as_str(), mapping))
        .collect::<BTreeMap<_, _>>();

    let source_grants = legacy_grants.iter().cloned().collect::<BTreeSet<_>>();
    let mut projected_operations = BTreeMap::new();
    for legacy_grant in &source_grants {
        let mapping = mapping_by_grant.get(legacy_grant.as_str()).ok_or_else(|| {
            ConsolePolicyMigrationError::new(format!("unknown legacy grant {legacy_grant}"))
        })?;
        let ConsolePolicyMigrationLegacyGrantProjection::Operations(operations) =
            &mapping.projection
        else {
            continue;
        };
        for operation in operations {
            let merged = projected_operations
                .get(operation.operation_id())
                .map(|existing| merge_granted_operations(existing, operation))
                .transpose()?
                .unwrap_or_else(|| operation.clone());
            projected_operations.insert(operation.operation_id().clone(), merged);
        }
    }

    let mut group_policies = Vec::with_capacity(catalog_groups.len());
    for (group, full_operations) in &catalog_groups {
        let full_by_id = operation_map(full_operations);
        let explicit = projected_operations
            .values()
            .filter(|operation| {
                catalog_operations
                    .get(operation.operation_id())
                    .is_some_and(|(operation_group, _)| operation_group == group)
            })
            .cloned()
            .collect::<Vec<_>>();
        let explicit_by_id = operation_map(&explicit);
        if explicit.is_empty() {
            group_policies.push(RoleConsoleGroupPolicy::disabled(group.clone()));
        } else if explicit_by_id == full_by_id {
            group_policies.push(RoleConsoleGroupPolicy::full(group.clone()));
        } else {
            group_policies.push(RoleConsoleGroupPolicy::custom(group.clone(), explicit));
        }
    }

    let policy = RoleConsolePolicy::new(role_id, group_policies);
    let effective_after = expand_policy(&policy, &catalog_groups);
    let added = effective_after
        .iter()
        .filter(|(operation_id, policy)| projected_operations.get(*operation_id) != Some(*policy))
        .map(|(_, policy)| policy.clone())
        .collect();
    let removed = projected_operations
        .iter()
        .filter(|(operation_id, policy)| effective_after.get(*operation_id) != Some(*policy))
        .map(|(_, policy)| policy.clone())
        .collect();

    let effective_before = effective_authorization_matrix(&projected_operations);
    let effective_after = effective_authorization_matrix(&effective_after);
    let effective_delta = effective_authorization_delta(&effective_before, &effective_after);
    if !effective_delta.is_empty() {
        return Err(ConsolePolicyMigrationError::new(
            "projected policy changes the effective authorization matrix",
        ));
    }

    Ok(ConsolePolicyMigrationPreview {
        source_grants,
        policy,
        authorization_delta: ConsolePolicyAuthorizationDelta { added, removed },
        effective_before,
        effective_after,
        effective_delta,
    })
}

fn effective_authorization_matrix(
    operations: &BTreeMap<ConsoleOperationId, ConsoleOperationPolicy>,
) -> Vec<ConsolePolicyEffectiveAuthorization> {
    operations
        .values()
        .map(|operation| match operation {
            ConsoleOperationPolicy::Simple {
                operation_id,
                enabled,
            } => ConsolePolicyEffectiveAuthorization {
                operation_id: operation_id.clone(),
                simple_enabled: Some(*enabled),
                same_scope_own: None,
                same_scope_other: None,
                cross_scope: None,
            },
            ConsoleOperationPolicy::Row {
                operation_id,
                scope,
            } => ConsolePolicyEffectiveAuthorization {
                operation_id: operation_id.clone(),
                simple_enabled: None,
                same_scope_own: Some(*scope != ConsoleOperationRowScope::Disabled),
                same_scope_other: Some(*scope == ConsoleOperationRowScope::ScopeAll),
                cross_scope: Some(false),
            },
        })
        .collect()
}

fn effective_authorization_delta(
    before: &[ConsolePolicyEffectiveAuthorization],
    after: &[ConsolePolicyEffectiveAuthorization],
) -> Vec<ConsolePolicyEffectiveAuthorizationDelta> {
    let before = before
        .iter()
        .map(|entry| (entry.operation_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .iter()
        .map(|entry| (entry.operation_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|operation_id| {
            let before_entry = before.get(&operation_id).copied();
            let after_entry = after.get(&operation_id).copied();
            (before_entry != after_entry).then(|| ConsolePolicyEffectiveAuthorizationDelta {
                operation_id,
                before: before_entry.cloned(),
                after: after_entry.cloned(),
            })
        })
        .collect()
}

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

pub fn compile_console_policy_migration_probes(
    plan: &CompiledConsolePolicyMigrationPlan,
) -> Result<Vec<ConsolePolicyMigrationProbe>, ConsolePolicyMigrationError> {
    let (_, catalog_operations) = canonical_catalog_indexes(plan.catalog())?;
    Ok(expected_actor_probes(&catalog_operations)
        .into_iter()
        .collect())
}

pub fn preview_console_policy_migration_actor_authorizations(
    plan: &CompiledConsolePolicyMigrationPlan,
    actor_probe_sets: &[ConsolePolicyMigrationActorProbeSet],
    role_previews: &[ConsolePolicyMigrationPreview],
) -> Result<Vec<ConsolePolicyMigrationActorPreview>, ConsolePolicyMigrationError> {
    let previews_by_role = role_previews
        .iter()
        .map(|preview| (preview.policy.role_id(), preview))
        .collect::<BTreeMap<_, _>>();
    if previews_by_role.len() != role_previews.len() {
        return Err(ConsolePolicyMigrationError::new(
            "role migration preview has duplicate role ids",
        ));
    }
    let (_, catalog_operations) = canonical_catalog_indexes(plan.catalog())?;
    let mut actor_previews = BTreeMap::new();
    for probe_set in actor_probe_sets {
        let binding = canonical_actor_role_binding(&probe_set.binding)?;
        if actor_previews.contains_key(&binding.actor_user_id) {
            return Err(ConsolePolicyMigrationError::new(
                "actor migration preview has duplicate actor binding",
            ));
        }
        for role_id in &binding.role_ids {
            if !previews_by_role.contains_key(role_id) {
                return Err(ConsolePolicyMigrationError::new(format!(
                    "actor migration binding references unknown role {role_id}"
                )));
            }
        }
        let probes = canonical_probe_set(&catalog_operations, &probe_set.probes)?;
        let effective_before = probes
            .iter()
            .cloned()
            .map(|probe| ConsolePolicyMigrationProbeResult {
                allowed: effective_probe_allow(&binding.role_ids, &previews_by_role, &probe, true),
                probe,
            })
            .collect::<Vec<_>>();
        let effective_after = probes
            .iter()
            .cloned()
            .map(|probe| ConsolePolicyMigrationProbeResult {
                allowed: effective_probe_allow(&binding.role_ids, &previews_by_role, &probe, false),
                probe,
            })
            .collect::<Vec<_>>();
        let effective_delta = effective_before
            .iter()
            .zip(&effective_after)
            .filter(|(before, after)| before.allowed != after.allowed)
            .map(|(before, after)| ConsolePolicyMigrationProbeDelta {
                probe: before.probe.clone(),
                before: before.allowed,
                after: after.allowed,
            })
            .collect();
        actor_previews.insert(
            binding.actor_user_id,
            ConsolePolicyMigrationActorPreview {
                binding,
                probes,
                effective_before,
                effective_after,
                effective_delta,
            },
        );
    }
    Ok(actor_previews.into_values().collect())
}

pub fn validate_console_policy_migration_actor_previews(
    plan: &CompiledConsolePolicyMigrationPlan,
    role_previews: &[ConsolePolicyMigrationPreview],
    actor_previews: &[ConsolePolicyMigrationActorPreview],
) -> Result<(), ConsolePolicyMigrationError> {
    let probe_sets = actor_previews
        .iter()
        .map(|preview| ConsolePolicyMigrationActorProbeSet {
            binding: preview.binding.clone(),
            probes: preview.probes.clone(),
        })
        .collect::<Vec<_>>();
    let expected =
        preview_console_policy_migration_actor_authorizations(plan, &probe_sets, role_previews)?;
    if expected != actor_previews {
        return Err(ConsolePolicyMigrationError::new(
            "actor migration authorization preview drift",
        ));
    }
    Ok(())
}

fn canonical_actor_role_binding(
    binding: &ConsolePolicyMigrationActorRoleBinding,
) -> Result<ConsolePolicyMigrationActorRoleBinding, ConsolePolicyMigrationError> {
    let mut role_ids = binding.role_ids.clone();
    role_ids.sort_unstable();
    role_ids.dedup();
    if role_ids.is_empty() || role_ids.len() != binding.role_ids.len() {
        return Err(ConsolePolicyMigrationError::new(
            "actor migration binding must contain unique role ids",
        ));
    }
    Ok(ConsolePolicyMigrationActorRoleBinding {
        actor_user_id: binding.actor_user_id,
        role_ids,
    })
}

fn canonical_probe_set(
    catalog_operations: &CatalogOperationIndex,
    probes: &[ConsolePolicyMigrationProbe],
) -> Result<Vec<ConsolePolicyMigrationProbe>, ConsolePolicyMigrationError> {
    let expected = expected_actor_probes(catalog_operations);
    let supplied = probes.iter().cloned().collect::<BTreeSet<_>>();
    if probes.len() != supplied.len() || supplied != expected {
        return Err(ConsolePolicyMigrationError::new(
            "actor migration probes must cover every compiled simple/create operation once and every row operation with own-row/same-scope-other/cross-scope exactly once",
        ));
    }
    let mut probes = probes.to_vec();
    probes.sort();
    for probe in &probes {
        let (_, operation) = catalog_operations.get(&probe.operation_id).ok_or_else(|| {
            ConsolePolicyMigrationError::new(format!(
                "actor migration probe references unknown operation {}",
                probe.operation_id.as_str()
            ))
        })?;
        let matches_kind = matches!(
            (operation, probe.kind),
            (
                ConsoleOperationPolicy::Simple { .. },
                ConsolePolicyMigrationProbeKind::Simple
            ) | (
                ConsoleOperationPolicy::Simple { .. },
                ConsolePolicyMigrationProbeKind::Create
            ) | (
                ConsoleOperationPolicy::Row { .. },
                ConsolePolicyMigrationProbeKind::OwnRow
            ) | (
                ConsoleOperationPolicy::Row { .. },
                ConsolePolicyMigrationProbeKind::SameScopeOther
            ) | (
                ConsoleOperationPolicy::Row { .. },
                ConsolePolicyMigrationProbeKind::CrossScope
            )
        );
        if !matches_kind {
            return Err(ConsolePolicyMigrationError::new(format!(
                "actor migration probe kind does not match operation {}",
                probe.operation_id.as_str()
            )));
        }
    }
    Ok(probes)
}

fn expected_actor_probes(
    catalog_operations: &CatalogOperationIndex,
) -> BTreeSet<ConsolePolicyMigrationProbe> {
    let mut probes = BTreeSet::new();
    for (operation_id, (_, operation)) in catalog_operations {
        match operation {
            ConsoleOperationPolicy::Simple { .. } => {
                let kind = if operation_id.as_str().rsplit('.').next() == Some("create") {
                    ConsolePolicyMigrationProbeKind::Create
                } else {
                    ConsolePolicyMigrationProbeKind::Simple
                };
                probes.insert(ConsolePolicyMigrationProbe {
                    operation_id: operation_id.clone(),
                    kind,
                });
            }
            ConsoleOperationPolicy::Row { .. } => {
                for kind in [
                    ConsolePolicyMigrationProbeKind::OwnRow,
                    ConsolePolicyMigrationProbeKind::SameScopeOther,
                    ConsolePolicyMigrationProbeKind::CrossScope,
                ] {
                    probes.insert(ConsolePolicyMigrationProbe {
                        operation_id: operation_id.clone(),
                        kind,
                    });
                }
            }
        }
    }
    probes
}

fn effective_probe_allow(
    role_ids: &[Uuid],
    previews_by_role: &BTreeMap<Uuid, &ConsolePolicyMigrationPreview>,
    probe: &ConsolePolicyMigrationProbe,
    before: bool,
) -> bool {
    role_ids.iter().any(|role_id| {
        let preview = previews_by_role
            .get(role_id)
            .expect("actor migration bindings are validated against role previews");
        let authorizations = if before {
            &preview.effective_before
        } else {
            &preview.effective_after
        };
        let entry = authorizations
            .iter()
            .find(|entry| entry.operation_id == probe.operation_id);
        match probe.kind {
            ConsolePolicyMigrationProbeKind::Simple | ConsolePolicyMigrationProbeKind::Create => {
                entry.is_some_and(|entry| entry.simple_enabled == Some(true))
            }
            ConsolePolicyMigrationProbeKind::OwnRow => {
                entry.is_some_and(|entry| entry.same_scope_own == Some(true))
            }
            ConsolePolicyMigrationProbeKind::SameScopeOther => {
                entry.is_some_and(|entry| entry.same_scope_other == Some(true))
            }
            ConsolePolicyMigrationProbeKind::CrossScope => false,
        }
    })
}

fn validate_granted_operation(
    operation: &ConsoleOperationPolicy,
) -> Result<(), ConsolePolicyMigrationError> {
    match operation {
        ConsoleOperationPolicy::Simple { enabled: true, .. }
        | ConsoleOperationPolicy::Row {
            scope: ConsoleOperationRowScope::Own | ConsoleOperationRowScope::ScopeAll,
            ..
        } => Ok(()),
        _ => Err(ConsolePolicyMigrationError::new(format!(
            "legacy grant mapping disables operation {}",
            operation.operation_id().as_str()
        ))),
    }
}

fn merge_granted_operations(
    existing: &ConsoleOperationPolicy,
    incoming: &ConsoleOperationPolicy,
) -> Result<ConsoleOperationPolicy, ConsolePolicyMigrationError> {
    match (existing, incoming) {
        (
            ConsoleOperationPolicy::Simple {
                operation_id,
                enabled: existing_enabled,
            },
            ConsoleOperationPolicy::Simple {
                enabled: incoming_enabled,
                ..
            },
        ) => Ok(ConsoleOperationPolicy::simple(
            operation_id.clone(),
            *existing_enabled || *incoming_enabled,
        )),
        (
            ConsoleOperationPolicy::Row {
                operation_id,
                scope: existing_scope,
            },
            ConsoleOperationPolicy::Row {
                scope: incoming_scope,
                ..
            },
        ) => Ok(ConsoleOperationPolicy::row(
            operation_id.clone(),
            (*existing_scope).max(*incoming_scope),
        )),
        _ => Err(ConsolePolicyMigrationError::new(format!(
            "ambiguous projected policy kind for {}",
            incoming.operation_id().as_str()
        ))),
    }
}

fn validate_full_operation(
    operation: &ConsoleOperationPolicy,
) -> Result<(), ConsolePolicyMigrationError> {
    match operation {
        ConsoleOperationPolicy::Simple { enabled: true, .. }
        | ConsoleOperationPolicy::Row {
            scope: ConsoleOperationRowScope::ScopeAll,
            ..
        } => Ok(()),
        _ => Err(ConsolePolicyMigrationError::new(format!(
            "compiled full profile contains a non-full operation {}",
            operation.operation_id().as_str()
        ))),
    }
}

fn operation_map(
    operations: &[ConsoleOperationPolicy],
) -> BTreeMap<ConsoleOperationId, ConsoleOperationPolicy> {
    operations
        .iter()
        .map(|operation| (operation.operation_id().clone(), operation.clone()))
        .collect()
}

fn expand_policy(
    policy: &RoleConsolePolicy,
    catalog_groups: &BTreeMap<ConsolePolicyGroup, Vec<ConsoleOperationPolicy>>,
) -> BTreeMap<ConsoleOperationId, ConsoleOperationPolicy> {
    let mut effective = BTreeMap::new();
    for group_policy in policy.groups() {
        match group_policy {
            RoleConsoleGroupPolicy::Disabled { .. } => {}
            RoleConsoleGroupPolicy::Full { group } => {
                if let Some(operations) = catalog_groups.get(group) {
                    effective.extend(operation_map(operations));
                }
            }
            RoleConsoleGroupPolicy::Custom { operations, .. } => {
                effective.extend(operation_map(operations));
            }
        }
    }
    effective
}
