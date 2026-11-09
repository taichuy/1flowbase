//! Documentation-only result unions for the handlers registered by frontstage_query_kernel and
//! frontstage_action_kernel. Runtime model records remain Value because the selected model owns
//! their field schema; the stable dispatcher envelopes and page-detail DTO remain explicit.
use serde_json::Value;
use utoipa::ToSchema;

#[derive(ToSchema)]
pub struct FrontstageModelListResultSchema {
    pub model: String,
    /// Actual RuntimeListResult.items values governed by the selected runtime model's fields.
    pub items: Vec<Value>,
    pub total: i64,
}
#[derive(ToSchema)]
pub struct FrontstageModelRecordResultSchema {
    pub model: String,
    /// Actual runtime get/create/update/delete result, governed by the selected model's schema.
    pub record: Value,
}
#[derive(ToSchema)]
#[serde(untagged)]
pub enum FrontstageQueryResultSchema {
    PageDetail(super::FrontstagePageDetailResponse),
    ModelList(FrontstageModelListResultSchema),
    ModelRecord(FrontstageModelRecordResultSchema),
}
#[derive(ToSchema)]
#[serde(untagged)]
pub enum FrontstageActionResultSchema {
    PageDetail(super::FrontstagePageDetailResponse),
    ModelRecord(FrontstageModelRecordResultSchema),
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/dispatch_openapi.rs"]
mod tests;
