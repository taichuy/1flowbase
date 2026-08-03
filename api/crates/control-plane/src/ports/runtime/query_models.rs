use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListApplicationRunsPageInput {
    pub page: i64,
    pub page_size: i64,
    pub created_after: Option<OffsetDateTime>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunSummaryPage {
    pub items: Vec<domain::ApplicationRunSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunLogSummaryPage {
    pub items: Vec<domain::ApplicationRunLogSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunCountTokensResult {
    pub flow_run_id: Uuid,
    pub input_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ListApplicationConversationRunsPageInput {
    pub external_conversation_id: String,
    pub around_run_id: Option<Uuid>,
    pub before_run_id: Option<Uuid>,
    pub after_run_id: Option<Uuid>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationConversationRunsPage {
    pub items: Vec<domain::ApplicationConversationRunSummary>,
    pub has_before: bool,
    pub has_after: bool,
    pub before_cursor: Option<Uuid>,
    pub after_cursor: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListApplicationRunConversationMessageItemsPageInput {
    pub before_sequence: Option<i64>,
    pub after_sequence: Option<i64>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunConversationMessageItemsPage {
    pub items: Vec<domain::ApplicationRunConversationMessageItem>,
    pub total_count: i64,
    pub has_before: bool,
    pub has_after: bool,
    pub before_cursor: Option<i64>,
    pub after_cursor: Option<i64>,
}
