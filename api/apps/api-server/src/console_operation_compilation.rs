use std::collections::{BTreeMap, BTreeSet};

use access_control::{ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteBinding};
use serde::Serialize;
use thiserror::Error;

pub(crate) const COMPILED_CONSOLE_OPERATION_SNAPSHOT_SCHEMA_V1: &str =
    "1flowbase.compiled-console-operation-snapshot/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConsoleBindingOwnerKind {
    Core,
    Family,
    HostExtension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsoleOperationPolicyContribution {
    pub(crate) operation_id: String,
    pub(crate) authorization_profile_id: String,
    pub(crate) owner_id: String,
    pub(crate) owner_active: bool,
    pub(crate) policy_group: ConsolePolicyGroup,
    pub(crate) authorization: ConsoleAuthorization,
    pub(crate) routes: Vec<ConsoleRouteBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsoleBindingOwnershipContribution {
    pub(crate) contribution_id: String,
    pub(crate) owner_kind: ConsoleBindingOwnerKind,
    pub(crate) owner_id: String,
    pub(crate) owner_active: bool,
    pub(crate) interface_id: String,
    pub(crate) binding_id: String,
    pub(crate) protocol: String,
    pub(crate) route: ConsoleRouteBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ConsoleMigrationDisposition {
    LegacyOperations { legacy_grants: Vec<String> },
    NoProjection { evidence: String },
    DefaultDisabled { evidence: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsoleMigrationDispositionContribution {
    pub(crate) operation_id: String,
    pub(crate) disposition: ConsoleMigrationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CompiledConsoleBindingOwnership {
    pub(crate) contribution_id: String,
    pub(crate) owner_kind: ConsoleBindingOwnerKind,
    pub(crate) owner_id: String,
    pub(crate) interface_id: String,
    pub(crate) binding_id: String,
    pub(crate) protocol: String,
    pub(crate) route: ConsoleRouteBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CompiledConsoleOperationRecord {
    pub(crate) operation_id: String,
    pub(crate) authorization_profile_id: String,
    pub(crate) policy_group: ConsolePolicyGroup,
    pub(crate) authorization: ConsoleAuthorization,
    pub(crate) bindings: Vec<CompiledConsoleBindingOwnership>,
    pub(crate) migration: ConsoleMigrationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CompiledConsoleOperationSnapshot {
    pub(crate) schema_version: &'static str,
    pub(crate) operations: Vec<CompiledConsoleOperationRecord>,
}

impl CompiledConsoleOperationSnapshot {
    pub(crate) fn operation(&self, operation_id: &str) -> Option<&CompiledConsoleOperationRecord> {
        self.operations
            .binary_search_by(|operation| operation.operation_id.as_str().cmp(operation_id))
            .ok()
            .and_then(|index| self.operations.get(index))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("console operation compilation failed: {0}")]
pub(crate) struct ConsoleOperationCompilationError(String);

pub(crate) fn compile_console_operation_snapshot(
    policies: impl IntoIterator<Item = ConsoleOperationPolicyContribution>,
    bindings: impl IntoIterator<Item = ConsoleBindingOwnershipContribution>,
    migrations: impl IntoIterator<Item = ConsoleMigrationDispositionContribution>,
    known_binding_owners: impl IntoIterator<Item = String>,
) -> Result<CompiledConsoleOperationSnapshot, ConsoleOperationCompilationError> {
    let known_binding_owners = known_binding_owners.into_iter().collect::<BTreeSet<_>>();
    let mut findings = BTreeSet::new();
    let mut policies_by_id = BTreeMap::new();
    let mut route_owners = BTreeMap::new();

    for mut policy in policies {
        policy.routes.sort();
        if !policy.owner_active {
            findings.insert(format!("inactive operation owner {}", policy.owner_id));
        }
        for route in &policy.routes {
            let key = route_key(route);
            if let Some(existing) = route_owners.insert(key.clone(), policy.operation_id.clone()) {
                if existing != policy.operation_id {
                    findings.insert(format!(
                        "conflicting operation route {} owned by {} and {}",
                        display_route_key(&key),
                        existing,
                        policy.operation_id
                    ));
                }
            }
        }
        let operation_id = policy.operation_id.clone();
        if policies_by_id
            .insert(operation_id.clone(), policy)
            .is_some()
        {
            findings.insert(format!("duplicate operation contribution {operation_id}"));
        }
    }

    let mut bindings_by_operation = BTreeMap::<String, Vec<CompiledConsoleBindingOwnership>>::new();
    let mut binding_ids = BTreeMap::<String, String>::new();
    let mut binding_routes = BTreeMap::<(String, String), String>::new();
    for binding in bindings {
        if !binding.owner_active {
            findings.insert(format!("inactive binding owner {}", binding.owner_id));
        }
        if !known_binding_owners.contains(&binding.owner_id) {
            findings.insert(format!("unknown binding owner {}", binding.owner_id));
        }
        if let Some(existing) =
            binding_ids.insert(binding.binding_id.clone(), binding.contribution_id.clone())
        {
            findings.insert(format!(
                "duplicate binding identity {} from {} and {}",
                binding.binding_id, existing, binding.contribution_id
            ));
        }
        let route_key = route_key(&binding.route);
        let Some(operation_id) = route_owners.get(&route_key) else {
            findings.insert(format!(
                "extra binding {} for unknown route {}",
                binding.binding_id,
                display_route_key(&route_key)
            ));
            continue;
        };
        if let Some(existing) =
            binding_routes.insert(route_key.clone(), binding.contribution_id.clone())
        {
            findings.insert(format!(
                "conflicting binding owner for {}: {} and {}",
                display_route_key(&route_key),
                existing,
                binding.contribution_id
            ));
        }
        bindings_by_operation
            .entry(operation_id.clone())
            .or_default()
            .push(CompiledConsoleBindingOwnership {
                contribution_id: binding.contribution_id,
                owner_kind: binding.owner_kind,
                owner_id: binding.owner_id,
                interface_id: binding.interface_id,
                binding_id: binding.binding_id,
                protocol: binding.protocol,
                route: binding.route,
            });
    }

    for (key, operation_id) in &route_owners {
        if !binding_routes.contains_key(key) {
            findings.insert(format!(
                "missing binding owner for operation {} route {}",
                operation_id,
                display_route_key(key)
            ));
        }
    }

    let mut migrations_by_operation = BTreeMap::new();
    for migration in migrations {
        let operation_id = migration.operation_id.clone();
        if migrations_by_operation
            .insert(operation_id.clone(), migration.disposition)
            .is_some()
        {
            findings.insert(format!("duplicate migration disposition {operation_id}"));
        }
    }
    for operation_id in policies_by_id.keys() {
        if !migrations_by_operation.contains_key(operation_id) {
            findings.insert(format!("missing migration disposition {operation_id}"));
        }
    }
    for operation_id in migrations_by_operation.keys() {
        if !policies_by_id.contains_key(operation_id) {
            findings.insert(format!("extra migration disposition {operation_id}"));
        }
    }

    if !findings.is_empty() {
        return Err(ConsoleOperationCompilationError(
            findings.into_iter().collect::<Vec<_>>().join("; "),
        ));
    }

    let operations = policies_by_id
        .into_iter()
        .map(|(operation_id, policy)| {
            let mut bindings = bindings_by_operation
                .remove(&operation_id)
                .unwrap_or_default();
            bindings.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
            CompiledConsoleOperationRecord {
                operation_id: operation_id.clone(),
                authorization_profile_id: policy.authorization_profile_id,
                policy_group: policy.policy_group,
                authorization: policy.authorization,
                bindings,
                migration: migrations_by_operation
                    .remove(&operation_id)
                    .expect("validated operation must own a migration disposition"),
            }
        })
        .collect();

    Ok(CompiledConsoleOperationSnapshot {
        schema_version: COMPILED_CONSOLE_OPERATION_SNAPSHOT_SCHEMA_V1,
        operations,
    })
}

fn route_key(route: &ConsoleRouteBinding) -> (String, String) {
    (
        route.method.to_ascii_uppercase(),
        route
            .path
            .split('/')
            .map(|segment| {
                if segment.starts_with(':') || (segment.starts_with('{') && segment.ends_with('}'))
                {
                    "{}"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn display_route_key(key: &(String, String)) -> String {
    format!("{} {}", key.0, key.1)
}
