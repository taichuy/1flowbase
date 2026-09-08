//! Finite managed lifecycle exchange; host context is correlation, never delegated actor authority.
use crate::extension_bus::ManagedExecutionIdentity;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use thiserror::Error;

pub const MANAGED_EVENT_PROTOCOL_V1: &str = "1flowbase.managed-event/v1";
pub const MANAGED_EVENT_MAX_FRAME_BYTES: usize = 65_536;
pub const MANAGED_PROCESSED_EVENT_ID: &str = "acme.composition-a.processed";
pub const MANAGED_CREATE_EVENT_ID: &str = "model_definition.committed";
pub const MANAGED_CREATE_EVENT_POINT: &str = "1flowbase.application.runtime-event.after-commit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedEventStatus {
    Committed,
    Processed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventPayload {
    pub model_id: String,
    pub status: ManagedEventStatus,
    pub result_reference: Option<String>,
}

/// The worker chooses only the finite target contract and a non-sensitive result reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventPublication {
    pub contract_id: String,
    pub contract_version: String,
    pub payload: ManagedEventPayload,
}

/// Persisted by the host in the existing Outbox, in its own publication transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventFact {
    pub event_id: String,
    pub transaction_id: String,
    pub contract_id: String,
    pub contract_version: String,
    pub workspace_id: String,
    pub publisher: ManagedExecutionIdentity,
    pub causation_id: String,
    pub correlation_id: String,
    pub payload: ManagedEventPayload,
}

/// Trusted delivery input; actor credentials and actor rights are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventDelivery {
    pub event_id: String,
    pub contract_id: String,
    pub contract_version: String,
    pub workspace_id: String,
    pub causation_id: String,
    pub correlation_id: String,
    pub payload: ManagedEventPayload,
}
impl ManagedEventDelivery {
    pub fn point_id(&self) -> Result<&'static str, ManagedEventContractError> {
        match (self.contract_id.as_str(), self.contract_version.as_str()) {
            (MANAGED_CREATE_EVENT_ID, "v1") => Ok(MANAGED_CREATE_EVENT_POINT),
            (MANAGED_PROCESSED_EVENT_ID, "1") => Ok(MANAGED_PROCESSED_EVENT_ID),
            _ => Err(ManagedEventContractError("event contract is not opened")),
        }
    }
    pub fn point_contract_version(&self) -> Result<&'static str, ManagedEventContractError> {
        self.point_id()?;
        Ok("1")
    }
    pub fn validate(&self) -> Result<(), ManagedEventContractError> {
        self.point_id()?;
        if [
            &self.event_id,
            &self.workspace_id,
            &self.causation_id,
            &self.correlation_id,
        ]
        .iter()
        .any(|v| !bounded(v))
        {
            return Err(ManagedEventContractError("invalid event context"));
        }
        validate_payload(&self.payload)?;
        if (self.contract_id == MANAGED_CREATE_EVENT_ID)
            != (self.payload.status == ManagedEventStatus::Committed)
        {
            return Err(ManagedEventContractError(
                "event payload does not match contract",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventHostFrame {
    pub protocol: String,
    pub call_id: String,
    pub handler: String,
    pub execution_identity: ManagedExecutionIdentity,
    pub generation: NonZeroU64,
    pub graph_fingerprint: String,
    pub authority_revision: i64,
    pub deadline_unix_ms: i64,
    pub delivery: ManagedEventDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedEventOutcome {
    Acknowledged,
    ApplyProcessed {
        effect: ManagedEventPayload,
    },
    Publish {
        publication: ManagedEventPublication,
    },
    Failed {
        classification: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventWorkerFrame {
    pub protocol: String,
    pub call_id: String,
    pub result: ManagedEventOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("managed event protocol violation: {0}")]
pub struct ManagedEventContractError(pub &'static str);

impl ManagedEventPublication {
    pub fn validate(&self) -> Result<(), ManagedEventContractError> {
        if self.contract_id != MANAGED_PROCESSED_EVENT_ID
            || self.contract_version != "1"
            || self.payload.status != ManagedEventStatus::Processed
        {
            return Err(ManagedEventContractError(
                "publication contract is not opened",
            ));
        }
        validate_payload(&self.payload)
    }
}
impl ManagedEventHostFrame {
    pub fn validate(&self) -> Result<(), ManagedEventContractError> {
        self.delivery.validate()?;
        if self.protocol != MANAGED_EVENT_PROTOCOL_V1
            || [&self.call_id, &self.handler, &self.graph_fingerprint]
                .iter()
                .any(|v| !bounded(v))
            || self.authority_revision < 0
            || self.deadline_unix_ms <= 0
            || self.delivery.workspace_id != self.execution_identity.workspace_id().as_str()
        {
            return Err(ManagedEventContractError("invalid host context"));
        }
        Ok(())
    }
}
impl ManagedEventWorkerFrame {
    pub fn validate_for(
        &self,
        frame: &ManagedEventHostFrame,
    ) -> Result<(), ManagedEventContractError> {
        if self.protocol != MANAGED_EVENT_PROTOCOL_V1 || self.call_id != frame.call_id {
            return Err(ManagedEventContractError("uncorrelated response"));
        }
        match &self.result {
            ManagedEventOutcome::ApplyProcessed { effect } => {
                if frame.delivery.contract_id != MANAGED_PROCESSED_EVENT_ID
                    || frame.delivery.contract_version != "1"
                    || effect != &frame.delivery.payload
                    || effect.status != ManagedEventStatus::Processed
                {
                    return Err(ManagedEventContractError(
                        "effect must match delivered processed fact",
                    ));
                }
                validate_payload(effect)?;
            }
            ManagedEventOutcome::Publish { publication } => {
                publication.validate()?;
                if publication.payload.model_id != frame.delivery.payload.model_id {
                    return Err(ManagedEventContractError(
                        "publication changes the delivered model",
                    ));
                }
            }
            ManagedEventOutcome::Failed { classification } if !bounded(classification) => {
                return Err(ManagedEventContractError("invalid failure classification"))
            }
            _ => {}
        }
        Ok(())
    }
}
fn bounded(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512
}
fn validate_payload(payload: &ManagedEventPayload) -> Result<(), ManagedEventContractError> {
    if !bounded(&payload.model_id)
        || payload.result_reference.as_ref().is_some_and(|value| {
            !value.starts_with("processed_models/")
                || value.len() > 512
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.'))
        })
    {
        return Err(ManagedEventContractError("invalid finite event payload"));
    }
    Ok(())
}
