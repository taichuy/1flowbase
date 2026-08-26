use serde::{Deserialize, Serialize};

pub const NETWORK_EGRESS_PROVIDER_CONTRACT: &str = "1flowbase.network_egress_provider/v1";

/// A provider-owned, stable egress identity with only host-safe display data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EgressDescriptor {
    pub provider_egress_key: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    pub availability: EgressAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressAvailability {
    Available,
    Unavailable,
}
