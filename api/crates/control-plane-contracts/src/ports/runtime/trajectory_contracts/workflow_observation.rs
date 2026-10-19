//! Host-owned observation identity for one execution segment, never public input or a checkpoint.
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowObservationContext {
    pub client_request_id: Uuid,
    /// A previously authenticated Responses context, not the current trigger.
    pub context_flow_run_id: Option<Uuid>,
    pub context_response_id: Option<String>,
    /// Set only by the admitted callback-resume path.
    pub is_resume: bool,
}
