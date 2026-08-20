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
    pub installation_id: Uuid,
    pub provider_code: String,
    pub display_name: String,
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
