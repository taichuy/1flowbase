//! Shared API operation discovery and phase capability; independent of MCP bindability.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{EventSourceScope, InterfaceEventPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiEventUnavailableReason {
    OperationalControl,
    ProtocolControl,
    MissingOpenapiContract,
    MissingBinding,
    MissingTypedCodec,
    NoWorkspacePrincipal,
    PermissionDenied,
    TargetUnavailable,
    BinaryRequest,
    BinaryResponse,
    MissingStreamingTerminal,
    CredentialBoundary,
    AmbiguousOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiEventPhaseAvailability {
    Available,
    Unavailable { reason: ApiEventUnavailableReason },
}

impl ApiEventPhaseAvailability {
    pub fn available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiEventOperationSummary {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub description: String,
    #[schema(value_type = Object)]
    pub scope: EventSourceScope,
    pub before: ApiEventPhaseAvailability,
    pub completion: ApiEventPhaseAvailability,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiEventOperationDetails {
    #[serde(flatten)]
    pub operation: ApiEventOperationSummary,
    pub request_schema: Option<Value>,
    pub result_schema: Option<Value>,
    pub before_schema: Option<Value>,
    pub completion_schema: Option<Value>,
}

impl ApiEventOperationDetails {
    pub fn phase_schema(&self, phase: InterfaceEventPhase) -> Option<&Value> {
        match phase {
            InterfaceEventPhase::Before if self.operation.before.available() => {
                self.before_schema.as_ref()
            }
            InterfaceEventPhase::Completion if self.operation.completion.available() => {
                self.completion_schema.as_ref()
            }
            _ => None,
        }
    }
}
