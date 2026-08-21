use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEgressProviderLifecycle {
    Draft,
    Active,
    Disabled,
}

impl NetworkEgressProviderLifecycle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEgressHealthStatus {
    Unknown,
    Healthy,
    Unhealthy,
}

impl NetworkEgressHealthStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Unhealthy => "unhealthy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEgressProviderRecord {
    pub id: Uuid,
    /// Extension-backed providers retain their installation. Built-in providers are implemented
    /// by Core and intentionally have no extension installation.
    pub installation_id: Option<Uuid>,
    pub provider_code: String,
    pub display_name: String,
    pub description: String,
    /// Opaque secret-store locator. It is never an API projection.
    pub secret_ref: String,
    pub lifecycle: NetworkEgressProviderLifecycle,
    pub health_status: NetworkEgressHealthStatus,
    pub last_sync_error: Option<String>,
    pub last_synced_at: Option<OffsetDateTime>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Durable encrypted material owned by the provider registry. This storage record is deliberately
/// not an API DTO; callers must resolve it through the typed runner-provisioning boundary.
#[derive(Clone, PartialEq)]
pub struct NetworkEgressProviderSecretRecord {
    pub provider_id: Uuid,
    pub secret_ref: String,
    pub encrypted_secret_json: serde_json::Value,
    pub secret_version: i32,
    pub updated_at: OffsetDateTime,
}

impl std::fmt::Debug for NetworkEgressProviderSecretRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NetworkEgressProviderSecretRecord")
            .field("provider_id", &self.provider_id)
            .field("secret_ref", &self.secret_ref)
            .field("encrypted_secret_json", &"<encrypted>")
            .field("secret_version", &self.secret_version)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEgressProjectionRecord {
    pub provider_id: Uuid,
    pub provider_egress_key: String,
    pub display_name: String,
    pub region: Option<String>,
    pub tags: Vec<String>,
    pub availability: String,
    pub synced_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEgressPoolSelectionStrategy {
    HealthyFirst,
}

impl NetworkEgressPoolSelectionStrategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HealthyFirst => "healthy_first",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEgressPoolMemberHealth {
    Healthy,
    Unhealthy,
    Invalid,
}

impl NetworkEgressPoolMemberHealth {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Unhealthy => "unhealthy",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEgressPool {
    pub id: Uuid,
    pub display_name: String,
    /// A provider-owned pool mirrors that provider's current projection and its members are not
    /// independently editable.
    pub owner_provider_id: Option<Uuid>,
    pub selection_strategy: NetworkEgressPoolSelectionStrategy,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A durable reference to a provider descriptor. Runtime proxy leases deliberately do not cross
/// this boundary: each consumer obtains a fresh lease only after selecting this member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEgressPoolMember {
    pub id: Uuid,
    pub pool_id: Uuid,
    pub provider_id: Uuid,
    pub provider_egress_key: String,
    pub enabled: bool,
    pub sequence: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Closed consumer identities for Network Center routing. Persistence may project these as a
/// `consumer_kind` plus an optional UUID, but callers never compose arbitrary strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkEgressConsumerSelector {
    GithubOfficialSources,
    ModelProviderDefault,
    ModelProviderInstance { instance_id: Uuid },
    HttpNodeDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkEgressConsumerSelectorParseError;

impl std::fmt::Display for NetworkEgressConsumerSelectorParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid network egress consumer selector")
    }
}

impl std::error::Error for NetworkEgressConsumerSelectorParseError {}

impl NetworkEgressConsumerSelector {
    pub const fn consumer_kind(&self) -> &'static str {
        match self {
            Self::GithubOfficialSources => "github",
            Self::ModelProviderDefault | Self::ModelProviderInstance { .. } => "model_provider",
            Self::HttpNodeDefault => "http_node",
        }
    }

    pub const fn consumer_reference(&self) -> Option<Uuid> {
        match self {
            Self::ModelProviderInstance { instance_id } => Some(*instance_id),
            Self::GithubOfficialSources | Self::ModelProviderDefault | Self::HttpNodeDefault => {
                None
            }
        }
    }

    pub fn from_storage(
        consumer_kind: &str,
        consumer_reference: Option<Uuid>,
    ) -> Result<Self, NetworkEgressConsumerSelectorParseError> {
        match (consumer_kind, consumer_reference) {
            ("github", None) => Ok(Self::GithubOfficialSources),
            ("model_provider", None) => Ok(Self::ModelProviderDefault),
            ("model_provider", Some(instance_id)) => {
                Ok(Self::ModelProviderInstance { instance_id })
            }
            ("http_node", None) => Ok(Self::HttpNodeDefault),
            _ => Err(NetworkEgressConsumerSelectorParseError),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEgressRoute {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub selector: NetworkEgressConsumerSelector,
    pub pool_id: Uuid,
    pub enabled: bool,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
