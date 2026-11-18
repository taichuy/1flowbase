use std::collections::{BTreeMap, BTreeSet};

use access_control::ConsoleRouteBinding;
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    ConsolePolicyGroupKind, ConsolePolicyStrategy, RoleConsoleGroupPolicy, RoleConsolePolicy,
};

use crate::errors::ControlPlaneError;

use super::{ConsolePolicyGroupInput, ConsolePolicyOperationInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyCatalog {
    pub schema_version: String,
    pub locale: String,
    pub settings_order_revision: i64,
    pub group_strategy_options: Vec<ConsolePolicyCatalogOption>,
    pub groups: Vec<ConsolePolicyCatalogGroup>,
    pub resources: Vec<ConsolePolicyCatalogResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyCatalogOption {
    pub value: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyCatalogGroup {
    pub settings_feature_id: Option<String>,
    pub kind: ConsolePolicyGroupKind,
    pub group_id: String,
    pub label: String,
    pub description: String,
    pub operations: Vec<ConsolePolicyCatalogOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyCatalogOperation {
    pub operation_id: String,
    pub summary: String,
    pub description: String,
    pub order: i32,
    pub route: ConsoleRouteBinding,
    pub full_profile: ConsolePolicyCatalogFullProfile,
    pub allowed_row_scopes: Vec<ConsolePolicyCatalogOption>,
    pub authorization: ConsolePolicyAuthorization,
}

/// The locale-independent policy materialized when a group is set to `full`.
///
/// This is deliberately separate from the localized options catalog so API clients cannot infer
/// the full semantics from presentation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsolePolicyCatalogFullProfile {
    Simple { enabled: bool },
    Row { scope: ConsoleOperationRowScope },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsolePolicyAuthorization {
    Simple,
    ResourceAction {
        resource_code: String,
        action_code: String,
    },
}

impl ConsolePolicyAuthorization {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::ResourceAction { .. } => "resource_action",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyCatalogResource {
    pub resource_code: String,
    pub label: String,
    pub description: String,
    pub actions: Vec<ConsolePolicyCatalogAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyCatalogAction {
    pub action_code: String,
    pub label: String,
    pub description: String,
}

pub(super) type ConsolePolicyGroupKey = (String, String);
pub(super) type CompiledConsolePolicyOperationIndex =
    BTreeMap<ConsolePolicyGroupKey, BTreeMap<String, ConsolePolicyCatalogFullProfile>>;

pub(super) fn role_console_policy_groups_from_input(
    inputs: &[ConsolePolicyGroupInput],
    operation_index: &CompiledConsolePolicyOperationIndex,
) -> Result<Vec<RoleConsoleGroupPolicy>, ControlPlaneError> {
    let mut seen_groups = BTreeSet::new();
    let mut groups = Vec::with_capacity(operation_index.len());

    for input in inputs {
        let group = group_from_input(input)?;
        let group_key = group_key(&group);
        let expected_operations = operation_index
            .get(&group_key)
            .ok_or(ControlPlaneError::InvalidInput("console_policy_group"))?;
        if !seen_groups.insert(group_key) {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_group_duplicate",
            ));
        }

        let strategy = ConsolePolicyStrategy::parse(&input.strategy)
            .ok_or(ControlPlaneError::InvalidInput("console_policy_strategy"))?;
        let operations = custom_operations_from_input(&input.operations, expected_operations)?;
        let policy = RoleConsoleGroupPolicy::new(group, input.enabled, strategy, operations);
        groups.push(policy);
    }

    for group_key in operation_index.keys() {
        if seen_groups.contains(group_key) {
            continue;
        }
        groups.push(RoleConsoleGroupPolicy::disabled(group_from_key(group_key)?));
    }

    Ok(groups)
}

pub(super) fn validate_stored_console_policy(
    policy: &RoleConsolePolicy,
    operation_index: &CompiledConsolePolicyOperationIndex,
) -> Result<(), ControlPlaneError> {
    let mut seen_groups = BTreeSet::new();
    for group_policy in policy.groups() {
        let key = group_key(group_policy.group());
        if !seen_groups.insert(key.clone()) {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_group_duplicate",
            ));
        }
        // Unpublished plugin groups remain durable but do not participate in this catalog.
        if let Some(expected) = operation_index.get(&key) {
            let active = group_policy
                .operations()
                .iter()
                .filter(|operation| expected.contains_key(operation.operation_id().as_str()))
                .cloned()
                .collect::<Vec<_>>();
            validate_custom_operations(&active, expected)?;
        }
    }
    Ok(())
}

pub(super) fn complete_stored_console_policy(
    policy: RoleConsolePolicy,
    operation_index: &CompiledConsolePolicyOperationIndex,
) -> Result<RoleConsolePolicy, ControlPlaneError> {
    validate_stored_console_policy(&policy, operation_index)?;
    let mut stored = policy
        .groups()
        .iter()
        .map(|group| (group_key(group.group()), group))
        .collect::<BTreeMap<_, _>>();
    let mut groups = Vec::with_capacity(operation_index.len());
    for (key, operations) in operation_index {
        groups.push(match stored.remove(key) {
            Some(group) => RoleConsoleGroupPolicy::new(
                group.group().clone(),
                group.enabled(),
                group.strategy(),
                group
                    .operations()
                    .iter()
                    .filter(|operation| operations.contains_key(operation.operation_id().as_str()))
                    .cloned()
                    .collect(),
            ),
            None => RoleConsoleGroupPolicy::disabled(group_from_key(key)?),
        });
    }
    Ok(RoleConsolePolicy::new(policy.role_id(), groups))
}

/// Editing the visible catalog must not erase settings belonging to stopped plugins.
pub(super) fn retain_unpublished_console_policy(
    mut groups: Vec<RoleConsoleGroupPolicy>,
    previous: &RoleConsolePolicy,
    operation_index: &CompiledConsolePolicyOperationIndex,
) -> Vec<RoleConsoleGroupPolicy> {
    for prior in previous.groups() {
        let key = group_key(prior.group());
        let Some(active) = operation_index.get(&key) else {
            groups.push(prior.clone());
            continue;
        };
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group_key(group.group()) == key)
        {
            let mut operations = group.operations().to_vec();
            operations.extend(
                prior
                    .operations()
                    .iter()
                    .filter(|operation| !active.contains_key(operation.operation_id().as_str()))
                    .cloned(),
            );
            *group = RoleConsoleGroupPolicy::new(
                group.group().clone(),
                group.enabled(),
                group.strategy(),
                operations,
            );
        }
    }
    groups
}

fn group_from_input(
    input: &ConsolePolicyGroupInput,
) -> Result<ConsolePolicyGroup, ControlPlaneError> {
    let kind = ConsolePolicyGroupKind::parse(&input.kind)
        .ok_or(ControlPlaneError::InvalidInput("console_policy_group"))?;
    ConsolePolicyGroup::new(kind, &input.group_id)
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))
}

fn group_from_key(
    group_key: &ConsolePolicyGroupKey,
) -> Result<ConsolePolicyGroup, ControlPlaneError> {
    let kind = ConsolePolicyGroupKind::parse(&group_key.0)
        .ok_or(ControlPlaneError::InvalidInput("console_policy_group"))?;
    ConsolePolicyGroup::new(kind, &group_key.1)
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))
}

fn group_key(group: &ConsolePolicyGroup) -> ConsolePolicyGroupKey {
    (
        group.kind().as_str().to_string(),
        group.group_id().as_str().to_string(),
    )
}

fn custom_operations_from_input(
    inputs: &[ConsolePolicyOperationInput],
    expected_operations: &BTreeMap<String, ConsolePolicyCatalogFullProfile>,
) -> Result<Vec<ConsoleOperationPolicy>, ControlPlaneError> {
    let mut seen_operations = BTreeSet::new();
    let mut operations = Vec::with_capacity(inputs.len());

    for input in inputs {
        let operation_id_value = match input {
            ConsolePolicyOperationInput::Simple { operation_id, .. }
            | ConsolePolicyOperationInput::Row { operation_id, .. } => operation_id,
        };
        if !seen_operations.insert(operation_id_value.as_str()) {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_operation_duplicate",
            ));
        }
        let expected_profile = expected_operations
            .get(operation_id_value)
            .ok_or(ControlPlaneError::InvalidInput("console_policy_operation"))?;
        let operation_id = ConsoleOperationId::try_from(operation_id_value.as_str())
            .map_err(|_| ControlPlaneError::InvalidInput("console_policy_operation"))?;

        let operation = match (input, expected_profile) {
            (
                ConsolePolicyOperationInput::Simple { enabled, .. },
                ConsolePolicyCatalogFullProfile::Simple { .. },
            ) => ConsoleOperationPolicy::simple(operation_id, *enabled),
            (
                ConsolePolicyOperationInput::Row { scope, .. },
                ConsolePolicyCatalogFullProfile::Row { .. },
            ) => {
                let scope = ConsoleOperationRowScope::parse(scope)
                    .ok_or(ControlPlaneError::InvalidInput("console_policy_scope"))?;
                ConsoleOperationPolicy::row(operation_id, scope)
            }
            (
                ConsolePolicyOperationInput::Simple { .. },
                ConsolePolicyCatalogFullProfile::Row { .. },
            )
            | (
                ConsolePolicyOperationInput::Row { .. },
                ConsolePolicyCatalogFullProfile::Simple { .. },
            ) => {
                return Err(ControlPlaneError::InvalidInput(
                    "console_policy_operation_type",
                ));
            }
        };
        operations.push(operation);
    }

    Ok(operations)
}

fn validate_custom_operations(
    operations: &[ConsoleOperationPolicy],
    expected_operations: &BTreeMap<String, ConsolePolicyCatalogFullProfile>,
) -> Result<(), ControlPlaneError> {
    let mut seen_operations = BTreeSet::new();
    for operation in operations {
        let operation_id = operation.operation_id().as_str();
        if !seen_operations.insert(operation_id) {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_operation_duplicate",
            ));
        }
        let expected_profile = expected_operations
            .get(operation_id)
            .ok_or(ControlPlaneError::InvalidInput("console_policy_operation"))?;
        if !matches!(
            (operation, expected_profile),
            (
                ConsoleOperationPolicy::Simple { .. },
                ConsolePolicyCatalogFullProfile::Simple { .. }
            ) | (
                ConsoleOperationPolicy::Row { .. },
                ConsolePolicyCatalogFullProfile::Row { .. }
            )
        ) {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_operation_type",
            ));
        }
    }

    Ok(())
}
