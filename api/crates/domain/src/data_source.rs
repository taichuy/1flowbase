use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::DataModelStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSourceInstanceStatus {
    Draft,
    Ready,
    Invalid,
    Disabled,
}

impl DataSourceInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Invalid => "invalid",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSourceCatalogRefreshStatus {
    Idle,
    Ready,
    Failed,
}

impl DataSourceCatalogRefreshStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSourceDefaults {
    pub data_model_status: DataModelStatus,
}

impl Default for DataSourceDefaults {
    fn default() -> Self {
        Self {
            data_model_status: DataModelStatus::Published,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSourceInstanceRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub source_code: String,
    pub display_name: String,
    pub status: DataSourceInstanceStatus,
    pub config_json: serde_json::Value,
    pub metadata_json: serde_json::Value,
    pub secret_ref: Option<String>,
    pub secret_version: Option<i32>,
    pub defaults: DataSourceDefaults,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSourceSecretRecord {
    pub data_source_instance_id: Uuid,
    pub secret_ref: String,
    pub encrypted_secret_json: serde_json::Value,
    pub secret_version: i32,
    pub updated_at: OffsetDateTime,
}

pub fn data_source_secret_ref(data_source_instance_id: Uuid) -> String {
    format!("secret://workspace/data-source/{data_source_instance_id}/config")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSourceCatalogCacheRecord {
    pub data_source_instance_id: Uuid,
    pub refresh_status: DataSourceCatalogRefreshStatus,
    pub catalog_json: serde_json::Value,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSourcePreviewSessionRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub data_source_instance_id: Option<Uuid>,
    pub config_fingerprint: String,
    pub preview_json: serde_json::Value,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}
