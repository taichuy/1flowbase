//! Temporary observation of committed application run transitions, never a second state owner.
use domain::{FlowRunMode, FlowRunStatus};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

pub const APPLICATION_RUN_STARTED_CONTRACT_ID: &str = "host.application.run.started.v1";
pub const APPLICATION_RUN_ENDED_CONTRACT_ID: &str = "host.application.run.ended.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationLifecyclePhase {
    Started,
    Ended,
}
impl ApplicationLifecyclePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Ended => "ended",
        }
    }
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Started => APPLICATION_RUN_STARTED_CONTRACT_ID,
            Self::Ended => APPLICATION_RUN_ENDED_CONTRACT_ID,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationLifecycleFact {
    pub phase: ApplicationLifecyclePhase,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub run_mode: FlowRunMode,
    pub status: FlowRunStatus,
    pub occurred_at: OffsetDateTime,
    /// Trusted execution ancestry for the event envelope; never populated from run input.
    pub workflow_ancestor_ids: Vec<Uuid>,
}
impl ApplicationLifecycleFact {
    pub fn contract_id(&self) -> &'static str {
        self.phase.contract_id()
    }
    /// The directory and emitted value share this explicit finite field declaration.
    pub fn event_source_schema() -> Value {
        json!({
            "type":"object","additionalProperties":false,
            "required":["phase","workspace_id","application_id","flow_run_id","run_mode","status","occurred_at"],
            "properties":{
                "phase":{"type":"string","enum":["started","ended"]},
                "workspace_id":{"type":"string","format":"uuid"},
                "application_id":{"type":"string","format":"uuid"},
                "flow_run_id":{"type":"string","format":"uuid"},
                "run_mode":{"type":"string","enum":["debug_node_preview","debug_flow_run","published_api_run","assistant_execution","workflow_http_run","workflow_schedule_run","workflow_event_run"]},
                "status":{"type":"string","enum":["running","succeeded","incomplete","failed","cancelled"]},
                "occurred_at":{"type":"string"}
            },
            "allOf":[
                {"if":{"properties":{"phase":{"const":"started"}}},"then":{"properties":{"status":{"const":"running"}}}},
                {"if":{"properties":{"phase":{"const":"ended"}}},"then":{"properties":{"status":{"enum":["succeeded","incomplete","failed","cancelled"]}}}}
            ]
        })
    }
    /// Source payload excludes credentials, arbitrary runtime bodies and trusted envelope ancestry.
    pub fn event_payload(&self) -> Value {
        json!({
            "phase":self.phase.as_str(),"workspace_id":self.workspace_id,
            "application_id":self.application_id,"flow_run_id":self.flow_run_id,
            "run_mode":self.run_mode.as_str(),"status":self.status.as_str(),
            "occurred_at":self.occurred_at.to_string()
        })
    }
}

/// Called after the winning transaction commits, before fallible post-commit projections.
/// Implementations enqueue best-effort observations and cannot veto the committed business result.
pub trait ApplicationLifecycleObserver: Send + Sync {
    fn observe(&self, fact: ApplicationLifecycleFact);
}

#[cfg(test)]
#[path = "application_lifecycle/_tests/projection.rs"]
mod tests;
