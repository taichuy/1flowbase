use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::Display,
};

use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    RoleConsoleGroupPolicy, RoleConsolePolicy,
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CompiledConsolePolicyCatalog {
    pub complete: bool,
    pub groups: Vec<CompiledConsolePolicyGroup>,
}

#[derive(Debug, Clone)]
pub struct CompiledConsolePolicyGroup {
    pub group: ConsolePolicyGroup,
    pub full_operations: Vec<ConsoleOperationPolicy>,
}

#[derive(Debug, Clone)]
pub struct LegacyConsoleGrantMapping {
    pub legacy_grant: String,
    pub operations: Vec<ConsoleOperationPolicy>,
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

pub fn project_legacy_role_console_policy(
    role_id: Uuid,
    legacy_grants: &[String],
    catalog: &CompiledConsolePolicyCatalog,
    mappings: &[LegacyConsoleGrantMapping],
) -> Result<ConsolePolicyMigrationPreview, ConsolePolicyMigrationError> {
    if !catalog.complete {
        return Err(ConsolePolicyMigrationError::new(
            "operation catalog is incomplete",
        ));
    }

    let mut catalog_groups = BTreeMap::new();
    let mut catalog_operations = BTreeMap::new();
    for group in &catalog.groups {
        if catalog_groups
            .insert(group.group.clone(), group.full_operations.clone())
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "duplicate compiled policy group {}:{}",
                group.group.kind().as_str(),
                group.group.group_id().as_str()
            )));
        }
        for operation in &group.full_operations {
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

    let mut mapping_by_grant = BTreeMap::new();
    for mapping in mappings {
        if mapping.legacy_grant.is_empty() || mapping.legacy_grant.trim() != mapping.legacy_grant {
            return Err(ConsolePolicyMigrationError::new(
                "legacy mapping contains an invalid grant code",
            ));
        }
        if mapping_by_grant
            .insert(mapping.legacy_grant.as_str(), mapping)
            .is_some()
        {
            return Err(ConsolePolicyMigrationError::new(format!(
                "ambiguous legacy mapping for {}",
                mapping.legacy_grant
            )));
        }
        if mapping.operations.is_empty() {
            return Err(ConsolePolicyMigrationError::new(format!(
                "legacy mapping for {} has no operations",
                mapping.legacy_grant
            )));
        }
        for operation in &mapping.operations {
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
            validate_granted_operation(operation)?;
        }
    }

    let source_grants = legacy_grants.iter().cloned().collect::<BTreeSet<_>>();
    let mut projected_operations = BTreeMap::new();
    for legacy_grant in &source_grants {
        let mapping = mapping_by_grant.get(legacy_grant.as_str()).ok_or_else(|| {
            ConsolePolicyMigrationError::new(format!("unknown legacy grant {legacy_grant}"))
        })?;
        for operation in &mapping.operations {
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
