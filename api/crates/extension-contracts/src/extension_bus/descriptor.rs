use std::collections::BTreeSet;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use thiserror::Error;

macro_rules! domain_string {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, DescriptorValueError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(DescriptorValueError::Empty {
                        kind: stringify!($name),
                    });
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DescriptorValueError {
    #[error("{kind} must not be empty")]
    Empty { kind: &'static str },
}

domain_string!(ModuleId);
domain_string!(ModuleVersion);
domain_string!(ExtensionPointId);
domain_string!(ContributionId);
domain_string!(ContractId);
domain_string!(ContractVersion);
domain_string!(PermissionCode);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionBusVersion {
    V1,
}

impl ExtensionBusVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "1flowbase.extension-bus/v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    BootCore,
    TrustedHost,
    Runtime,
    Capability,
    User,
}

impl ModuleKind {
    pub fn may_define_points(self) -> bool {
        matches!(self, Self::BootCore | Self::TrustedHost)
    }

    pub fn may_override(self) -> bool {
        matches!(self, Self::BootCore | Self::TrustedHost)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BootCore => "boot_core",
            Self::TrustedHost => "trusted_host",
            Self::Runtime => "runtime",
            Self::Capability => "capability",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionPointKind {
    Slot,
    Pipeline,
    EventStream,
    Contribution,
    ResourceAction,
}

impl ExtensionPointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slot => "slot",
            Self::Pipeline => "pipeline",
            Self::EventStream => "event_stream",
            Self::Contribution => "contribution",
            Self::ResourceAction => "resource_action",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeSemantics {
    Global,
    System,
    Workspace,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    ExactlyOne,
    ZeroOrOne,
    OneOrMore,
    Many,
}

impl Cardinality {
    /// Returns whether an effective contribution count satisfies this contract.
    pub fn accepts(self, actual: usize) -> bool {
        match self {
            Self::ExactlyOne => actual == 1,
            Self::ZeroOrOne => actual <= 1,
            Self::OneOrMore => actual >= 1,
            Self::Many => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderingSemantics {
    Lexicographic,
    Dependency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureSemantics {
    FailClosed,
    IsolateContribution,
    BestEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverySemantics {
    Synchronous,
    Asynchronous,
    AfterCommitDurable,
    RequiredStream,
    DiagnosticBestEffort,
}

impl DeliverySemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synchronous => "synchronous",
            Self::Asynchronous => "asynchronous",
            Self::AfterCommitDurable => "after_commit_durable",
            Self::RequiredStream => "required_stream",
            Self::DiagnosticBestEffort => "diagnostic_best_effort",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleSemantics {
    BootSnapshot,
    Invocation,
    RuntimeWorker,
    WorkspaceAssignment,
    UiMount,
}

impl LifecycleSemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BootSnapshot => "boot_snapshot",
            Self::Invocation => "invocation",
            Self::RuntimeWorker => "runtime_worker",
            Self::WorkspaceAssignment => "workspace_assignment",
            Self::UiMount => "ui_mount",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleDisableReason {
    DeploymentPolicy,
    DesiredState,
    WorkspaceAssignment,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ModuleActivationDeclaration {
    #[default]
    Active,
    Disabled {
        reason: ModuleDisableReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverridePolicy {
    Sealed,
    TrustedHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionMode {
    Append,
    Override,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDescriptor {
    pub contract_id: ContractId,
    pub contract_version: ContractVersion,
}

impl ContractDescriptor {
    pub fn new(
        contract_id: impl Into<String>,
        contract_version: impl Into<String>,
    ) -> Result<Self, DescriptorValueError> {
        Ok(Self {
            contract_id: ContractId::new(contract_id)?,
            contract_version: ContractVersion::new(contract_version)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleDependency {
    pub module_id: ModuleId,
    pub required_version: ModuleVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributionOrdering {
    #[serde(default)]
    pub after: BTreeSet<ContributionId>,
    #[serde(default)]
    pub before: BTreeSet<ContributionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionPointDescriptor {
    pub point_id: ExtensionPointId,
    pub owner_module_id: ModuleId,
    pub point_kind: ExtensionPointKind,
    pub contract: ContractDescriptor,
    pub scope: ScopeSemantics,
    pub cardinality: Cardinality,
    pub ordering: OrderingSemantics,
    pub failure: FailureSemantics,
    pub delivery: DeliverySemantics,
    pub lifecycle: LifecycleSemantics,
    #[serde(default)]
    pub allowed_permissions: BTreeSet<PermissionCode>,
    pub override_policy: OverridePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributionDescriptor {
    pub contribution_id: ContributionId,
    pub contributor_module_id: ModuleId,
    pub point_id: ExtensionPointId,
    pub contract_version: ContractVersion,
    #[serde(default)]
    pub required_permissions: BTreeSet<PermissionCode>,
    pub mode: ContributionMode,
    #[serde(default)]
    pub ordering: ContributionOrdering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleDescriptor {
    pub bus_version: ExtensionBusVersion,
    pub module_id: ModuleId,
    pub module_version: ModuleVersion,
    pub module_kind: ModuleKind,
    #[serde(default)]
    pub activation: ModuleActivationDeclaration,
    #[serde(default)]
    pub dependencies: BTreeSet<ModuleDependency>,
    #[serde(default)]
    pub granted_permissions: BTreeSet<PermissionCode>,
    #[serde(default)]
    pub extension_points: Vec<ExtensionPointDescriptor>,
    #[serde(default)]
    pub contributions: Vec<ContributionDescriptor>,
}
