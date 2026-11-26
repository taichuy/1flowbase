use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SYSTEM_SCOPE_ID: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000000");
pub const DEFAULT_SCOPE_ID: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000001");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    Active,
    Suspended,
}

impl TenantStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMembershipStatus {
    Active,
    Disabled,
}

impl WorkspaceMembershipStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TenantRecord {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub is_root: bool,
    pub is_hidden: bool,
    pub status: TenantStatus,
    pub cell_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeContext {
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
}
