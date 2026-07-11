use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailoverQueueSnapshotItem {
    pub sort_index: i32,
    pub provider_instance_id: Uuid,
    pub provider_instance_display_name: String,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub protocol: String,
    pub enabled: bool,
}

impl From<domain::ModelFailoverQueueItemRecord> for FailoverQueueSnapshotItem {
    fn from(item: domain::ModelFailoverQueueItemRecord) -> Self {
        Self {
            sort_index: item.sort_index,
            provider_instance_id: item.provider_instance_id,
            provider_instance_display_name: item.provider_instance_display_name,
            provider_code: item.provider_code,
            upstream_model_id: item.upstream_model_id,
            protocol: item.protocol,
            enabled: item.enabled,
        }
    }
}

pub fn freeze_queue_items(items: &[FailoverQueueSnapshotItem]) -> serde_json::Value {
    let mut ordered = items.to_vec();
    ordered.sort_by_key(|item| item.sort_index);
    serde_json::to_value(ordered).unwrap_or_else(|_| serde_json::json!([]))
}
