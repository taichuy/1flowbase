use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginWorkerStatus {
    Unloaded,
    Starting,
    Idle,
    Busy,
    Recycled,
    Crashed,
}

impl PluginWorkerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unloaded => "unloaded",
            Self::Starting => "starting",
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Recycled => "recycled",
            Self::Crashed => "crashed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginWorkerLeaseRecord {
    pub id: Uuid,
    pub installation_id: Uuid,
    pub worker_key: String,
    pub status: PluginWorkerStatus,
    pub runtime_scope: serde_json::Value,
    pub last_heartbeat_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
