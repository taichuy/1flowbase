use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionCategory {
    AgentFlow,
    CapabilityPlugins,
    HostExtensions,
    I18n,
    Mcp,
    RuntimeExtensions,
}

impl ExtensionCategory {
    pub const ALL: [Self; 6] = [
        Self::AgentFlow,
        Self::CapabilityPlugins,
        Self::HostExtensions,
        Self::I18n,
        Self::Mcp,
        Self::RuntimeExtensions,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentFlow => "agent-flow",
            Self::CapabilityPlugins => "capability-plugins",
            Self::HostExtensions => "host-extensions",
            Self::I18n => "i18n",
            Self::Mcp => "mcp",
            Self::RuntimeExtensions => "runtime-extensions",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionInstallationIdentity {
    pub category: ExtensionCategory,
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionInstallationStatus {
    Installed,
    Missing,
}

impl ExtensionInstallationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Missing => "missing",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "installed" => Some(Self::Installed),
            "missing" => Some(Self::Missing),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSignatureStatus {
    Verified,
    Missing,
    UnknownKey,
    Invalid,
}

impl ExtensionSignatureStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Missing => "missing",
            Self::UnknownKey => "unknown_key",
            Self::Invalid => "invalid",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "verified" => Some(Self::Verified),
            "missing" => Some(Self::Missing),
            "unknown_key" => Some(Self::UnknownKey),
            "invalid" => Some(Self::Invalid),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionIntegrityWarning {
    pub code: String,
    pub message: String,
    pub overridable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionCompatibilityWarning {
    pub reason: String,
    pub current_host_version: String,
    pub minimum_host_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRiskChallenge {
    pub warnings: Vec<ExtensionIntegrityWarning>,
    pub compatibility: Option<ExtensionCompatibilityWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInstallationReceipt {
    pub source: String,
    pub trust: String,
    pub expected_checksum: Option<String>,
    pub actual_checksum: String,
    pub signature_status: ExtensionSignatureStatus,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub warnings: Vec<ExtensionIntegrityWarning>,
    pub override_receipt: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInstallationRecord {
    pub id: Uuid,
    pub identity: ExtensionInstallationIdentity,
    pub source: String,
    pub trust: String,
    pub local_path: String,
    pub checksum: String,
    pub signature_status: ExtensionSignatureStatus,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub warnings: Vec<ExtensionIntegrityWarning>,
    pub receipt: serde_json::Value,
    pub status: ExtensionInstallationStatus,
    pub installed_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
