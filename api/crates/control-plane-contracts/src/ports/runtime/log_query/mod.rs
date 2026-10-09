//! Source-neutral log querying; fields retain persisted contract names.
use super::*;
use domain::ResourceFilterExpr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ApplicationLogRecordsQuery {
    pub filter: ResourceFilterExpr,
    pub sort_field: String,
    pub descending: bool,
    pub cursor: Option<String>,
    pub limit: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationLogRecordSummary {
    pub record_id: Uuid,
    pub application_id: Uuid,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_client: Option<String>,
    pub source_session_id: Option<String>,
    pub source_task_id: Option<String>,
    pub native_run_id: Option<Uuid>,
    pub log_conversation_id: Option<Uuid>,
    pub requested_model_id: Option<String>,
    pub reasoning_effort: Option<String>,
    pub status: String,
    pub outcome: String,
    pub title: String,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub total_cost: Option<String>,
    pub cost_breakdown: ApplicationLogCostBreakdown,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub available_views: Vec<ApplicationLogView>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationLogRecordsPage {
    pub items: Vec<ApplicationLogRecordSummary>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone)]
pub struct RecordClientTrajectoryQuery {
    pub filter: ResourceFilterExpr,
    pub cursor: Option<String>,
    pub limit: i64,
    pub keyword: Option<String>,
    /// Only overview / parameters / result. Empty means result when keyword is present.
    pub search_sections: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordTrajectoryMatch {
    pub step_id: Uuid,
    pub section: String,
    pub sequence: i64,
    pub snippet: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordClientTrajectoryQueryPage {
    pub items: Vec<ClientTrajectoryStep>,
    pub matches: Vec<RecordTrajectoryMatch>,
    pub next_cursor: Option<String>,
    pub search_sections: Vec<String>,
    pub integrity: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogQueryValueType {
    String,
    Uuid,
    Number,
    Boolean,
    Datetime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQueryField {
    pub field: String,
    pub value_type: LogQueryValueType,
    pub operators: Vec<String>,
    pub sortable: bool,
}
mod fields;
pub use fields::*;
mod predicates;
pub use predicates::*;
#[cfg(test)]
#[path = "../_tests/log_query.rs"]
mod tests;

mod cursor;
pub use cursor::*;
