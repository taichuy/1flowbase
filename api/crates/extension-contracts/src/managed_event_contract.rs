//! Registered finite event exchange. Business payloads never delegate actor authority.
use crate::extension_bus::ManagedExecutionIdentity;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use thiserror::Error;

pub const MANAGED_EVENT_PROTOCOL_V1: &str = "1flowbase.managed-event/v1";
pub const MANAGED_EVENT_PROTOCOL_V2: &str = "1flowbase.managed-event/v2";
pub const MANAGED_EVENT_MAX_FRAME_BYTES: usize = 65_536;
// Existing business contract identities remain usable by their registered owners.
pub const MANAGED_PROCESSED_EVENT_ID: &str = "acme.composition-a.processed";
pub const MANAGED_CREATE_EVENT_ID: &str = "model_definition.committed";
pub const MANAGED_CREATE_EVENT_POINT: &str = "1flowbase.application.runtime-event.after-commit";

/// Legacy business DTO retained for existing callers; the event transport is schema-driven.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventSchema {
    pub contract_id: String,
    pub contract_version: String,
    pub payload_schema: serde_json::Value,
}
impl ManagedEventSchema {
    pub fn from_descriptor(
        contract: &crate::ContractDescriptor,
    ) -> Result<Self, ManagedEventContractError> {
        let schema = Self {
            contract_id: contract.contract_id.as_str().into(),
            contract_version: contract.contract_version.as_str().into(),
            payload_schema: contract
                .payload_schema
                .clone()
                .ok_or(ManagedEventContractError("registered event schema missing"))?,
        };
        schema.compile()?;
        Ok(schema)
    }
    fn compile(&self) -> Result<crate::CompiledManagedProjection, ManagedEventContractError> {
        crate::ManagedProjectionContract {
            contract_id: self.contract_id.clone(),
            contract_version: self.contract_version.clone(),
            schema: self.payload_schema.clone(),
        }
        .compile()
        .map_err(|_| ManagedEventContractError("invalid bounded event schema"))
    }
    pub fn validate_payload(
        &self,
        value: &serde_json::Value,
    ) -> Result<(), ManagedEventContractError> {
        self.compile()?
            .validate(value)
            .map_err(|_| ManagedEventContractError("event payload violates registered schema"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventPublication {
    pub contract_id: String,
    pub contract_version: String,
    pub payload: serde_json::Value,
}
impl ManagedEventPublication {
    /// Envelope validation is not authorization. The host must use validate_against with its registry.
    pub fn validate(&self) -> Result<(), ManagedEventContractError> {
        if !bounded(&self.contract_id)
            || !bounded(&self.contract_version)
            || !bounded_json(&self.payload, 0)
            || serde_json::to_vec(self)
                .map_err(|_| ManagedEventContractError("invalid publication"))?
                .len()
                > MANAGED_EVENT_MAX_FRAME_BYTES
        {
            return Err(ManagedEventContractError("invalid finite publication"));
        }
        Ok(())
    }
    pub fn validate_against(
        &self,
        registered: &ManagedEventSchema,
    ) -> Result<(), ManagedEventContractError> {
        self.validate()?;
        if self.contract_id != registered.contract_id
            || self.contract_version != registered.contract_version
        {
            return Err(ManagedEventContractError("event contract version mismatch"));
        }
        registered.validate_payload(&self.payload)
    }
}

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
    pub payload: serde_json::Value,
}

/// Schema and point come from the host's frozen registered publication, never the worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventDelivery {
    pub event_id: String,
    pub contract_id: String,
    pub contract_version: String,
    pub point_id: String,
    pub point_contract_version: String,
    pub schema: ManagedEventSchema,
    pub workspace_id: String,
    pub causation_id: String,
    pub correlation_id: String,
    pub payload: serde_json::Value,
}
impl ManagedEventDelivery {
    pub fn point_id(&self) -> Result<&str, ManagedEventContractError> {
        if !bounded(&self.point_id) {
            return Err(ManagedEventContractError("event point missing"));
        }
        Ok(&self.point_id)
    }
    pub fn point_contract_version(&self) -> Result<&str, ManagedEventContractError> {
        if !bounded(&self.point_contract_version) {
            return Err(ManagedEventContractError("event point version missing"));
        }
        Ok(&self.point_contract_version)
    }
    pub fn validate(&self) -> Result<(), ManagedEventContractError> {
        self.point_id()?;
        self.point_contract_version()?;
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
        ManagedEventPublication {
            contract_id: self.contract_id.clone(),
            contract_version: self.contract_version.clone(),
            payload: self.payload.clone(),
        }
        .validate_against(&self.schema)
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedEventOutcome {
    Acknowledged,
    ApplyOwned {
        operations: Vec<crate::PluginDataOperation>,
    },
    Publish {
        publication: ManagedEventPublication,
    },
    Failed {
        classification: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEventWorkerFrame {
    pub protocol: String,
    pub call_id: String,
    pub result: ManagedEventOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("managed event protocol violation: {0}")]
pub struct ManagedEventContractError(pub &'static str);

impl ManagedEventHostFrame {
    pub fn validate(&self) -> Result<(), ManagedEventContractError> {
        self.delivery.validate()?;
        if self.protocol != MANAGED_EVENT_PROTOCOL_V2
            || [&self.call_id, &self.handler, &self.graph_fingerprint]
                .iter()
                .any(|v| !bounded(v))
            || self.authority_revision < 0
            || self.deadline_unix_ms <= 0
            || self.delivery.workspace_id != self.execution_identity.workspace_id().as_str()
            || serde_json::to_vec(self)
                .map_err(|_| ManagedEventContractError("invalid host frame"))?
                .len()
                > MANAGED_EVENT_MAX_FRAME_BYTES
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
        if self.protocol != MANAGED_EVENT_PROTOCOL_V2
            || self.call_id != frame.call_id
            || serde_json::to_vec(self)
                .map_err(|_| ManagedEventContractError("invalid worker frame"))?
                .len()
                > MANAGED_EVENT_MAX_FRAME_BYTES
        {
            return Err(ManagedEventContractError(
                "uncorrelated or oversized response",
            ));
        }
        match &self.result {
            ManagedEventOutcome::ApplyOwned { operations } => {
                validate_owned_event_operations(operations)?
            }
            ManagedEventOutcome::Publish { publication } => publication.validate()?,
            ManagedEventOutcome::Failed { classification } if !bounded(classification) => {
                return Err(ManagedEventContractError("invalid failure classification"))
            }
            _ => {}
        }
        Ok(())
    }
}
/// Finite typed owned writes only. Host supplies binding, current grants and causal receipt key.
pub fn validate_owned_event_operations(
    operations: &[crate::PluginDataOperation],
) -> Result<(), ManagedEventContractError> {
    crate::PluginDataRequest {
        idempotency_key: None,
        operations: operations.to_vec(),
    }
    .validate()
    .map_err(|_| ManagedEventContractError("invalid owned event effect"))?;
    if operations.iter().any(|op| {
        !matches!(
            op,
            crate::PluginDataOperation::Upsert {
                target: crate::PluginDataTarget::OwnedCollection { .. },
                ..
            }
        )
    }) {
        return Err(ManagedEventContractError(
            "event effects require owned collection upserts",
        ));
    }
    Ok(())
}
fn bounded(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512
}
fn bounded_json(value: &serde_json::Value, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    match value {
        serde_json::Value::String(v) => v.len() <= 16384,
        serde_json::Value::Array(v) => {
            v.len() <= 256 && v.iter().all(|v| bounded_json(v, depth + 1))
        }
        serde_json::Value::Object(v) => {
            v.len() <= 128
                && v.iter()
                    .all(|(k, v)| k.len() <= 512 && bounded_json(v, depth + 1))
        }
        _ => true,
    }
}
