use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionActivationStatus {
    Discovered,
    PolicyRejected,
    PendingRestart,
    Active,
    Unhealthy,
}

impl HostExtensionActivationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::PolicyRejected => "policy_rejected",
            Self::PendingRestart => "pending_restart",
            Self::Active => "active",
            Self::Unhealthy => "unhealthy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionTrustLevel {
    TrustedHost,
    LocalTrusted,
    UnverifiedHost,
}

impl HostExtensionTrustLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrustedHost => "trusted_host",
            Self::LocalTrusted => "local_trusted",
            Self::UnverifiedHost => "unverified_host",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostExtensionInventoryRecord {
    pub id: Uuid,
    pub extension_id: String,
    pub version: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: HostExtensionTrustLevel,
    pub activation_status: HostExtensionActivationStatus,
    pub provides_contracts: Vec<String>,
    pub overrides_contracts: Vec<String>,
    pub registers_slots: Vec<String>,
    pub registers_storage: Vec<String>,
    pub last_error: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
