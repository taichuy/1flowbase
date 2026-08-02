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
pub struct ExtensionCatalogIdentity {
    category: ExtensionCategory,
    organization: String,
    artifact_id: String,
}

impl ExtensionCatalogIdentity {
    pub fn parse(category: ExtensionCategory, catalog_id: &str) -> Option<Self> {
        let (catalog_category, artifact_path) = catalog_id.split_once(':')?;
        let (organization, artifact_id) = artifact_path.split_once('/')?;
        let identity = Self {
            category,
            organization: organization.to_string(),
            artifact_id: artifact_id.to_string(),
        };
        if catalog_category != category.as_str()
            || !valid_catalog_segment(organization)
            || !valid_catalog_segment(artifact_id)
            || identity.catalog_id() != catalog_id
        {
            return None;
        }
        Some(identity)
    }

    pub fn catalog_id(&self) -> String {
        format!(
            "{}:{}/{}",
            self.category.as_str(),
            self.organization,
            self.artifact_id
        )
    }

    pub fn organization(&self) -> &str {
        &self.organization
    }

    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }
}

fn valid_catalog_segment(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !matches!(value, "." | "..")
        && !value.contains([':', '/', '\\', '\0'])
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionInstallationIdentity {
    pub category: ExtensionCategory,
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
    pub node_id: String,
}

impl ExtensionInstallationIdentity {
    pub fn catalog_id(&self) -> String {
        ExtensionCatalogIdentity {
            category: self.category,
            organization: self.organization.clone(),
            artifact_id: self.artifact_id.clone(),
        }
        .catalog_id()
    }
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
pub enum ExtensionApplicationAction {
    None,
    ImportAgentFlow,
    ImportMcp,
    ActivateI18n,
    ConfigureModelProvider,
}

impl ExtensionApplicationAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ImportAgentFlow => "import_agent_flow",
            Self::ImportMcp => "import_mcp",
            Self::ActivateI18n => "activate_i18n",
            Self::ConfigureModelProvider => "configure_model_provider",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "import_agent_flow" => Some(Self::ImportAgentFlow),
            "import_mcp" => Some(Self::ImportMcp),
            "activate_i18n" => Some(Self::ActivateI18n),
            "configure_model_provider" => Some(Self::ConfigureModelProvider),
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
    pub application_action: ExtensionApplicationAction,
    pub status: ExtensionInstallationStatus,
    pub is_current: bool,
    pub installed_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
