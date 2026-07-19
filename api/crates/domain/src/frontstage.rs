use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontstagePageKind {
    Group,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontstageNavigationPlacement {
    Topbar,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontstagePageContentPresentation {
    Single,
    Tabs,
}

impl FrontstagePageContentPresentation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Tabs => "tabs",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "single" => Some(Self::Single),
            "tabs" => Some(Self::Tabs),
            _ => None,
        }
    }
}

impl FrontstageNavigationPlacement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Topbar => "topbar",
            Self::Sidebar => "sidebar",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "topbar" => Some(Self::Topbar),
            "sidebar" => Some(Self::Sidebar),
            _ => None,
        }
    }
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
    pub placement: FrontstageNavigationPlacement,
    pub content_presentation: FrontstagePageContentPresentation,
    pub slug: Option<String>,
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
    pub tab_id: Option<Uuid>,
    pub role_id: Uuid,
    pub visibility: FrontstagePageVisibility,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontstageTabDocumentRecord {
    pub workspace_id: Uuid,
    pub tab_id: Uuid,
    pub root_uid: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontstagePageDetail {
    pub page: FrontstagePageRecord,
    pub tab: FrontstagePageTabRecord,
    pub document: FrontstageTabDocumentRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrontstagePageCreation {
    pub page: FrontstagePageRecord,
    pub default_tab: Option<FrontstagePageTabRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstagePageTabRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub title: Option<String>,
    pub rank: String,
    pub is_default: bool,
    pub route_segment: Option<String>,
    pub document_root_uid: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_001_navigation_placement_has_stable_storage_values() {
        assert_eq!(FrontstageNavigationPlacement::Topbar.as_str(), "topbar");
        assert_eq!(FrontstageNavigationPlacement::Sidebar.as_str(), "sidebar");
        assert_eq!(
            FrontstageNavigationPlacement::from_db("topbar"),
            Some(FrontstageNavigationPlacement::Topbar)
        );
        assert_eq!(FrontstageNavigationPlacement::from_db("unknown"), None);
    }

    #[test]
    fn ac_003_page_tab_is_a_distinct_document_owner() {
        let page_id = Uuid::from_u128(1);
        let tab_id = Uuid::from_u128(2);
        let workspace_id = Uuid::from_u128(3);
        let now = OffsetDateTime::UNIX_EPOCH;
        let tab = FrontstagePageTabRecord {
            id: tab_id,
            workspace_id,
            page_id,
            title: Some("Default".to_owned()),
            rank: "a".to_owned(),
            is_default: true,
            route_segment: None,
            document_root_uid: "frontstage.tab.2.root".to_owned(),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(tab.page_id, page_id);
        assert_eq!(tab.id, tab_id);
        assert!(tab.is_default);
        assert_eq!(tab.route_segment, None);
        assert_eq!(tab.document_root_uid, "frontstage.tab.2.root");
    }
}
