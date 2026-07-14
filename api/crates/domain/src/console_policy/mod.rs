use std::{error::Error, fmt::Display};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolePolicyIdentifierError {
    field: &'static str,
}

impl ConsolePolicyIdentifierError {
    fn new(field: &'static str) -> Self {
        Self { field }
    }
}

impl Display for ConsolePolicyIdentifierError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid {}", self.field)
    }
}

impl Error for ConsolePolicyIdentifierError {}

fn valid_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ConsolePolicyGroupId(String);

impl ConsolePolicyGroupId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ConsolePolicyGroupId {
    type Error = ConsolePolicyIdentifierError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        valid_stable_id(value)
            .then(|| Self(value.to_string()))
            .ok_or_else(|| ConsolePolicyIdentifierError::new("console policy group id"))
    }
}

impl TryFrom<String> for ConsolePolicyGroupId {
    type Error = ConsolePolicyIdentifierError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ConsoleOperationId(String);

impl ConsoleOperationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ConsoleOperationId {
    type Error = ConsolePolicyIdentifierError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        valid_stable_id(value)
            .then(|| Self(value.to_string()))
            .ok_or_else(|| ConsolePolicyIdentifierError::new("console operation id"))
    }
}

impl TryFrom<String> for ConsoleOperationId {
    type Error = ConsolePolicyIdentifierError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyGroupKind {
    SettingsFeature,
    Other,
}

impl ConsolePolicyGroupKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SettingsFeature => "settings_feature",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "settings_feature" => Some(Self::SettingsFeature),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ConsolePolicyGroup {
    kind: ConsolePolicyGroupKind,
    group_id: ConsolePolicyGroupId,
}

impl ConsolePolicyGroup {
    pub fn settings_feature(group_id: &str) -> Result<Self, ConsolePolicyIdentifierError> {
        Self::new(ConsolePolicyGroupKind::SettingsFeature, group_id)
    }

    pub fn other(group_id: &str) -> Result<Self, ConsolePolicyIdentifierError> {
        Self::new(ConsolePolicyGroupKind::Other, group_id)
    }

    pub fn new(
        kind: ConsolePolicyGroupKind,
        group_id: &str,
    ) -> Result<Self, ConsolePolicyIdentifierError> {
        Ok(Self {
            kind,
            group_id: ConsolePolicyGroupId::try_from(group_id)?,
        })
    }

    pub fn kind(&self) -> ConsolePolicyGroupKind {
        self.kind
    }

    pub fn group_id(&self) -> &ConsolePolicyGroupId {
        &self.group_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyMode {
    Disabled,
    Full,
    Custom,
}

impl ConsolePolicyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Full => "full",
            Self::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "disabled" => Some(Self::Disabled),
            "full" => Some(Self::Full),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleOperationRowScope {
    Disabled,
    Own,
    ScopeAll,
}

impl ConsoleOperationRowScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Own => "own",
            Self::ScopeAll => "scope_all",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "disabled" => Some(Self::Disabled),
            "own" => Some(Self::Own),
            "scope_all" => Some(Self::ScopeAll),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsoleOperationPolicy {
    Simple {
        operation_id: ConsoleOperationId,
        enabled: bool,
    },
    Row {
        operation_id: ConsoleOperationId,
        scope: ConsoleOperationRowScope,
    },
}

impl ConsoleOperationPolicy {
    pub fn simple(operation_id: ConsoleOperationId, enabled: bool) -> Self {
        Self::Simple {
            operation_id,
            enabled,
        }
    }

    pub fn row(operation_id: ConsoleOperationId, scope: ConsoleOperationRowScope) -> Self {
        Self::Row {
            operation_id,
            scope,
        }
    }

    pub fn operation_id(&self) -> &ConsoleOperationId {
        match self {
            Self::Simple { operation_id, .. } | Self::Row { operation_id, .. } => operation_id,
        }
    }

    pub fn policy_kind(&self) -> &'static str {
        match self {
            Self::Simple { .. } => "simple",
            Self::Row { .. } => "row",
        }
    }

    pub fn simple_enabled(&self) -> Option<bool> {
        match self {
            Self::Simple { enabled, .. } => Some(*enabled),
            Self::Row { .. } => None,
        }
    }

    pub fn row_scope(&self) -> Option<ConsoleOperationRowScope> {
        match self {
            Self::Simple { .. } => None,
            Self::Row { scope, .. } => Some(*scope),
        }
    }

    pub fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Simple { .. }, Self::Simple { .. }) | (Self::Row { .. }, Self::Row { .. })
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RoleConsoleGroupPolicy {
    Disabled {
        group: ConsolePolicyGroup,
    },
    Full {
        group: ConsolePolicyGroup,
    },
    Custom {
        group: ConsolePolicyGroup,
        operations: Vec<ConsoleOperationPolicy>,
    },
}

impl RoleConsoleGroupPolicy {
    pub fn disabled(group: ConsolePolicyGroup) -> Self {
        Self::Disabled { group }
    }

    pub fn full(group: ConsolePolicyGroup) -> Self {
        Self::Full { group }
    }

    pub fn custom(group: ConsolePolicyGroup, mut operations: Vec<ConsoleOperationPolicy>) -> Self {
        operations.sort_by(|left, right| left.operation_id().cmp(right.operation_id()));
        Self::Custom { group, operations }
    }

    pub fn group(&self) -> &ConsolePolicyGroup {
        match self {
            Self::Disabled { group } | Self::Full { group } | Self::Custom { group, .. } => group,
        }
    }

    pub fn mode(&self) -> ConsolePolicyMode {
        match self {
            Self::Disabled { .. } => ConsolePolicyMode::Disabled,
            Self::Full { .. } => ConsolePolicyMode::Full,
            Self::Custom { .. } => ConsolePolicyMode::Custom,
        }
    }

    pub fn operations(&self) -> &[ConsoleOperationPolicy] {
        match self {
            Self::Custom { operations, .. } => operations,
            Self::Disabled { .. } | Self::Full { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoleConsolePolicy {
    role_id: Uuid,
    groups: Vec<RoleConsoleGroupPolicy>,
}

impl RoleConsolePolicy {
    pub fn new(role_id: Uuid, mut groups: Vec<RoleConsoleGroupPolicy>) -> Self {
        groups.sort_by(|left, right| left.group().cmp(right.group()));
        Self { role_id, groups }
    }

    pub fn role_id(&self) -> Uuid {
        self.role_id
    }

    pub fn groups(&self) -> &[RoleConsoleGroupPolicy] {
        &self.groups
    }
}

pub fn effective_console_simple_operation(
    policies: &[RoleConsolePolicy],
    group: &ConsolePolicyGroup,
    operation_id: &ConsoleOperationId,
) -> bool {
    policies.iter().any(|policy| {
        policy.groups().iter().any(|group_policy| {
            if group_policy.group() != group {
                return false;
            }
            match group_policy {
                RoleConsoleGroupPolicy::Full { .. } => true,
                RoleConsoleGroupPolicy::Custom { operations, .. } => {
                    operations.iter().any(|operation| {
                        matches!(
                            operation,
                            ConsoleOperationPolicy::Simple {
                                operation_id: candidate,
                                enabled: true,
                            } if candidate == operation_id
                        )
                    })
                }
                RoleConsoleGroupPolicy::Disabled { .. } => false,
            }
        })
    })
}

pub fn effective_console_row_scope(
    policies: &[RoleConsolePolicy],
    group: &ConsolePolicyGroup,
    operation_id: &ConsoleOperationId,
) -> ConsoleOperationRowScope {
    policies
        .iter()
        .flat_map(RoleConsolePolicy::groups)
        .filter(|group_policy| group_policy.group() == group)
        .fold(
            ConsoleOperationRowScope::Disabled,
            |effective, group_policy| {
                let granted = match group_policy {
                    RoleConsoleGroupPolicy::Full { .. } => ConsoleOperationRowScope::ScopeAll,
                    RoleConsoleGroupPolicy::Custom { operations, .. } => operations
                        .iter()
                        .filter_map(|operation| match operation {
                            ConsoleOperationPolicy::Row {
                                operation_id: candidate,
                                scope,
                            } if candidate == operation_id => Some(*scope),
                            ConsoleOperationPolicy::Simple { .. }
                            | ConsoleOperationPolicy::Row { .. } => None,
                        })
                        .max()
                        .unwrap_or(ConsoleOperationRowScope::Disabled),
                    RoleConsoleGroupPolicy::Disabled { .. } => ConsoleOperationRowScope::Disabled,
                };
                effective.max(granted)
            },
        )
}
