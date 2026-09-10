//! Schema-free exchange for an explicitly selected, installation-bound interface protocol.
use crate::managed_interface_contract::{bounded_value, classification_code, identity};
use crate::{
    extension_bus::HookPhase, ManagedHookHostContext, ManagedHookOutcome, ManagedHookTerminal,
    ManagedInterfaceContractError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

type ContractResult<T = ()> = Result<T, ManagedInterfaceContractError>;
pub const MANAGED_INTERFACE_PROTOCOL_V2: &str = "1flowbase.managed-interface/v2";
pub const MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES: usize = 131_072;
pub const MANAGED_INTERFACE_MAX_PROJECTION_BYTES: usize = 65_536;
pub const MANAGED_INTERFACE_MAX_REQUEST_BYTES: usize = 98_304;
pub const MANAGED_INTERFACE_MAX_REPLY_BYTES: usize = 8_192;

/// Manifest selection is separate from runtime transport and point contract versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedInterfaceProtocol {
    #[serde(rename = "interface-v1")]
    InterfaceV1,
    #[serde(rename = "reference-v2")]
    ReferenceV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedProjectionReference {
    pub contract_id: String,
    pub contract_version: String,
    pub schema_fingerprint: String,
}
impl ManagedProjectionReference {
    pub fn validate(&self) -> ContractResult {
        if !identity(&self.contract_id)
            || !identity(&self.contract_version)
            || !self
                .schema_fingerprint
                .strip_prefix("sha256:")
                .is_some_and(|digest| {
                    digest.len() == 64
                        && digest
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        {
            return Err(ManagedInterfaceContractError(
                "invalid projection reference".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedInterfaceReferenceView {
    pub contract: ManagedProjectionReference,
    pub value: Value,
}
impl ManagedInterfaceReferenceView {
    /// SDK checks shape/resources only. The host validates against its frozen schema.
    pub fn validate(&self) -> ContractResult {
        self.contract.validate()?;
        if !bounded_value(&self.value, 0)
            || serde_json::to_vec(&self.value)
                .map_err(|_| invalid_value())?
                .len()
                > MANAGED_INTERFACE_MAX_PROJECTION_BYTES
        {
            return Err(invalid_value());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedInterfaceReferenceInput {
    Authorization {
        input: ManagedInterfaceReferenceView,
    },
    Admission {
        input: ManagedInterfaceReferenceView,
    },
    Before {
        input: ManagedInterfaceReferenceView,
    },
    After {
        output: ManagedInterfaceReferenceView,
    },
    Failure {
        classification: String,
    },
    Completion {
        terminal: ManagedHookTerminal,
    },
}

impl ManagedInterfaceReferenceInput {
    pub fn phase(&self) -> HookPhase {
        match self {
            Self::Authorization { .. } => HookPhase::Authorization,
            Self::Admission { .. } => HookPhase::Admission,
            Self::Before { .. } => HookPhase::Before,
            Self::After { .. } => HookPhase::After,
            Self::Failure { .. } => HookPhase::Failure,
            Self::Completion { .. } => HookPhase::Completion,
        }
    }

    pub fn validate(&self) -> ContractResult {
        match self {
            Self::Authorization { input } | Self::Admission { input } | Self::Before { input } => {
                input.validate()
            }
            Self::After { output } => output.validate(),
            Self::Failure { classification } if !classification_code(classification) => {
                Err(invalid_value())
            }
            Self::Failure { .. } | Self::Completion { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedInterfaceReferenceHostFrame {
    pub protocol: String,
    pub call_id: String,
    pub handler: String,
    pub interface_id: String,
    pub interface_version: String,
    pub context: ManagedHookHostContext,
    pub input: ManagedInterfaceReferenceInput,
}

impl ManagedInterfaceReferenceHostFrame {
    pub fn validate(&self) -> ContractResult {
        let invocation = &self.context.invocation;
        if self.protocol != MANAGED_INTERFACE_PROTOCOL_V2
            || [
                &self.call_id,
                &self.handler,
                &self.interface_id,
                &self.interface_version,
                &invocation.invocation_id,
                &invocation.registry_fingerprint,
                &invocation.graph_fingerprint,
            ]
            .iter()
            .any(|s| !identity(s))
            || invocation.authority_revision < 0
            || self.context.deadline_unix_ms <= 0
            || serde_json::to_vec(self).map_err(|_| invalid_value())?.len()
                > MANAGED_INTERFACE_MAX_REQUEST_BYTES
        {
            return Err(ManagedInterfaceContractError(
                "invalid host context or frame budget".into(),
            ));
        }
        self.input.validate()
    }
}

/// The response has no actor, workspace, execution identity, patched input or replacement result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedInterfaceReferenceWorkerFrame {
    pub protocol: String,
    pub call_id: String,
    pub phase: HookPhase,
    pub outcome: ManagedHookOutcome,
}

impl ManagedInterfaceReferenceWorkerFrame {
    pub fn validate_for(&self, request: &ManagedInterfaceReferenceHostFrame) -> ContractResult {
        if serde_json::to_vec(self).map_err(|_| invalid_value())?.len()
            > MANAGED_INTERFACE_MAX_REPLY_BYTES
            || self.protocol != MANAGED_INTERFACE_PROTOCOL_V2
            || self.call_id != request.call_id
            || self.phase != request.input.phase()
        {
            return Err(ManagedInterfaceContractError(
                "uncorrelated response".into(),
            ));
        }
        let veto_phase = matches!(self.phase, HookPhase::Authorization | HookPhase::Admission);
        let permitted = match &self.outcome {
            ManagedHookOutcome::Continue => veto_phase,
            ManagedHookOutcome::Deny { classification } => {
                veto_phase && classification_code(classification)
            }
            ManagedHookOutcome::Observed => !veto_phase,
            ManagedHookOutcome::Failed { classification } => classification_code(classification),
        };
        if !permitted {
            return Err(ManagedInterfaceContractError(
                "outcome is not permitted for this phase".into(),
            ));
        }
        Ok(())
    }
}

fn invalid_value() -> ManagedInterfaceContractError {
    ManagedInterfaceContractError("projection violates resource budget".into())
}
