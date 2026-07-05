use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontstagePageKind {
    Group,
    Page,
}

impl FrontstagePageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::Page => "page",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "group" => Some(Self::Group),
            "page" => Some(Self::Page),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontstagePageVisibility {
    Visible,
    Hidden,
}

impl FrontstagePageVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "visible" => Some(Self::Visible),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstagePageRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: FrontstagePageKind,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub is_hidden: bool,
    pub slug: Option<String>,
    pub schema_root_uid: Option<String>,
    pub rank: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstagePageTreeNode {
    pub page: FrontstagePageRecord,
    pub children: Vec<FrontstagePageTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstagePageVisibilityRuleRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Option<Uuid>,
    pub role_id: Uuid,
    pub visibility: FrontstagePageVisibility,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontstagePageSchemaRecord {
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub root_uid: String,
    pub schema_payload: serde_json::Value,
    pub root_payload: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontstagePageDetail {
    pub page: FrontstagePageRecord,
    pub schema: FrontstagePageSchemaRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstageBlockCodeRecord {
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
    pub code: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
