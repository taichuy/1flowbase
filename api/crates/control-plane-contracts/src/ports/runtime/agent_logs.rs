//! Versioned source facts. Imported content shares the task/message/client directories.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const AGENT_LOGS_SCHEMA_VERSION: &str = "1flowbase.agent-logs/v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentLogsBatch {
    pub schema_version: String,
    pub source_id: String,
    pub source_client: String,
    pub events: Vec<AgentLogEvent>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentLogEvent {
    pub event_id: String,
    pub source_session_id: String,
    pub source_task_id: String,
    pub parent_source_task_id: Option<String>,
    pub sequence: i64,
    pub occurred_at: String,
    pub kind: AgentLogEventKind,
    pub content: Option<String>,
    /// Source declaration: `final_answer` with nonempty content on assistant or
    /// task_end supplies the final answer; `cancelled` on task_end marks cancellation.
    /// Conversation projects only the last eligible non-inherited final declaration.
    pub phase: Option<String>,
    pub name: Option<String>,
    pub call_id: Option<String>,
    pub model_id: Option<String>,
    /// Source-declared effort; omission preserves the canonical identity of v1 events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    pub provider_code: Option<String>,
    pub usage: Option<AgentLogUsage>,
    #[serde(default)]
    pub inherited: bool,
    #[serde(default)]
    pub raw: Value,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentLogEventKind {
    System,
    User,
    Assistant,
    ToolCall,
    ToolResult,
    Usage,
    Context,
    TaskEnd,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentLogUsage {
    pub basis: AgentLogUsageBasis,
    pub response_id: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentLogUsageBasis {
    Delta,
    Cumulative,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogsReceipt {
    pub accepted_events: usize,
    pub duplicate_events: usize,
    pub record_ids: Vec<Uuid>,
}

/// Internal maintenance projection; does not load conversation or raw bodies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogPricingEvent {
    pub event_id: String,
    pub sequence: i64,
    pub occurred_at: String,
    pub model_id: Option<String>,
    pub inherited: bool,
    pub usage: AgentLogUsage,
}
#[derive(Debug, Clone)]
pub struct AgentLogPricingRecord {
    pub record_id: Uuid,
    pub events: Vec<AgentLogPricingEvent>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AgentLogsRepriceRecordReceipt {
    pub record_id: Uuid,
    pub usage_events: usize,
    pub changed_events: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogMessage {
    pub role: String,
    pub content: String,
    pub sequence: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationLogCostBreakdown {
    /// Internal 1flowbase credits, as exact decimal text; never debited by ingestion.
    pub total_cost: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationLogView {
    Conversation,
    ClientTrajectory,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationLogRecordOverview {
    pub record_id: Uuid,
    pub source_kind: String,
    pub source_client: Option<String>,
    pub source_session_id: Option<String>,
    pub source_task_id: Option<String>,
    pub native_run_id: Option<Uuid>,
    pub title: String,
    pub outcome: String,
    pub messages: Vec<AgentLogMessage>,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub cost_breakdown: ApplicationLogCostBreakdown,
    pub available_views: Vec<ApplicationLogView>,
}

impl AgentLogsBatch {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == AGENT_LOGS_SCHEMA_VERSION,
            "agent_logs.schema_version"
        );
        anyhow::ensure!(
            !self.source_id.trim().is_empty() && !self.source_client.trim().is_empty(),
            "agent_logs.source_identity"
        );
        anyhow::ensure!(!self.events.is_empty(), "agent_logs.batch_size");
        for event in &self.events {
            anyhow::ensure!(
                !event.event_id.is_empty()
                    && !event.source_session_id.is_empty()
                    && !event.source_task_id.is_empty()
                    && event.sequence >= 0,
                "agent_logs.event_identity"
            );
            time::OffsetDateTime::parse(
                &event.occurred_at,
                &time::format_description::well_known::Rfc3339,
            )?;
            if let Some(usage) = &event.usage {
                anyhow::ensure!(
                    [
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.input_cache_hit_tokens,
                        usage.cache_write_tokens,
                        usage.total_tokens
                    ]
                    .into_iter()
                    .flatten()
                    .all(|n| n >= 0),
                    "agent_logs.negative_usage"
                );
            }
        }
        Ok(())
    }
}

/// Native pages retain their numeric wire position. Imported positions are opaque
/// versioned strings encoding source sequence and immutable step identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RecordClientTrajectoryCursor {
    Native(i64),
    Imported(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordClientTrajectoryPage {
    pub items: Vec<super::ClientTrajectoryStep>,
    pub next_cursor: Option<RecordClientTrajectoryCursor>,
    pub integrity: String,
}
impl RecordClientTrajectoryCursor {
    pub fn imported(sequence: i64, step_id: Uuid) -> Self {
        Self::Imported(format!("s1:{sequence}:{step_id}"))
    }
    pub fn imported_position(cursor: &str) -> anyhow::Result<(i64, Uuid)> {
        let mut parts = cursor.split(':');
        anyhow::ensure!(parts.next() == Some("s1"), "record_trajectory_cursor");
        let sequence = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("record_trajectory_cursor"))?
            .parse::<i64>()?;
        let id = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("record_trajectory_cursor"))?
            .parse::<Uuid>()?;
        anyhow::ensure!(
            sequence >= 0 && parts.next().is_none(),
            "record_trajectory_cursor"
        );
        Ok((sequence, id))
    }
}

/// Explicit deletion scope; an omitted mode can never mean all retained history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentLogsDeleteScope {
    AllTime {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        batch_size: Option<std::num::NonZeroU32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ingested_at_before: Option<String>,
    },
    TimeRange {
        started_at_from: String,
        started_at_to: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        batch_size: Option<std::num::NonZeroU32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ingested_at_before: Option<String>,
    },
}
impl AgentLogsDeleteScope {
    pub fn all_time() -> Self {
        Self::AllTime {
            batch_size: None,
            ingested_at_before: None,
        }
    }
    pub fn batch_size(&self) -> Option<std::num::NonZeroU32> {
        match self {
            Self::AllTime { batch_size, .. } | Self::TimeRange { batch_size, .. } => *batch_size,
        }
    }
    /// Reuse the first receipt's boundary so later imports are outside this operation.
    pub fn ingested_at_before(&self) -> anyhow::Result<Option<time::OffsetDateTime>> {
        let value = match self {
            Self::AllTime {
                ingested_at_before, ..
            }
            | Self::TimeRange {
                ingested_at_before, ..
            } => ingested_at_before,
        };
        value
            .as_deref()
            .map(|value| {
                time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            })
            .transpose()
            .map_err(Into::into)
    }
    /// Imported turns are selected by task start time, using [from, to).
    pub fn bounds(&self) -> anyhow::Result<Option<(time::OffsetDateTime, time::OffsetDateTime)>> {
        match self {
            Self::AllTime { .. } => Ok(None),
            Self::TimeRange {
                started_at_from,
                started_at_to,
                ..
            } => {
                let from = time::OffsetDateTime::parse(
                    started_at_from,
                    &time::format_description::well_known::Rfc3339,
                )?;
                let to = time::OffsetDateTime::parse(
                    started_at_to,
                    &time::format_description::well_known::Rfc3339,
                )?;
                anyhow::ensure!(from < to, "agent_logs_delete_time_range");
                Ok(Some((from, to)))
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentLogsDeleteReceipt {
    pub deleted_records: u64,
    pub has_more: bool,
    pub ingested_at_before: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentLogsDeletePreview {
    pub total_records: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentLogsDeleteJobCreate {
    /// Client-generated identity makes an acknowledged or lost start safely discoverable.
    pub job_id: Uuid,
    pub scope: AgentLogsDeleteScope,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentLogsDeleteJobStatus {
    Queued,
    Running,
    Succeeded,
    Stopped,
    Failed,
}
impl AgentLogsDeleteJobStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
    /// Fixed-workset state machine; stopped batches never claim full completion.
    pub fn after_batch(deleted: u64, total: u64, stop_requested: bool) -> Self {
        if deleted == total {
            Self::Succeeded
        } else if stop_requested {
            Self::Stopped
        } else {
            Self::Running
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogsDeleteJob {
    pub job_id: Uuid,
    pub application_id: Uuid,
    pub scope: AgentLogsDeleteScope,
    pub ingested_at_before: String,
    pub status: AgentLogsDeleteJobStatus,
    pub total_records: u64,
    pub deleted_records: u64,
    pub stop_requested: bool,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
