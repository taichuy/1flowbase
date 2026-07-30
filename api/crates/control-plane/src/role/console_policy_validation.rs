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
        let group_key = group_key(group_policy.group());
        let expected_operations = operation_index
            .get(&group_key)
            .ok_or(ControlPlaneError::InvalidInput("console_policy_group"))?;
        if !seen_groups.insert(group_key) {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_group_duplicate",
            ));
        }

        validate_custom_operations(group_policy.operations(), expected_operations)?;
    }

    Ok(())
}

pub(super) fn complete_stored_console_policy(
    policy: RoleConsolePolicy,
    operation_index: &CompiledConsolePolicyOperationIndex,
) -> Result<RoleConsolePolicy, ControlPlaneError> {
    validate_stored_console_policy(&policy, operation_index)?;
    let mut policies_by_group = policy
        .groups()
        .iter()
        .cloned()
        .map(|group_policy| (group_key(group_policy.group()), group_policy))
        .collect::<BTreeMap<_, _>>();
    let mut groups = Vec::with_capacity(operation_index.len());

    for group_key in operation_index.keys() {
        let policy = policies_by_group
            .remove(group_key)
            .unwrap_or(RoleConsoleGroupPolicy::disabled(group_from_key(group_key)?));
        groups.push(policy);
    }

    Ok(RoleConsolePolicy::new(policy.role_id(), groups))
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
